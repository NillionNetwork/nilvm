#![allow(clippy::arithmetic_side_effects, clippy::panic, clippy::indexing_slicing)]

use crate::{
    bit_operations::bit_adder::state::{BitAdderOperands, BitAdderState},
    random::random_bitwise::BitwiseNumberShares,
    simulator::symmetric::{InitializedProtocol, Protocol, SymmetricProtocolSimulator},
};
use anyhow::{anyhow, Error, Result};
use math_lib::{
    fields::PrimeField,
    modular::{AsBits, Modular, ModularNumber, SafePrime, U128SafePrime, U256SafePrime, U64SafePrime},
    polynomial::{point::Point, point_sequence::PointSequence},
};
use rstest::rstest;
use shamir_sharing::{
    party::{PartyId, PartyMapper},
    protocol::{PolyDegree, Shamir},
    secret_sharer::{FieldSecretSharer, PartyShares, ShamirSecretSharer},
};
use std::{collections::HashMap, marker::PhantomData, sync::Arc};

struct BitAdderProtocol<T: SafePrime> {
    operands: Vec<(u64, u64)>,
    polynomial_degree: u64,
    _unused: PhantomData<T>,
}

impl<T: SafePrime> BitAdderProtocol<T> {
    fn new(operands: Vec<(u64, u64)>, polynomial_degree: u64) -> Self {
        Self { operands, polynomial_degree, _unused: Default::default() }
    }

    fn validate_output(self, party_shares_outputs: HashMap<PartyId, Vec<BitwiseNumberShares<T>>>) -> Result<()> {
        // Reconstruct the outputs.
        let mapper = PartyMapper::<PrimeField<T>>::new(party_shares_outputs.keys().cloned().collect())?;
        let mut point_sequences =
            vec![vec![PointSequence::<PrimeField<T>>::default(); T::MODULO.bits()]; self.operands.len()];
        for (party_id, party_shares) in party_shares_outputs {
            if party_shares.len() != self.operands.len() {
                return Err(anyhow!(
                    "unexpected element share count: expected {}, got {}",
                    self.operands.len(),
                    party_shares.len()
                ));
            }
            let x =
                *mapper.abscissa(&party_id).ok_or_else(|| anyhow!("failed to find abscissa for party {party_id:?}"))?;
            for (element_index, shares) in party_shares.into_iter().enumerate() {
                for (bit_index, share) in shares.shares().into_iter().enumerate() {
                    point_sequences[element_index][bit_index].push(Point::new(x, *share.value()));
                }
            }
        }

        // Lagrange interpolate the outputs and check against the expected result.
        let zipped = point_sequences.into_iter().zip(self.operands.iter());
        for (point_sequences, bit_add_op) in zipped {
            let mut output = ModularNumber::ZERO;
            let mut factor = ModularNumber::ONE;
            for sequence in point_sequences {
                let bit = sequence.lagrange_interpolate()?;
                output = output + &(factor * &bit);
                factor = factor * &ModularNumber::two();
            }

            let expected_value = ModularNumber::from_u64(bit_add_op.0 + bit_add_op.1);
            assert_eq!(output, expected_value, "failed for {} and {}", bit_add_op.0, bit_add_op.1);
        }

        Ok(())
    }
}

impl<T: SafePrime> Protocol for BitAdderProtocol<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: FieldSecretSharer<PrimeField<T>> + FieldSecretSharer<PrimeField<T::SophiePrime>>,
{
    type State = BitAdderState<T>;
    type PrepareOutput = BitAdderConfig<T>;

    fn prepare(&self, parties: &[PartyId]) -> Result<Self::PrepareOutput, Error> {
        let parties = parties.to_vec();
        let mapper = PartyMapper::<PrimeField<T>>::new(parties.clone())?;
        // Note: the party id doesn't matter in this context
        let shamir = Shamir::<PrimeField<T>>::new(PartyId::from(0), self.polynomial_degree, parties.clone())?;

        let mut party_operands: PartyShares<Vec<BitAdderOperands<T>>> = PartyShares::default();
        for (left, right) in self.operands.iter() {
            let mut left = *left;
            let mut left_party_bits: PartyShares<Vec<ModularNumber<T>>> = PartyShares::default();
            let mut right = *right;
            let mut right_party_bits: PartyShares<Vec<ModularNumber<T>>> = PartyShares::default();
            let mut product_party_bits: PartyShares<Vec<ModularNumber<T>>> = PartyShares::default();
            for _ in 0..T::MODULO.bits() {
                let left_bit = if left % 2 == 0 { ModularNumber::ZERO } else { ModularNumber::ONE };
                left = left >> 1;
                let left_bit_shares = shamir.generate_shares(&left_bit, PolyDegree::T)?;
                for share in left_bit_shares.into_points().into_iter() {
                    let (x, bit_share) = share.into_coordinates();
                    let party_id = mapper.party(&x).ok_or_else(|| anyhow!("party id for {x:?} not found"))?;
                    left_party_bits.entry(party_id.clone()).or_default().push(bit_share);
                }
                let right_bit = if right % 2 == 0 { ModularNumber::ZERO } else { ModularNumber::ONE };
                right = right >> 1;
                let right_bit_shares = shamir.generate_shares(&right_bit, PolyDegree::T)?;
                for share in right_bit_shares.into_points().into_iter() {
                    let (x, bit_share) = share.into_coordinates();
                    let party_id = mapper.party(&x).ok_or_else(|| anyhow!("party id for {x:?} not found"))?;
                    right_party_bits.entry(party_id.clone()).or_default().push(bit_share);
                }
                let product_bit = left_bit * &right_bit;
                let product_bit_shares = shamir.generate_shares(&product_bit, PolyDegree::T)?;
                for share in product_bit_shares.into_points().into_iter() {
                    let (x, bit_share) = share.into_coordinates();
                    let party_id = mapper.party(&x).ok_or_else(|| anyhow!("party id for {x:?} not found"))?;
                    product_party_bits.entry(party_id.clone()).or_default().push(bit_share);
                }
            }
            for (party_id, left_bits) in left_party_bits.into_iter() {
                let right_bits = right_party_bits
                    .get(&party_id)
                    .ok_or_else(|| anyhow!("right shares for {party_id:?} not found"))?;
                let product_bits = product_party_bits
                    .get(&party_id)
                    .ok_or_else(|| anyhow!("product shares for {party_id:?} not found"))?;
                let operand =
                    BitAdderOperands::new(left_bits.into(), right_bits.clone().into(), product_bits.clone().into());
                party_operands.entry(party_id.clone()).or_default().push(operand);
            }
        }
        Ok(BitAdderConfig { parties: parties.to_vec(), party_operands })
    }

    fn initialize(
        &self,
        party_id: PartyId,
        config: &Self::PrepareOutput,
    ) -> Result<InitializedProtocol<Self::State>, anyhow::Error> {
        let operands =
            config.party_operands.get(&party_id).cloned().ok_or_else(|| anyhow!("shares for party {party_id:?}"))?;
        let secret_sharer = ShamirSecretSharer::new(party_id, self.polynomial_degree, config.parties.clone())?;
        let (state, messages) = BitAdderState::new(operands, Arc::new(secret_sharer))?;

        Ok(InitializedProtocol::new(state, messages))
    }
}

/// The internal configuration of a BIT-ADDER protocol.
struct BitAdderConfig<T: Modular> {
    parties: Vec<PartyId>,
    party_operands: PartyShares<Vec<BitAdderOperands<T>>>,
}

#[rstest]
#[case::u64(U64SafePrime)]
#[case::u128(U128SafePrime)]
#[case::u256(U256SafePrime)]
fn end_to_end<T: SafePrime>(#[case] _prime: T)
where
    ShamirSecretSharer<T>: FieldSecretSharer<PrimeField<T>> + FieldSecretSharer<PrimeField<T::SophiePrime>>,
{
    let max_rounds = 100;
    let polynomial_degree = 1;
    let network_size = 3;

    let inputs = vec![(4, 6), (34, 66), (135, 1464), (413, 9766), (1344, 1326)];

    let protocol = BitAdderProtocol::<T>::new(inputs, polynomial_degree);
    let simulator = SymmetricProtocolSimulator::new(network_size, max_rounds);
    let outputs = simulator.run_protocol(&protocol).expect("protocol run failed");
    let mut party_shares = HashMap::new();
    for output in outputs {
        match output.output {
            shares => {
                party_shares.insert(output.party_id, shares);
            }
        };
    }

    protocol.validate_output(party_shares).expect("validation failed");
}

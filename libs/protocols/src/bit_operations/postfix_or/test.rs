#![allow(clippy::arithmetic_side_effects, clippy::panic, clippy::indexing_slicing)]

use crate::{
    bit_operations::postfix_or::state::PostfixOrState,
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

struct PostfixOrProtocol<T: SafePrime> {
    operands: Vec<u64>,
    polynomial_degree: u64,
    _unused: PhantomData<T>,
}

impl<T: SafePrime> PostfixOrProtocol<T> {
    fn new(operands: Vec<u64>, polynomial_degree: u64) -> Self {
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
        for (point_sequences, operand) in zipped {
            let mut output = ModularNumber::ZERO;
            let mut factor = ModularNumber::ONE;
            for sequence in point_sequences {
                let bit = sequence.lagrange_interpolate()?;
                output = output + &(factor * &bit);
                factor = factor * &ModularNumber::two();
            }
            let mut expected = Vec::new();
            let mut number = *operand;
            for _ in 0..T::MODULO.bits() {
                let number_bit = if number % 2 == 0 { ModularNumber::ZERO } else { ModularNumber::ONE };
                number = number >> 1;
                expected.push(number_bit);
            }
            expected.reverse();
            let mut rolling = ModularNumber::ZERO;
            for bit in expected.iter_mut() {
                rolling = rolling + bit - &(rolling * bit);
                *bit = rolling.clone();
            }
            expected.reverse();
            let mut expected_value = ModularNumber::ZERO;
            let mut factor = ModularNumber::ONE;
            for bit in expected {
                expected_value = expected_value + &(factor * &bit);
                factor = factor * &ModularNumber::two();
            }
            assert_eq!(output, expected_value, "failed for {}", operand);
        }

        Ok(())
    }
}

impl<T: SafePrime> Protocol for PostfixOrProtocol<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: FieldSecretSharer<PrimeField<T>> + FieldSecretSharer<PrimeField<T::SophiePrime>>,
{
    type State = PostfixOrState<T>;
    type PrepareOutput = PostfixOrConfig<T>;

    fn prepare(&self, parties: &[PartyId]) -> Result<Self::PrepareOutput, Error> {
        let parties = parties.to_vec();
        let mapper = PartyMapper::<PrimeField<T>>::new(parties.clone())?;
        // Note: the party id doesn't matter in this context
        let shamir = Shamir::<PrimeField<T>>::new(PartyId::from(0), self.polynomial_degree, parties.clone())?;

        let mut party_operands: PartyShares<Vec<BitwiseNumberShares<T>>> = PartyShares::default();
        for number in self.operands.iter() {
            let mut number = *number;
            let mut party_bits: PartyShares<Vec<ModularNumber<T>>> = PartyShares::default();
            for _ in 0..T::MODULO.bits() {
                let number_bit = if number % 2 == 0 { ModularNumber::ZERO } else { ModularNumber::ONE };
                number = number >> 1;
                let number_bit_shares = shamir.generate_shares(&number_bit, PolyDegree::T)?;
                for share in number_bit_shares.into_points().into_iter() {
                    let (x, bit_share) = share.into_coordinates();
                    let party_id = mapper.party(&x).ok_or_else(|| anyhow!("party id for {x:?} not found"))?;
                    party_bits.entry(party_id.clone()).or_default().push(bit_share);
                }
            }
            for (party_id, bitwise) in party_bits.into_iter() {
                party_operands.entry(party_id.clone()).or_default().push(bitwise.into());
            }
        }
        Ok(PostfixOrConfig { parties: parties.to_vec(), party_operands })
    }

    fn initialize(
        &self,
        party_id: PartyId,
        config: &Self::PrepareOutput,
    ) -> Result<InitializedProtocol<Self::State>, anyhow::Error> {
        let operands =
            config.party_operands.get(&party_id).cloned().ok_or_else(|| anyhow!("shares for party {party_id:?}"))?;
        let secret_sharer = ShamirSecretSharer::new(party_id, self.polynomial_degree, config.parties.clone())?;
        let (state, messages) = PostfixOrState::new(operands, Arc::new(secret_sharer))?;

        Ok(InitializedProtocol::new(state, messages))
    }
}

/// The internal configuration of a POSTFIX-OR protocol.
struct PostfixOrConfig<T: Modular> {
    parties: Vec<PartyId>,
    party_operands: PartyShares<Vec<BitwiseNumberShares<T>>>,
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
    let polynomial_degree = 0;
    let network_size = 1;

    let inputs = vec![4, 134, 1234, 453, 312, 3432, 212023940];

    let protocol = PostfixOrProtocol::<T>::new(inputs, polynomial_degree);
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

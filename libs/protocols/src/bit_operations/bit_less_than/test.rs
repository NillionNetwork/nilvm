#![allow(clippy::arithmetic_side_effects, clippy::panic, clippy::indexing_slicing)]

use crate::{
    bit_operations::bit_less_than::state::{BitLessThanState, Comparands},
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

struct BitLessThanProtocol<T: SafePrime> {
    operands: Vec<(u64, u64)>,
    polynomial_degree: u64,
    _unused: PhantomData<T>,
}

impl<T: SafePrime> BitLessThanProtocol<T> {
    fn new(operands: Vec<(u64, u64)>, polynomial_degree: u64) -> Self {
        Self { operands, polynomial_degree, _unused: Default::default() }
    }

    fn validate_output(self, party_shares_outputs: HashMap<PartyId, Vec<ModularNumber<T>>>) -> Result<()> {
        // Reconstruct the outputs.
        let mapper = PartyMapper::<PrimeField<T>>::new(party_shares_outputs.keys().cloned().collect())?;
        let mut point_sequences = vec![PointSequence::<PrimeField<T>>::default(); self.operands.len()];
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
            for (element_index, share) in party_shares.into_iter().enumerate() {
                point_sequences[element_index].push(Point::new(x, share));
            }
        }

        // Lagrange interpolate the outputs and check against the expected result.
        let zipped = point_sequences.into_iter().zip(self.operands.iter());
        for (point_sequence, operand) in zipped {
            let output = point_sequence.lagrange_interpolate()?;
            let expected: u64 = (operand.0 < operand.1).into();
            let expected_value = ModularNumber::from_u64(expected);
            assert_eq!(output, expected_value, "failed for {} and {}", operand.0, operand.1);
        }

        Ok(())
    }
}

impl<T: SafePrime> Protocol for BitLessThanProtocol<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: FieldSecretSharer<PrimeField<T>> + FieldSecretSharer<PrimeField<T::SophiePrime>>,
{
    type State = BitLessThanState<T>;
    type PrepareOutput = BitLessThanConfig<T>;

    fn prepare(&self, parties: &[PartyId]) -> Result<Self::PrepareOutput, Error> {
        let parties = parties.to_vec();
        let mapper = PartyMapper::<PrimeField<T>>::new(parties.clone())?;
        // Note: the party id doesn't matter in this context
        let shamir = Shamir::<PrimeField<T>>::new(PartyId::from(0), self.polynomial_degree, parties.clone())?;

        let mut party_operands: PartyShares<Vec<Comparands<T>>> = PartyShares::default();
        for (public, secret) in self.operands.iter() {
            let mut party_bits: PartyShares<Vec<ModularNumber<T>>> = PartyShares::default();
            let mut number = *secret;
            for _ in 0..T::MODULO.bits() {
                let bit = if number % 2 == 0 { ModularNumber::ZERO } else { ModularNumber::ONE };
                number = number >> 1;
                let bit_shares = shamir.generate_shares(&bit, PolyDegree::T)?;
                for share in bit_shares.into_points().into_iter() {
                    let (x, bit_share) = share.into_coordinates();
                    let party_id = mapper.party(&x).ok_or_else(|| anyhow!("party id for {x:?} not found"))?;
                    party_bits.entry(party_id.clone()).or_default().push(bit_share);
                }
            }
            for (party_id, bits) in party_bits.into_iter() {
                let operand = Comparands::new(ModularNumber::from_u64(*public), bits.into());
                party_operands.entry(party_id.clone()).or_default().push(operand);
            }
        }
        Ok(BitLessThanConfig { parties: parties.to_vec(), party_operands })
    }

    fn initialize(
        &self,
        party_id: PartyId,
        config: &Self::PrepareOutput,
    ) -> Result<InitializedProtocol<Self::State>, anyhow::Error> {
        let operands =
            config.party_operands.get(&party_id).cloned().ok_or_else(|| anyhow!("shares for party {party_id:?}"))?;
        let secret_sharer = ShamirSecretSharer::new(party_id, self.polynomial_degree, config.parties.clone())?;
        let (state, messages) = BitLessThanState::new(operands, Arc::new(secret_sharer))?;

        Ok(InitializedProtocol::new(state, messages))
    }
}

/// The internal configuration of a BIT-LESS-THAN protocol.
struct BitLessThanConfig<T: Modular> {
    parties: Vec<PartyId>,
    party_operands: PartyShares<Vec<Comparands<T>>>,
}

#[rstest]
#[case::u64(U64SafePrime)]
#[case::u128(U128SafePrime)]
#[case::u256(U256SafePrime)]
fn end_to_end<T: SafePrime>(#[case] _prime: T)
where
    ShamirSecretSharer<T>: FieldSecretSharer<PrimeField<T>> + FieldSecretSharer<PrimeField<T::SophiePrime>>,
{
    let max_rounds = 10;
    let polynomial_degree = 1;
    let network_size = 3;

    let inputs = vec![
        (4, 6),
        (4, 4),
        (12304, 1231),
        (0, 0),
        (1, 0),
        (0, 1),
        (18446744072637906946, 18446744072637906949), // PRIME64 - 1, PRIME64 + 2
        (18446744072637906946, 18446744072637906946), // PRIME64 - 1, PRIME64 - 1
        (18446744072637906946, 18446744072637906945), // PRIME64 - 1, PRIME64 - 2
    ];

    let protocol = BitLessThanProtocol::<T>::new(inputs, polynomial_degree);
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

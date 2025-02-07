//! End-to-end tests for the RANDOM-BITWISE protocol.

#![allow(clippy::arithmetic_side_effects, clippy::panic, clippy::indexing_slicing)]

use super::state::{RanBitwiseState, RanBitwiseStateOutput};
use crate::{
    random::random_bitwise::BitwiseNumberShares,
    simulator::symmetric::{InitializedProtocol, Protocol, SymmetricProtocolSimulator},
};
use anyhow::{anyhow, Error};
use math_lib::{
    fields::PrimeField,
    modular::{Integer, ModularNumber, SafePrime, U64SafePrime},
    polynomial::{point::Point, point_sequence::PointSequence},
};
use shamir_sharing::{
    party::{PartyId, PartyMapper},
    secret_sharer::{PartyShares, SafePrimeSecretSharer, ShamirSecretSharer},
};
use std::{marker::PhantomData, sync::Arc};

struct RanBitwiseProtocol<T> {
    element_count: usize,
    polynomial_degree: u64,
    _unused: PhantomData<T>,
}

impl<T> RanBitwiseProtocol<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    pub fn new(element_count: usize, polynomial_degree: u64) -> Self {
        Self { element_count, polynomial_degree, _unused: Default::default() }
    }

    pub fn validate_output(&self, party_shares: PartyShares<Vec<BitwiseNumberShares<T>>>) -> Result<(), Error> {
        let prime_bits = T::Normal::BITS;
        let mapper = PartyMapper::<PrimeField<T>>::new(party_shares.keys().cloned().collect())?;

        // We have N vecs one per element count, which has M vecs one per bit, which is a point
        // sequence with a point per party.
        let mut point_sequences = vec![vec![PointSequence::<PrimeField<T>>::default(); prime_bits]; self.element_count];
        for (party_id, party_shares) in party_shares {
            if party_shares.len() != self.element_count {
                return Err(anyhow!("unexpected element share count"));
            }

            let x =
                *mapper.abscissa(&party_id).ok_or_else(|| anyhow!("failed to find abscissa for party {party_id:?}"))?;
            for (element_index, element_shares) in party_shares.into_iter().enumerate() {
                let element_shares = Vec::from(element_shares);
                if element_shares.len() != prime_bits {
                    return Err(anyhow!("unexpected bit share count"));
                }
                for (index, share) in element_shares.into_iter().enumerate() {
                    point_sequences[element_index][index].push(Point::new(x, ModularNumber::from(share)));
                }
            }
        }

        for number_sequences in point_sequences {
            for point_sequence in number_sequences {
                // We can't really check anything here besides that interpolation doesn't fail.
                point_sequence.lagrange_interpolate().expect("interpolation failed");
            }
        }

        Ok(())
    }
}

impl<T> Protocol for RanBitwiseProtocol<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    type State = RanBitwiseState<T>;
    type PrepareOutput = RanBitwiseConfig;

    fn prepare(&self, parties: &[PartyId]) -> Result<Self::PrepareOutput, Error> {
        let parties = parties.to_vec();
        Ok(RanBitwiseConfig { parties })
    }

    fn initialize(
        &self,
        party_id: PartyId,
        config: &Self::PrepareOutput,
    ) -> Result<InitializedProtocol<RanBitwiseState<T>>, Error> {
        let secret_sharer = ShamirSecretSharer::new(party_id, self.polynomial_degree, config.parties.clone())?;
        let (state, initial_messages) =
            RanBitwiseState::new(super::RanBitwiseMode::Full, self.element_count, Arc::new(secret_sharer))?;
        Ok(InitializedProtocol::new(state, initial_messages))
    }
}

struct RanBitwiseConfig {
    parties: Vec<PartyId>,
}

#[test]
fn end_to_end() {
    let max_rounds = 100;
    let polynomial_degree = 1;
    let network_size = 5;
    let output_numbers = 5;

    let protocol = RanBitwiseProtocol::<U64SafePrime>::new(output_numbers, polynomial_degree);
    let simulator = SymmetricProtocolSimulator::new(network_size, max_rounds);
    let outputs = simulator.run_protocol(&protocol).expect("protocol run failed");
    let mut party_shares = PartyShares::default();
    for output in outputs {
        match output.output {
            RanBitwiseStateOutput::Success { shares } => {
                party_shares.insert(output.party_id, shares);
            }
            // These two can happen by chance and should be retried. Once we have deterministic tests that are
            // guaranteed not to fail, this should be a test failure
            RanBitwiseStateOutput::RanBitAbort => {
                println!("RAN-BIT abort!");
                return;
            }
            RanBitwiseStateOutput::Abort => {
                println!("RANDOM-BITWISE abort!");
                return;
            }
        };
    }

    protocol.validate_output(party_shares).expect("validation failed");
}

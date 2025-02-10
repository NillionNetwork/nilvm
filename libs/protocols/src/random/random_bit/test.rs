//! End-to-end tests for the RAN-BIT protocol.

#![allow(clippy::arithmetic_side_effects, clippy::panic, clippy::indexing_slicing)]

use super::{RandomBitState, RandomBitStateOutput};
use crate::simulator::symmetric::{InitializedProtocol, Protocol, SymmetricProtocolSimulator};
use anyhow::{anyhow, Error};
use math_lib::{
    fields::PrimeField,
    modular::{ModularNumber, SafePrime, U64SafePrime},
    polynomial::{point::Point, point_sequence::PointSequence},
};
use shamir_sharing::{
    party::{PartyId, PartyMapper},
    secret_sharer::{PartyShares, SafePrimeSecretSharer, ShamirSecretSharer},
};
use std::{marker::PhantomData, sync::Arc};

struct RanBitProtocol<T> {
    bit_count: usize,
    polynomial_degree: u64,
    _unused: PhantomData<T>,
}

impl<T: SafePrime> RanBitProtocol<T> {
    pub fn new(bit_count: usize, polynomial_degree: u64) -> Self {
        Self { bit_count, polynomial_degree, _unused: Default::default() }
    }

    pub fn validate_output(&self, party_shares: PartyShares<Vec<ModularNumber<T>>>) -> Result<(), Error> {
        let mapper = PartyMapper::<PrimeField<T>>::new(party_shares.keys().cloned().collect())?;
        let mut point_sequences = vec![PointSequence::<PrimeField<T>>::default(); self.bit_count];
        for (party_id, shares) in party_shares {
            if shares.len() != self.bit_count {
                return Err(anyhow!("unexpected share count"));
            }
            for (index, share) in shares.into_iter().enumerate() {
                let x = *mapper
                    .abscissa(&party_id)
                    .ok_or_else(|| anyhow!("failed to find abscissa for party {party_id:?}"))?;
                point_sequences[index].push(Point::new(x, share));
            }
        }

        let mut zero_count = 0;
        let mut one_count = 0;
        for points in point_sequences {
            let value = points.lagrange_interpolate()?;
            if value == ModularNumber::ZERO {
                zero_count += 1;
            } else if value == ModularNumber::ONE {
                one_count += 1;
            } else {
                panic!("found unexpected value: {value}");
            }
        }
        // We're generating a bunch of bits so statistically, at least one of them should be zero
        // and another one should be one. This ensures we don't have a bogus algorithm that
        // generates always either 0 or 1.
        assert!(zero_count > 0, "no zero values found!");
        assert!(one_count > 0, "no one values found!");
        Ok(())
    }
}

impl<T> Protocol for RanBitProtocol<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    type State = RandomBitState<T>;
    type PrepareOutput = RanBitConfig;

    fn prepare(&self, parties: &[PartyId]) -> Result<Self::PrepareOutput, Error> {
        let parties = parties.to_vec();
        Ok(RanBitConfig { parties })
    }

    fn initialize(
        &self,
        party_id: PartyId,
        config: &Self::PrepareOutput,
    ) -> Result<InitializedProtocol<Self::State>, Error> {
        let secret_sharer = ShamirSecretSharer::new(party_id, self.polynomial_degree, config.parties.clone())?;
        let (state, initial_messages) = RandomBitState::new(self.bit_count, Arc::new(secret_sharer))?;
        Ok(InitializedProtocol::new(state, initial_messages))
    }
}

struct RanBitConfig {
    parties: Vec<PartyId>,
}

#[test]
fn end_to_end() {
    let max_rounds = 100;
    let polynomial_degree = 1;
    let network_size = 5;
    let output_bits = 100;

    let protocol = RanBitProtocol::<U64SafePrime>::new(output_bits, polynomial_degree);
    let simulator = SymmetricProtocolSimulator::new(network_size, max_rounds);
    let outputs = simulator.run_protocol(&protocol).expect("protocol run failed");
    let mut party_shares = PartyShares::default();
    for output in outputs {
        match output.output {
            RandomBitStateOutput::Success { shares: bit_shares } => {
                let bit_shares: Vec<_> = bit_shares.into_iter().map(ModularNumber::from).collect();
                party_shares.insert(output.party_id, bit_shares);
            }
            RandomBitStateOutput::Abort => {
                // This can happen by chance and should be retried. Once we have deterministic tests that are
                // guaranteed not to fail, this should be a test failure
                print!("RAN-BIT abort!");
                return;
            }
        };
    }

    protocol.validate_output(party_shares).expect("validation failed");
}

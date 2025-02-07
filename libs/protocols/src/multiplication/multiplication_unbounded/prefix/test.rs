//! End-to-end tests for the PREFIX-MULT-PRE protocol.

#![allow(clippy::indexing_slicing)]

use super::{
    super::prefix::{PrepPrefixMultState, PrepPrefixMultStateOutput},
    PrefixMultTuple,
};
use crate::simulator::symmetric::{InitializedProtocol, Protocol, SymmetricProtocolSimulator};
use anyhow::{anyhow, Error};
use basic_types::Batches;
use math_lib::{
    fields::{Inv, PrimeField},
    modular::{ModularNumber, SafePrime, U256SafePrime},
    polynomial::{point::Point, point_sequence::PointSequence},
};
use shamir_sharing::{
    party::{PartyId, PartyMapper},
    secret_sharer::{PartyShares, SafePrimeSecretSharer, ShamirSecretSharer},
};
use std::{marker::PhantomData, sync::Arc};

struct PrefixMultPreProtocol<T: SafePrime> {
    batch_count: usize,
    batch_size: usize,
    polynomial_degree: u64,
    _unused: PhantomData<T>,
}

impl<T: SafePrime> PrefixMultPreProtocol<T> {
    pub fn new(batch_count: usize, batch_size: usize, polynomial_degree: u64) -> Self {
        Self { batch_count, batch_size, polynomial_degree, _unused: Default::default() }
    }

    pub fn validate_output(&self, party_shares: PartyShares<Batches<PrefixMultTuple<T>>>) -> Result<(), Error> {
        let mapper = PartyMapper::<PrimeField<T>>::new(party_shares.keys().cloned().collect())?;
        let mut masks_point_sequences =
            vec![vec![PointSequence::<PrimeField<T>>::default(); self.batch_size]; self.batch_count];
        let mut dominos_point_sequences =
            vec![vec![PointSequence::<PrimeField<T>>::default(); self.batch_size]; self.batch_count];
        for (party_id, shares) in party_shares {
            if shares.len() != self.batch_count {
                return Err(anyhow!("unexpected batch count: expected {}, got {}", self.batch_count, shares.len()));
            }
            let x =
                *mapper.abscissa(&party_id).ok_or_else(|| anyhow!("failed to find abscissa for party {party_id:?}"))?;
            for (batch_index, shares) in shares.into_iter().enumerate() {
                for (index, share) in shares.into_iter().enumerate() {
                    masks_point_sequences[batch_index][index].push(Point::new(x, share.mask));
                    dominos_point_sequences[batch_index][index].push(Point::new(x, share.domino));
                }
            }
        }

        let sequence_batches = masks_point_sequences.into_iter().zip(dominos_point_sequences);
        for (masks_sequences, dominos_sequences) in sequence_batches {
            let sequences = masks_sequences.into_iter().zip(dominos_sequences.into_iter());
            let mut previous_mask = ModularNumber::ONE;
            for (masks_sequence, dominos_sequence) in sequences {
                let mask = masks_sequence.lagrange_interpolate()?;
                let domino = dominos_sequence.lagrange_interpolate()?;
                let expected_domino = mask.inv()? * &previous_mask;
                assert_eq!(domino, expected_domino);
                previous_mask = mask;
            }
        }
        Ok(())
    }
}

impl<T: SafePrime> Protocol for PrefixMultPreProtocol<T>
where
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    type State = PrepPrefixMultState<T>;
    type PrepareOutput = PrepPrefixMultConfig;

    fn prepare(&self, parties: &[PartyId]) -> Result<Self::PrepareOutput, Error> {
        let parties = parties.to_vec();
        Ok(PrepPrefixMultConfig { parties })
    }

    fn initialize(
        &self,
        party_id: PartyId,
        config: &Self::PrepareOutput,
    ) -> Result<InitializedProtocol<Self::State>, Error> {
        let secret_sharer = ShamirSecretSharer::new(party_id, self.polynomial_degree, config.parties.clone())?;
        let (state, initial_messages) =
            PrepPrefixMultState::new(self.batch_count, self.batch_size, Arc::new(secret_sharer))?;
        Ok(InitializedProtocol::new(state, initial_messages))
    }
}

struct PrepPrefixMultConfig {
    parties: Vec<PartyId>,
}

#[test]
fn end_to_end() {
    let max_rounds = 100;
    let polynomial_degree = 1;
    let network_size = 5;
    let batch_count = 4;
    let batch_size = 5;

    let protocol = PrefixMultPreProtocol::<U256SafePrime>::new(batch_count, batch_size, polynomial_degree);
    let simulator = SymmetricProtocolSimulator::new(network_size, max_rounds);
    let outputs = simulator.run_protocol(&protocol).expect("protocol run failed");
    let mut party_shares = PartyShares::default();
    for output in outputs {
        match output.output {
            PrepPrefixMultStateOutput::Success { shares } => {
                party_shares.insert(output.party_id, shares);
            }
            PrepPrefixMultStateOutput::InvRanAbort => {
                // This can happen by chance and should be retried. Once we have deterministic tests that are
                // guaranteed not to fail, this should be a test failure
                print!("INV-RAN fail!");
                return;
            }
        };
    }

    protocol.validate_output(party_shares).expect("validation failed");
}

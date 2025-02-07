//! Implementation the PREP-COMPARE protocol to be run under `simulator::SymmetricProtocolSimulator`

use super::state::PrepCompareState;
use crate::simulator::symmetric::{InitializedProtocol, Protocol};
use anyhow::Error;
use math_lib::modular::SafePrime;
use shamir_sharing::{
    party::PartyId,
    secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer},
};
use std::{marker::PhantomData, sync::Arc};

/// The PREP-COMPARE protocol.
///
/// This is only meant to be used under a simulator, be it for testing or benchmarking purposes.
pub struct PrepCompareProtocol<T> {
    element_count: usize,
    polynomial_degree: u64,
    _unused: PhantomData<T>,
}

impl<T> PrepCompareProtocol<T> {
    /// Constructs a new PREP-COMPARE protocol.
    pub fn new(element_count: usize, polynomial_degree: u64) -> Self {
        Self { element_count, polynomial_degree, _unused: Default::default() }
    }
}

impl<T> Protocol for PrepCompareProtocol<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    type State = PrepCompareState<T>;
    type PrepareOutput = PrepCompareConfig;

    fn prepare(&self, parties: &[PartyId]) -> Result<Self::PrepareOutput, Error> {
        let parties = parties.to_vec();
        Ok(PrepCompareConfig { parties })
    }

    fn initialize(
        &self,
        party_id: PartyId,
        config: &Self::PrepareOutput,
    ) -> Result<InitializedProtocol<Self::State>, Error> {
        let secret_sharer = ShamirSecretSharer::new(party_id, self.polynomial_degree, config.parties.clone())?;
        let (state, initial_messages) = PrepCompareState::new(self.element_count, Arc::new(secret_sharer))?;
        Ok(InitializedProtocol::new(state, initial_messages))
    }
}

/// The internal configuration of an PREP-COMPARE protocol.
pub struct PrepCompareConfig {
    parties: Vec<PartyId>,
}

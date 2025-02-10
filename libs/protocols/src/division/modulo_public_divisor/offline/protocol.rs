//! Implementation the PREP-MODULO protocol to be run under `simulator::SymmetricProtocolSimulator`

use super::state::PrepModuloState;
use crate::simulator::symmetric::{InitializedProtocol, Protocol};
use anyhow::Error;
use basic_types::PartyId;
use math_lib::modular::SafePrime;
use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer};
use std::{marker::PhantomData, sync::Arc};

/// The PREP-MODULO protocol.
///
/// This is only meant to be used under a simulator, be it for testing or benchmarking purposes.
pub struct PrepModuloProtocol<T> {
    element_count: usize,
    polynomial_degree: u64,
    kappa: usize,
    k: usize,
    _unused: PhantomData<T>,
}

impl<T> PrepModuloProtocol<T> {
    /// Constructs a new PREP-MODULO protocol.
    pub fn new(element_count: usize, polynomial_degree: u64, kappa: usize, k: usize) -> Self {
        Self { element_count, polynomial_degree, kappa, k, _unused: Default::default() }
    }
}

impl<T> Protocol for PrepModuloProtocol<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    type State = PrepModuloState<T>;
    type PrepareOutput = PrepModuloConfig;

    fn prepare(&self, parties: &[PartyId]) -> Result<Self::PrepareOutput, Error> {
        let parties = parties.to_vec();
        Ok(PrepModuloConfig { parties })
    }

    fn initialize(
        &self,
        party_id: PartyId,
        config: &Self::PrepareOutput,
    ) -> Result<InitializedProtocol<Self::State>, Error> {
        let secret_sharer = ShamirSecretSharer::new(party_id, self.polynomial_degree, config.parties.clone())?;
        let (state, initial_messages) =
            PrepModuloState::new(self.element_count, self.kappa, self.k, Arc::new(secret_sharer))?;
        Ok(InitializedProtocol::new(state, initial_messages))
    }
}

/// The internal configuration of an PREP-MODULO protocol.
pub struct PrepModuloConfig {
    parties: Vec<PartyId>,
}

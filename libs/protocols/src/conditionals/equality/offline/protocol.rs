//! Implementation the PREP PRIVATE OUTPUT EQUALITY protocol to be run under `simulator::SymmetricProtocolSimulator`

use super::state::PrepPrivateOutputEqualityState;
use crate::{
    conditionals::equality::POLY_EVAL_DEGREE,
    simulator::symmetric::{InitializedProtocol, Protocol},
};
use anyhow::Error;
use math_lib::modular::SafePrime;
use shamir_sharing::{
    party::PartyId,
    secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer},
};
use std::{marker::PhantomData, sync::Arc};

/// The PrepPrivateOutputEquality protocol.
///
/// This is only meant to be used under a simulator, be it for testing or benchmarking purposes.
pub struct PrepPrivateOutputEqualityProtocol<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    element_count: usize,
    polynomial_degree: u64,
    poly_eval_degree: u64,
    _unused: PhantomData<T>,
}

impl<T> PrepPrivateOutputEqualityProtocol<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Constructs a new PrepPrivateOutputEquality protocol.
    pub fn new(element_count: usize, polynomial_degree: u64) -> Self {
        Self { element_count, polynomial_degree, poly_eval_degree: POLY_EVAL_DEGREE, _unused: Default::default() }
    }
}

impl<T> Protocol for PrepPrivateOutputEqualityProtocol<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    type State = PrepPrivateOutputEqualityState<T>;
    type PrepareOutput = PrepPrivateOutputEqualityConfig;

    fn prepare(&self, parties: &[PartyId]) -> Result<Self::PrepareOutput, Error> {
        let parties = parties.to_vec();
        Ok(PrepPrivateOutputEqualityConfig { parties })
    }

    fn initialize(
        &self,
        party_id: PartyId,
        config: &Self::PrepareOutput,
    ) -> Result<InitializedProtocol<Self::State>, Error> {
        let secret_sharer = ShamirSecretSharer::new(party_id, self.polynomial_degree, config.parties.clone())?;
        let (state, initial_messages) =
            PrepPrivateOutputEqualityState::new(self.element_count, self.poly_eval_degree, Arc::new(secret_sharer))?;
        Ok(InitializedProtocol::new(state, initial_messages))
    }
}

/// The internal configuration of a PrepPrivateOutputEquality protocol.
pub struct PrepPrivateOutputEqualityConfig {
    parties: Vec<PartyId>,
}

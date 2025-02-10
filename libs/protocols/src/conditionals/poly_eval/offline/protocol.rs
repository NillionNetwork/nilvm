//! Implementation the PREP POLY EVAL protocol to be run under `simulator::SymmetricProtocolSimulator`

use super::state::PrepPolyEvalState;
use crate::simulator::symmetric::{InitializedProtocol, Protocol};
use anyhow::Error;
use math_lib::modular::SafePrime;
use shamir_sharing::{
    party::PartyId,
    secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer},
};
use std::{marker::PhantomData, sync::Arc};

/// The PrepPolyEval protocol.
///
/// This is only meant to be used under a simulator, be it for testing or benchmarking purposes.
pub struct PrepPolyEvalProtocol<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    element_count: usize,
    polynomial_degree: u64,
    poly_eval_degree: u64,
    _unused: PhantomData<T>,
}

impl<T> PrepPolyEvalProtocol<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Constructs a new PrepPolyEval protocol.
    pub fn new(element_count: usize, polynomial_degree: u64, poly_eval_degree: u64) -> Self {
        Self { element_count, polynomial_degree, poly_eval_degree, _unused: Default::default() }
    }
}

impl<T> Protocol for PrepPolyEvalProtocol<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    type State = PrepPolyEvalState<T>;
    type PrepareOutput = PrepPolyEvalConfig;

    fn prepare(&self, parties: &[PartyId]) -> Result<Self::PrepareOutput, Error> {
        let parties = parties.to_vec();
        Ok(PrepPolyEvalConfig { parties })
    }

    fn initialize(
        &self,
        party_id: PartyId,
        config: &Self::PrepareOutput,
    ) -> Result<InitializedProtocol<Self::State>, Error> {
        let secret_sharer = ShamirSecretSharer::new(party_id, self.polynomial_degree, config.parties.clone())?;
        let (state, initial_messages) =
            PrepPolyEvalState::new(self.element_count, self.poly_eval_degree, Arc::new(secret_sharer))?;
        Ok(InitializedProtocol::new(state, initial_messages))
    }
}

/// The internal configuration of a PrepPolyEval protocol.
pub struct PrepPolyEvalConfig {
    parties: Vec<PartyId>,
}

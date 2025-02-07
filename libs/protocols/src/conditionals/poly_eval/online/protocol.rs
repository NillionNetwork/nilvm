//! Implementation the PolyEval protocol to be run under `simulator::SymmetricProtocolSimulator`

use super::state::PolyEvalState;
use crate::{
    conditionals::poly_eval::offline::{output::PrepPolyEvalShares, validation::PrepPolyEvalBuilder},
    simulator::symmetric::{InitializedProtocol, Protocol},
};
use anyhow::{anyhow, Error};
use math_lib::{
    fields::PrimeField,
    modular::{Modular, ModularNumber, SafePrime},
    polynomial::Polynomial,
};
use shamir_sharing::{
    party::PartyId,
    secret_sharer::{PartyShares, SafePrimeSecretSharer, ShamirSecretSharer},
};
use std::{marker::PhantomData, sync::Arc};

/// The PolyEval protocol.
///
/// This is only meant to be used under a simulator, be it for testing or benchmarking purposes.
pub struct PolyEvalProtocol<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    polynomial_degree: u64,
    x: Vec<ModularNumber<T>>,                    // The x values
    polynomials: Vec<Polynomial<PrimeField<T>>>, // The polynomials to evaluate on the x values
    poly_eval_degree: u64,                       // The degree of the polynomial evaluated
    _unused: PhantomData<T>,
}

impl<T> PolyEvalProtocol<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Constructs a new PolyEval protocol.
    pub fn new(
        polynomial_degree: u64,
        x: Vec<ModularNumber<T>>,
        polynomials: Vec<Polynomial<PrimeField<T>>>,
        poly_eval_degree: u64,
    ) -> Self {
        Self { polynomial_degree, x, polynomials, poly_eval_degree, _unused: Default::default() }
    }
}

impl<T> Protocol for PolyEvalProtocol<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    type State = PolyEvalState<T>;
    type PrepareOutput = PolyEvalConfig<T>;

    fn prepare(&self, parties: &[PartyId]) -> Result<Self::PrepareOutput, Error> {
        // Produces the PREP POLY EVAL elements with a builder
        let parties = parties.to_vec();
        let party_id = parties.get(0).cloned().ok_or(anyhow!("Error extracting Party Id"))?;
        let secret_sharer = ShamirSecretSharer::new(party_id, self.polynomial_degree, parties.clone())?;
        let builder = PrepPolyEvalBuilder::new(&secret_sharer, rand::thread_rng())?;
        let party_shares: PartyShares<Vec<PrepPolyEvalShares<T>>> =
            builder.build(self.polynomials.len(), self.poly_eval_degree).unwrap();

        Ok(PolyEvalConfig { parties, party_shares })
    }

    fn initialize(
        &self,
        party_id: PartyId,
        config: &Self::PrepareOutput,
    ) -> Result<InitializedProtocol<Self::State>, Error> {
        let secret_sharer = ShamirSecretSharer::new(party_id.clone(), self.polynomial_degree, config.parties.clone())?;

        let prep_poly_eval_output =
            config.party_shares.get(&party_id).ok_or(anyhow!("Error extracting party shares"))?.clone();
        let (state, initial_messages) = PolyEvalState::new(
            self.x.clone(),
            self.polynomials.clone(),
            prep_poly_eval_output,
            Arc::new(secret_sharer),
        )?;
        Ok(InitializedProtocol::new(state, initial_messages))
    }
}

/// The internal configuration of a PolyEval protocol.
pub struct PolyEvalConfig<T: Modular> {
    parties: Vec<PartyId>,
    party_shares: PartyShares<Vec<PrepPolyEvalShares<T>>>,
}

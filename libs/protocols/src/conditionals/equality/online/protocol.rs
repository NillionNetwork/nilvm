//! Implementation the Private Output Equality protocol to be run under `simulator::SymmetricProtocolSimulator`

use super::state::PrivateOutputEqualityState;
use crate::{
    conditionals::equality::offline::{
        output::PrepPrivateOutputEqualityShares, validation::PrepPrivateOutputEqualitySharesBuilder,
    },
    simulator::symmetric::{InitializedProtocol, Protocol},
};
use anyhow::{anyhow, Error};
use math_lib::modular::{ModularNumber, SafePrime};
use shamir_sharing::{
    party::PartyId,
    protocol::PolyDegree,
    secret_sharer::{PartyShares, SafePrimeSecretSharer, SecretSharer, ShamirSecretSharer},
};
use std::sync::Arc;

/// The PrivateOutputEquality protocol.
///
/// This is only meant to be used under a simulator, be it for testing or benchmarking purposes.
pub struct PrivateOutputEqualityProtocol<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    polynomial_degree: u64,
    x: Vec<ModularNumber<T>>, // The x values
    y: Vec<ModularNumber<T>>, // The y values
}

impl<T> PrivateOutputEqualityProtocol<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Constructs a new PrivateOutputEquality protocol.
    pub fn new(polynomial_degree: u64, x: Vec<ModularNumber<T>>, y: Vec<ModularNumber<T>>) -> Self {
        Self { polynomial_degree, x, y }
    }

    /// Secret shares a value.
    fn secret_share(
        secret_sharer: &ShamirSecretSharer<T>,
        value: &ModularNumber<T>,
    ) -> Result<PartyShares<ModularNumber<T>>, Error> {
        Ok(secret_sharer.generate_shares(value, PolyDegree::T)?)
    }

    /// Secret shares a vector of values.
    fn secret_share_vector(
        secret_sharer: &ShamirSecretSharer<T>,
        values: &Vec<ModularNumber<T>>,
    ) -> Result<PartyShares<Vec<ModularNumber<T>>>, Error> {
        let mut shares = PartyShares::default();
        for value in values {
            let party_shares = Self::secret_share(secret_sharer, value)?;
            for (party_id, share) in party_shares {
                shares.entry(party_id).or_insert_with(Vec::new).push(share);
            }
        }
        Ok(shares)
    }

    /// Creates the shares for the x and y value vectors.
    fn create_shares(
        &self,
        secret_sharer: &ShamirSecretSharer<T>,
    ) -> (PartyShares<Vec<ModularNumber<T>>>, PartyShares<Vec<ModularNumber<T>>>) {
        let x_shares: PartyShares<Vec<ModularNumber<T>>> = Self::secret_share_vector(secret_sharer, &self.x).unwrap();
        let y_shares: PartyShares<Vec<ModularNumber<T>>> = Self::secret_share_vector(secret_sharer, &self.y).unwrap();
        (x_shares, y_shares)
    }
}

impl<T> Protocol for PrivateOutputEqualityProtocol<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    type State = PrivateOutputEqualityState<T>;
    type PrepareOutput = PrivateOutputEqualityConfig<T>;

    fn prepare(&self, parties: &[PartyId]) -> Result<Self::PrepareOutput, Error> {
        // Produces the PREP PRIVATE OUTPUT EQUALITY elements with a builder
        let parties = parties.to_vec();
        let party_id = parties.get(0).cloned().ok_or(anyhow!("Error extracting Party Id"))?;
        let secret_sharer = ShamirSecretSharer::new(party_id, self.polynomial_degree, parties.clone())?;
        let builder = PrepPrivateOutputEqualitySharesBuilder::new(&secret_sharer, rand::thread_rng())?;
        let party_shares: PartyShares<Vec<PrepPrivateOutputEqualityShares<T>>> = builder.build(self.x.len()).unwrap();

        let (x_shares, y_shares) = self.create_shares(&secret_sharer);
        Ok(PrivateOutputEqualityConfig { parties, party_shares, x_shares, y_shares })
    }

    fn initialize(
        &self,
        party_id: PartyId,
        config: &Self::PrepareOutput,
    ) -> Result<InitializedProtocol<Self::State>, Error> {
        let secret_sharer = ShamirSecretSharer::new(party_id.clone(), self.polynomial_degree, config.parties.clone())?;
        let x_shares =
            config.x_shares.get(&party_id).cloned().ok_or_else(|| anyhow!("x_shares for party {party_id:?}"))?;

        let y_shares =
            config.y_shares.get(&party_id).cloned().ok_or_else(|| anyhow!("y_shares for party {party_id:?}"))?;
        let prep_poly_eval_output =
            config.party_shares.get(&party_id).ok_or(anyhow!("Error extracting party shares"))?.clone();
        let (state, initial_messages) =
            PrivateOutputEqualityState::new(x_shares, y_shares, prep_poly_eval_output, Arc::new(secret_sharer))?;
        Ok(InitializedProtocol::new(state, initial_messages))
    }
}

/// The internal configuration of a PrivateOutputEquality protocol.
pub struct PrivateOutputEqualityConfig<T>
where
    T: SafePrime,
{
    parties: Vec<PartyId>,
    party_shares: PartyShares<Vec<PrepPrivateOutputEqualityShares<T>>>,
    x_shares: PartyShares<Vec<ModularNumber<T>>>, // The x values
    y_shares: PartyShares<Vec<ModularNumber<T>>>, // The y values
}

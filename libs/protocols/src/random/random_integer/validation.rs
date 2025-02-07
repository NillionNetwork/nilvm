//! Utilities for validation of a RANDOM output.
//!
//! These are enabled via the `validation` feature flag and should only be used for testing.

use math_lib::modular::{ModularNumber, SafePrime};
use shamir_sharing::{
    protocol::PolyDegree,
    secret_sharer::{PartyShares, SafePrimeSecretSharer, SecretSharer, ShamirSecretSharer},
};

/// Builder that creates Random Integer shares in plain.
///
/// **This is meant to be used for testing purposes only**.
pub struct RandomIntegerSharesBuilder<'a, T: SafePrime> {
    secret_sharer: &'a ShamirSecretSharer<T>,
}

impl<'a, T> RandomIntegerSharesBuilder<'a, T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Construct a new shares builder.
    pub fn new(secret_sharer: &'a ShamirSecretSharer<T>) -> anyhow::Result<Self> {
        Ok(Self { secret_sharer })
    }

    /// Build `count` Random Integer shares.
    pub fn build(self, count: usize) -> anyhow::Result<PartyShares<Vec<ModularNumber<T>>>> {
        let random_integer = (0..count).map(|_| ModularNumber::gen_random()).collect();
        let random_integer_shares: PartyShares<Vec<ModularNumber<T>>> =
            self.secret_sharer.generate_shares(&random_integer, PolyDegree::T)?;
        Ok(random_integer_shares)
    }
}

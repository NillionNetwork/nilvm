//! Utilities for validation of a Random Bit output.
//!
//! These are enabled via the `validation` feature flag and should only be used for testing.

use crate::random::random_bit::BitShare;
use math_lib::modular::{ModularNumber, SafePrime};
use shamir_sharing::{
    protocol::PolyDegree,
    secret_sharer::{PartyShares, SafePrimeSecretSharer, SecretSharer, ShamirSecretSharer},
};

/// Builder that creates Random Integer shares in plain.
///
/// **This is meant to be used for testing purposes only**.
pub struct RandomBooleanSharesBuilder<'a, T: SafePrime> {
    secret_sharer: &'a ShamirSecretSharer<T>,
}

impl<'a, T> RandomBooleanSharesBuilder<'a, T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Construct a new shares builder.
    pub fn new(secret_sharer: &'a ShamirSecretSharer<T>) -> anyhow::Result<Self> {
        Ok(Self { secret_sharer })
    }

    /// Build `count` Random Boolean shares.
    pub fn build(self, count: usize) -> anyhow::Result<PartyShares<Vec<BitShare<T>>>> {
        let mut party_shares: PartyShares<Vec<BitShare<T>>> = PartyShares::default();
        for _ in 0..count {
            let boolean = (ModularNumber::gen_random() % &ModularNumber::two())?;
            let boolean_shares: PartyShares<ModularNumber<T>> =
                self.secret_sharer.generate_shares(&boolean, PolyDegree::T)?;
            for (party_id, share) in boolean_shares {
                party_shares.entry(party_id).or_default().push(share.into());
            }
        }
        Ok(party_shares)
    }
}

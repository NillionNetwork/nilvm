//! Utilities for validation of the PREP-PUBLIC-OUTPUT-EQUALITY output.
//!
//! These are enabled via the `validation` feature flag and should only be used for testing.

use anyhow::{anyhow, Error, Result};
use math_lib::{
    fields::PrimeField,
    modular::{CryptoRngCore, ModularNumber, SafePrime},
    polynomial::{point::Point, point_sequence::PointSequence},
};
use shamir_sharing::{
    party::{PartyId, PartyMapper},
    protocol::PolyDegree,
    secret_sharer::{PartyShares, SafePrimeSecretSharer, SecretSharer, ShamirSecretSharer},
};
use std::{collections::HashMap, marker::PhantomData};

use super::PrepPublicOutputEqualityShares;

/// Validates the output of a PREP-PUBLIC-OUTPUT-EQUALITY run.
pub struct PrepPublicOutputEqualityValidator<T> {
    _unused: PhantomData<T>,
}

impl<T> Default for PrepPublicOutputEqualityValidator<T> {
    fn default() -> Self {
        Self { _unused: Default::default() }
    }
}

impl<T: SafePrime> PrepPublicOutputEqualityValidator<T> {
    // This deals with splitting the lambda pair shares into two point sequences for the prime and binary extension field
    // parts of the lambda, and another one for the g^lambda one.
    fn generate_split_shares(
        &self,
        party_shares: HashMap<PartyId, PrepPublicOutputEqualityShares<T>>,
    ) -> Result<SplitShares<T>> {
        let mut ran_points = PointSequence::<PrimeField<T>>::default();
        let mut zero_two_t_points = PointSequence::<PrimeField<T>>::default();
        let prime_mapper = PartyMapper::<PrimeField<T>>::new(party_shares.keys().cloned().collect())?;

        for (party_id, share) in party_shares {
            let x = *prime_mapper
                .abscissa(&party_id)
                .ok_or_else(|| anyhow!("failed to find abscissa for party {party_id:?}"))?;

            ran_points.push(Point::new(x, share.ran));
            zero_two_t_points.push(Point::new(x, share.zero_two_t));
        }
        let split_shares = SplitShares { ran_points, zero_two_t_points };
        Ok(split_shares)
    }

    /// Validates the output of a PREP-PUBLIC-OUTPUT-EQUALITY run.
    fn validate_single(&self, party_shares: HashMap<PartyId, PrepPublicOutputEqualityShares<T>>) -> Result<()> {
        // First split the shares into a 3 point sequences, one for each component.
        let SplitShares { ran_points, zero_two_t_points } = self.generate_split_shares(party_shares)?;

        let _ = ran_points.lagrange_interpolate()?;
        let zero_two_t = zero_two_t_points.lagrange_interpolate()?;
        assert_eq!(zero_two_t, ModularNumber::ZERO);
        Ok(())
    }

    /// Validates the output of a PREP-PUBLIC-OUTPUT-EQUALITY run.
    #[allow(clippy::indexing_slicing)]
    pub fn validate(
        &self,
        output_pairs: usize,
        party_shares: HashMap<PartyId, Vec<PrepPublicOutputEqualityShares<T>>>,
    ) -> Result<()> {
        let mut party_single_share = vec![HashMap::default(); output_pairs];
        for (party_id, shares) in party_shares {
            if shares.len() != output_pairs {
                return Err(anyhow!("expected {} shares, got {}", output_pairs, shares.len()));
            }
            for (index, share) in shares.into_iter().enumerate() {
                party_single_share[index].insert(party_id.clone(), share);
            }
        }
        for single_shares in party_single_share {
            self.validate_single(single_shares)?;
        }
        Ok(())
    }
}

struct SplitShares<T: SafePrime> {
    ran_points: PointSequence<PrimeField<T>>,
    zero_two_t_points: PointSequence<PrimeField<T>>,
}

/// Builder that creates PREP-PUBLIC-OUTPUT-EQUALITY shares without running the protocol.
///
/// **This is meant to be used for testing purposes only**.
pub struct PrepPublicOutputEqualitySharesBuilder<'a, R, T: SafePrime> {
    secret_sharer: &'a ShamirSecretSharer<T>,
    rng: R,
}

impl<'a, R, T> PrepPublicOutputEqualitySharesBuilder<'a, R, T>
where
    R: CryptoRngCore,
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Construct a new shares builder.
    pub fn new(secret_sharer: &'a ShamirSecretSharer<T>, rng: R) -> Result<Self, Error> {
        Ok(Self { secret_sharer, rng })
    }

    /// Build `count` PREP-PUBLIC-OUTPUT-EQUALITY shares.
    pub fn build(mut self, count: usize) -> Result<PartyShares<Vec<PrepPublicOutputEqualityShares<T>>>, Error> {
        let mut party_shares: PartyShares<Vec<PrepPublicOutputEqualityShares<T>>> = PartyShares::default();
        for _ in 0..count {
            let shares = self.build_one()?;
            for (party_id, shares) in shares {
                party_shares.entry(party_id).or_default().push(shares);
            }
        }
        Ok(party_shares)
    }

    fn build_one(&mut self) -> Result<PartyShares<PrepPublicOutputEqualityShares<T>>, Error> {
        let random = ModularNumber::<T>::gen_random_with_rng(&mut self.rng);
        let zero = ModularNumber::ZERO;
        self.share_values(random, zero)
    }

    fn share_values(
        &self,
        ran: ModularNumber<T>,
        zero_two_t: ModularNumber<T>,
    ) -> Result<PartyShares<PrepPublicOutputEqualityShares<T>>, Error> {
        let mut ran: PartyShares<ModularNumber<T>> = self.secret_sharer.generate_shares(&ran, PolyDegree::T)?;
        let zero_two_t: PartyShares<ModularNumber<T>> =
            self.secret_sharer.generate_shares(&zero_two_t, PolyDegree::TwoT)?;
        let mut output_shares = PartyShares::default();
        for (party_id, zero_two_t) in zero_two_t {
            let ran = ran.remove(&party_id).ok_or_else(|| anyhow!("{party_id} not found"))?;
            output_shares.insert(party_id, PrepPublicOutputEqualityShares { ran, zero_two_t });
        }
        Ok(output_shares)
    }
}

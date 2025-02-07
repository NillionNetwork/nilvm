//! Validator for the PREP-DIV-INT-SECRET protocol.

// This is only meant to be used for testing so panic'ing is fine.
#![allow(clippy::indexing_slicing, clippy::arithmetic_side_effects, clippy::panic, clippy::expect_used)]
use super::PrepDivisionIntegerSecretShares;
use crate::{
    conditionals::less_than::offline::validation::{PrepCompareSharesBuilder, PrepCompareValidator},
    division::{
        modulo2m_public_divisor::offline::validation::{PrepModulo2mSharesBuilder, PrepModulo2mValidator},
        truncation_probabilistic::offline::validation::{PrepTruncPrSharesBuilder, PrepTruncPrValidator},
    },
};
use anyhow::{anyhow, Error};
use math_lib::modular::{AsBits, ModularNumber, SafePrime};
use shamir_sharing::{
    party::PartyId,
    protocol::PolyDegree,
    secret_sharer::{PartyShares, SafePrimeSecretSharer, SecretSharer, ShamirSecretSharer},
};
use std::{collections::HashMap, f64::consts::SQRT_2, marker::PhantomData};

const ALPHA: f64 = 1.5 - SQRT_2;

/// A validator for the output of the PREP-DIV-INT-SECRET protocol.
pub struct PrepDivisionIntegerSecretValidator<T>(PhantomData<T>);

/// Default implementation for SafePrimes.
/// #[derive(Default)] doesn't work because [`SafePrime`] does not implement [`Default`]
impl<T: SafePrime> Default for PrepDivisionIntegerSecretValidator<T> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<T: SafePrime> PrepDivisionIntegerSecretValidator<T> {
    /// Validates the output of a single PREP-DIV-INT-SECRET run.
    fn validate_single(
        &self,
        kappa: usize,
        k: usize,
        party_shares: HashMap<PartyId, PrepDivisionIntegerSecretShares<T>>,
    ) -> Result<(), Error> {
        let mut prep_comp_hashmap = PartyShares::default();
        let mut prep_truncpr_hashmap = PartyShares::default();
        let mut prep_trunc_hashmap = PartyShares::default();
        let mut prep_bit_decompose_hashmap = PartyShares::default();

        for (party_id, share) in party_shares {
            // Build hashmap for PREP-COMPARE validation.
            prep_comp_hashmap.insert(party_id.clone(), share.prep_compare);
            prep_truncpr_hashmap.insert(party_id.clone(), share.prep_truncpr);
            prep_trunc_hashmap.insert(party_id.clone(), vec![share.prep_trunc]);
            prep_bit_decompose_hashmap.insert(party_id.clone(), vec![share.prep_bit_decompose]);
        }

        let prep_compare_validator = PrepCompareValidator;
        prep_compare_validator.validate(4, prep_comp_hashmap).expect("PREP-COMPARE validation failed");

        let prep_truncpr_validator = PrepTruncPrValidator;
        let batch_size = ((-(k as f64) / 2.0) / ALPHA.log2()).log2().ceil() as usize;
        prep_truncpr_validator
            .validate(batch_size * 2 + 1, kappa, k, prep_truncpr_hashmap)
            .expect("PREP-TRUNCPR validation failed");

        let prep_trunc_validator = PrepModulo2mValidator { _unused: Default::default() };
        prep_trunc_validator.validate(1, kappa, k, prep_trunc_hashmap).expect("PREP-TRUNC validation failed");

        Ok(())
    }

    /// Validate that the provided shares are correct.
    pub fn validate(
        &self,
        output_elements: usize,
        kappa: usize,
        k: usize,
        party_shares: PartyShares<Vec<PrepDivisionIntegerSecretShares<T>>>,
    ) -> Result<(), Error> {
        let mut party_single_share: Vec<HashMap<PartyId, PrepDivisionIntegerSecretShares<T>>> =
            vec![HashMap::default(); output_elements];
        for (party_id, shares) in party_shares {
            if shares.len() != output_elements {
                return Err(anyhow!("expected {} shares, got {}", output_elements, shares.len()));
            }
            for (index, share) in shares.into_iter().enumerate() {
                party_single_share[index].insert(party_id.clone(), share);
            }
        }
        for single_shares in party_single_share {
            self.validate_single(kappa, k, single_shares)?;
        }
        Ok(())
    }
}

/// Builder that creates PREP-DIV-INT-SECRET shares without running PREP-DIV-INT-SECRET.
///
/// **This is meant to be used for testing purposes only**.
pub struct PrepDivisionIntegerSecretSharesBuilder<'a, T: SafePrime> {
    secret_sharer: &'a ShamirSecretSharer<T>,
    k: usize,
    kappa: usize,
}

impl<'a, T> PrepDivisionIntegerSecretSharesBuilder<'a, T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Construct a new shares builder.
    pub fn new(secret_sharer: &'a ShamirSecretSharer<T>, k: usize, kappa: usize) -> Result<Self, Error> {
        Ok(Self { secret_sharer, k, kappa })
    }

    /// Build `count` PREP-DIV-INT-SECRET shares.
    pub fn build(mut self, count: usize) -> Result<PartyShares<Vec<PrepDivisionIntegerSecretShares<T>>>, Error> {
        let mut party_shares: PartyShares<Vec<PrepDivisionIntegerSecretShares<T>>> = PartyShares::default();
        for _ in 0..count {
            let shares = self.build_one()?;
            for (party_id, shares) in shares {
                party_shares.entry(party_id).or_default().push(shares);
            }
        }
        Ok(party_shares)
    }

    fn build_one(&mut self) -> Result<PartyShares<PrepDivisionIntegerSecretShares<T>>, Error> {
        let prep_compare_builder = PrepCompareSharesBuilder::new(self.secret_sharer, rand::thread_rng())?;
        let prep_compare = prep_compare_builder.build(4).expect("failed to build PREP-COMPARE shares");

        let prep_truncpr_builder = PrepTruncPrSharesBuilder::new(self.secret_sharer, self.k, self.kappa)?;
        let batch_size = (((-(self.k as f64) / 2.0) / ALPHA.log2()).log2().ceil() * 2.0 + 1.0) as usize;
        let mut prep_truncpr = prep_truncpr_builder.build(batch_size).expect("failed to build PREP-TRUNCPR shares");

        let prep_trunc_builder = PrepModulo2mSharesBuilder::new(self.secret_sharer, self.k, self.kappa)?;
        let mut prep_trunc = prep_trunc_builder.build(1).expect("failed to build PREP-TRUNC shares");

        let mut rng = rand::thread_rng();
        let secret = ModularNumber::<T>::gen_random_with_rng(&mut rng);
        let secret = secret.into_value();
        let mut prep_bit_decompose: PartyShares<Vec<ModularNumber<T>>> = PartyShares::default();
        for i in 0..T::MODULO.bits() {
            let bit = ModularNumber::from_u32(secret.bit(i) as u32);
            let shares = self.secret_sharer.generate_shares(&bit, PolyDegree::T)?;
            for (party_id, share) in shares.into_iter() {
                prep_bit_decompose.entry(party_id.clone()).or_default().push(share);
            }
        }

        let mut output_shares = PartyShares::default();
        for (party_id, prep_compare_share) in prep_compare {
            let prep_truncpr = prep_truncpr.remove(&party_id).ok_or_else(|| anyhow!("{party_id} not found"))?;
            let prep_trunc = prep_trunc.remove(&party_id).ok_or_else(|| anyhow!("{party_id} not found"))?;
            let prep_bit_decompose =
                prep_bit_decompose.remove(&party_id).ok_or_else(|| anyhow!("{party_id} not found"))?;
            output_shares.insert(
                party_id,
                PrepDivisionIntegerSecretShares {
                    prep_compare: prep_compare_share,
                    prep_truncpr,
                    prep_trunc: prep_trunc[0].clone(),
                    prep_bit_decompose: prep_bit_decompose.into(),
                },
            );
        }
        Ok(output_shares)
    }
}

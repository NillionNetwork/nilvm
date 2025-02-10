//! Validator for the PREP-MODULO protocol.

// This is only meant to be used for testing so panic'ing is fine.
#![allow(clippy::indexing_slicing, clippy::arithmetic_side_effects, clippy::panic, clippy::expect_used)]
use super::super::offline::{state::PrepModulo2mCreateError, PrepModulo2mShares};
use crate::{
    conditionals::less_than::offline::validation::{PrepCompareSharesBuilder, PrepCompareValidator},
    random::{random_bit::BitShare, random_bitwise::BitwiseNumberShares},
};
use anyhow::{anyhow, Error};
use math_lib::{
    fields::PrimeField,
    modular::{ModularNumber, SafePrime},
    polynomial::{point::Point, point_sequence::PointSequence},
};
use rand::Rng;
use shamir_sharing::{
    party::{PartyId, PartyMapper},
    protocol::PolyDegree,
    secret_sharer::{PartyShares, SafePrimeSecretSharer, SecretSharer, ShamirSecretSharer},
};
use std::{collections::HashMap, marker::PhantomData};

/// A validator for the output of the PREP-MODULO protocol.
#[derive(Default)]
pub struct PrepModulo2mValidator<T> {
    /// Phantom Data.
    pub _unused: PhantomData<T>,
}

impl<T: SafePrime> PrepModulo2mValidator<T> {
    /// Validates the output of a single PREP-MODULO run.
    fn validate_single(
        &self,
        kappa: usize,
        k: usize,
        party_shares: HashMap<PartyId, PrepModulo2mShares<T>>,
    ) -> Result<(), Error> {
        let mapper = PartyMapper::<PrimeField<T>>::new(party_shares.keys().cloned().collect())?;
        let kappa_plus_k = kappa.checked_add(k).ok_or(PrepModulo2mCreateError::IntegerOverflow)?;
        let mut bit_point_sequences = vec![PointSequence::<PrimeField<T>>::default(); kappa_plus_k];
        let mut prep_comp_hashmap = PartyShares::default();

        for (party_id, share) in party_shares {
            // Build hashmap for PREP-COMPARE validation.
            prep_comp_hashmap.insert(party_id.clone(), share.prep_compare);

            let x =
                *mapper.abscissa(&party_id).ok_or_else(|| anyhow!("failed to find abscissa for party {party_id:?}"))?;
            for (index, bit_share) in share.ran_bits_r.shares().iter().enumerate() {
                bit_point_sequences[index].push(Point::new(x, ModularNumber::from(bit_share.clone())));
            }
        }

        for bit_point_sequence in bit_point_sequences {
            bit_point_sequence.lagrange_interpolate().expect("interpolation failed");
        }

        let validator = PrepCompareValidator;
        validator.validate(1, prep_comp_hashmap).expect("validation failed");

        Ok(())
    }

    /// Validate that the provided shares are correct.
    pub fn validate(
        &self,
        output_elements: usize,
        kappa: usize,
        k: usize,
        party_shares: PartyShares<Vec<PrepModulo2mShares<T>>>,
    ) -> Result<(), Error> {
        let mut party_single_share: Vec<HashMap<PartyId, PrepModulo2mShares<T>>> =
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

/// Builder that creates PREP-MOD2M shares without running PREP-MOD2M.
///
/// **This is meant to be used for testing purposes only**.
pub struct PrepModulo2mSharesBuilder<'a, T: SafePrime> {
    secret_sharer: &'a ShamirSecretSharer<T>,
    k: usize,
    kappa: usize,
}

impl<'a, T> PrepModulo2mSharesBuilder<'a, T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Construct a new shares builder.
    pub fn new(secret_sharer: &'a ShamirSecretSharer<T>, k: usize, kappa: usize) -> Result<Self, Error> {
        Ok(Self { secret_sharer, k, kappa })
    }

    /// Build `count` PREP-MOD2M shares.
    pub fn build(mut self, count: usize) -> Result<PartyShares<Vec<PrepModulo2mShares<T>>>, Error> {
        let mut party_shares: PartyShares<Vec<PrepModulo2mShares<T>>> = PartyShares::default();
        for _ in 0..count {
            let shares = self.build_one()?;
            for (party_id, shares) in shares {
                party_shares.entry(party_id).or_default().push(shares);
            }
        }
        Ok(party_shares)
    }

    fn build_one(&mut self) -> Result<PartyShares<PrepModulo2mShares<T>>, Error> {
        // Generate a random vectors with ones and zeros.
        let num_elements = self.k + self.kappa;
        let mut rng = rand::thread_rng();
        let mut ran_bits_r: Vec<ModularNumber<T>> = Vec::new();
        for _ in 0..num_elements {
            let r_bit = if rng.gen_bool(0.5) { ModularNumber::ZERO } else { ModularNumber::ONE };
            ran_bits_r.push(r_bit);
        }
        self.share_values(ran_bits_r)
    }

    #[allow(clippy::too_many_arguments)]
    fn share_values(&self, ran_bits_r: Vec<ModularNumber<T>>) -> Result<PartyShares<PrepModulo2mShares<T>>, Error> {
        let builder = PrepCompareSharesBuilder::new(self.secret_sharer, rand::thread_rng())?;
        let mut prep_compare = builder.build(1).expect("failed to build PREP-COMPARE shares");

        let ran_bits_r: PartyShares<Vec<ModularNumber<T>>> =
            self.secret_sharer.generate_shares(&ran_bits_r, PolyDegree::T)?;

        let mut output_shares = PartyShares::default();

        for (party_id, ran_bits_r) in ran_bits_r {
            let ran_bits_r = BitwiseNumberShares::from(ran_bits_r.into_iter().map(BitShare::from).collect::<Vec<_>>());
            let prep_compare = prep_compare.remove(&party_id).ok_or_else(|| anyhow!("{party_id} not found"))?;

            output_shares.insert(party_id, PrepModulo2mShares { ran_bits_r, prep_compare });
        }
        Ok(output_shares)
    }
}

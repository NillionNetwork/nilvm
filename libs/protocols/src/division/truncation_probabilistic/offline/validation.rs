//! Validator for the PREP-TRUNCPR protocol.

// This is only meant to be used for testing so panic'ing is fine.
#![allow(clippy::indexing_slicing, clippy::arithmetic_side_effects, clippy::panic)]
use super::super::offline::{state::PrepTruncPrCreateError, PrepTruncPrShares};
use crate::random::{random_bit::BitShare, random_bitwise::BitwiseNumberShares};
use anyhow::{anyhow, Error};
use math_lib::{
    fields::PrimeField,
    modular::{ModularNumber, SafePrime},
    polynomial::{point::Point, point_sequence::PointSequence},
};
use rand::Rng;
use shamir_sharing::{
    party::PartyMapper,
    protocol::PolyDegree,
    secret_sharer::{PartyShares, SafePrimeSecretSharer, SecretSharer, ShamirSecretSharer},
};

/// A validator for the output of the PREP-TRUNCPR protocol.
#[derive(Default)]
pub struct PrepTruncPrValidator;

impl PrepTruncPrValidator {
    /// Validates the output of a single PREP-TRUNCPR run.
    fn validate_single<T: SafePrime>(
        &self,
        kappa: usize,
        k: usize,
        party_shares: PartyShares<PrepTruncPrShares<T>>,
    ) -> Result<(), Error> {
        let mapper = PartyMapper::<PrimeField<T>>::new(party_shares.keys().cloned().collect())?;
        let kappa_plus_k = kappa.checked_add(k).ok_or(PrepTruncPrCreateError::IntegerOverflow)?;
        let mut bit_point_sequences = vec![PointSequence::<PrimeField<T>>::default(); kappa_plus_k];

        for (party_id, share) in party_shares {
            let x =
                *mapper.abscissa(&party_id).ok_or_else(|| anyhow!("failed to find abscissa for party {party_id:?}"))?;
            for (index, bit_share) in share.ran_bits_r.shares().iter().enumerate() {
                bit_point_sequences[index].push(Point::new(x, ModularNumber::from(bit_share.clone())));
            }
        }

        for bit_point_sequence in bit_point_sequences {
            bit_point_sequence.lagrange_interpolate()?;
        }

        Ok(())
    }

    /// Validate that the provided shares are correct.
    pub fn validate<T: SafePrime>(
        &self,
        output_elements: usize,
        kappa: usize,
        k: usize,
        party_shares: PartyShares<Vec<PrepTruncPrShares<T>>>,
    ) -> Result<(), Error> {
        let mut party_single_share: Vec<PartyShares<PrepTruncPrShares<T>>> =
            vec![PartyShares::default(); output_elements];
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

/// Builder that creates PREP-TRUNCPR shares without running PREP-TRUNCPR.
///
/// **This is meant to be used for testing purposes only**.
pub struct PrepTruncPrSharesBuilder<'a, T: SafePrime> {
    secret_sharer: &'a ShamirSecretSharer<T>,
    k: usize,
    kappa: usize,
}

impl<'a, T> PrepTruncPrSharesBuilder<'a, T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Construct a new shares builder.
    pub fn new(secret_sharer: &'a ShamirSecretSharer<T>, k: usize, kappa: usize) -> Result<Self, Error> {
        Ok(Self { secret_sharer, k, kappa })
    }

    /// Build `count` PREP-TRUNCPR shares.
    pub fn build(mut self, count: usize) -> Result<PartyShares<Vec<PrepTruncPrShares<T>>>, Error> {
        let mut party_shares: PartyShares<Vec<PrepTruncPrShares<T>>> = PartyShares::default();
        for _ in 0..count {
            let shares = self.build_one()?;
            for (party_id, shares) in shares {
                party_shares.entry(party_id).or_default().push(shares);
            }
        }
        Ok(party_shares)
    }

    fn build_one(&mut self) -> Result<PartyShares<PrepTruncPrShares<T>>, Error> {
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
    fn share_values(&self, ran_bits_r: Vec<ModularNumber<T>>) -> Result<PartyShares<PrepTruncPrShares<T>>, Error> {
        let ran_bits_r: PartyShares<Vec<ModularNumber<T>>> =
            self.secret_sharer.generate_shares(&ran_bits_r, PolyDegree::T)?;

        let mut output_shares = PartyShares::default();

        for (party_id, ran_bits_r) in ran_bits_r {
            let ran_bits_r = BitwiseNumberShares::from(ran_bits_r.into_iter().map(BitShare::from).collect::<Vec<_>>());

            output_shares.insert(party_id, PrepTruncPrShares { ran_bits_r });
        }
        Ok(output_shares)
    }
}

//! Validator for the PREP Private Output Equality protocol.

// This is only meant to be used for testing so panic'ing is fine.
#![allow(clippy::indexing_slicing, clippy::arithmetic_side_effects, clippy::panic, clippy::expect_used)]
use super::output::PrepPrivateOutputEqualityShares;
use crate::{
    conditionals::poly_eval::offline::{
        output::PrepPolyEvalShares,
        validation::{PrepPolyEvalBuilder, PrepPolyEvalValidator},
    },
    random::{random_bit::BitShare, random_bitwise::BitwiseNumberShares},
};
use anyhow::{anyhow, Error, Ok};
use math_lib::{
    decoders::lagrange_polynomial,
    fields::PrimeField,
    modular::{AsBits, CryptoRngCore, Integer, ModularNumber, SafePrime},
    polynomial::{point::Point, point_sequence::PointSequence, Polynomial},
};
use shamir_sharing::{
    party::PartyMapper,
    protocol::PolyDegree,
    secret_sharer::{PartyShares, SafePrimeSecretSharer, SecretSharer, ShamirSecretSharer},
};

use crate::conditionals::equality::POLY_EVAL_DEGREE;
use shamir_sharing::party::PartyId;
use std::collections::HashMap;

/// A validator for the output of the PrepPrivateOutputEquality protocol.
pub struct PrepPrivateOutputEqualityValidator<T> {
    _phantom: std::marker::PhantomData<T>,
}

impl<T> Default for PrepPrivateOutputEqualityValidator<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T> PrepPrivateOutputEqualityValidator<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Creates a new Private Output Equality Validator.
    pub fn new() -> Self {
        Self { _phantom: Default::default() }
    }
    fn validate_prep_poly_eval(
        element_count: usize,
        party_shares: PartyShares<Vec<PrepPrivateOutputEqualityShares<T>>>,
        polynomial_degree: u64,
        poly_eval_degree: u64,
    ) -> Result<(), Error> {
        let prep_poly_eval: PartyShares<Vec<PrepPolyEvalShares<T>>> = party_shares
            .iter()
            .map(|(party_id, vec)| {
                let prep_poly_eval = vec
                    .iter()
                    .map(|prep_private_equality_output| prep_private_equality_output.prep_poly_eval.clone())
                    .collect();
                (party_id.clone(), prep_poly_eval)
            })
            .collect();

        let prep_poly_eval_validator = PrepPolyEvalValidator;
        prep_poly_eval_validator.validate(element_count, prep_poly_eval, polynomial_degree, poly_eval_degree)?;
        Ok(())
    }

    /// Validates the validity of bitwise shares
    fn validate_bitwise_shares(
        element_count: usize,
        party_shares: PartyShares<Vec<PrepPrivateOutputEqualityShares<T>>>,
    ) -> Result<(), Error> {
        let party_shares: PartyShares<Vec<BitwiseNumberShares<T>>> = party_shares
            .iter()
            .map(|(party_id, vec)| {
                let prep_poly_eval = vec
                    .iter()
                    .map(|prep_private_equality_output| prep_private_equality_output.bitwise_number_shares.clone())
                    .collect();
                (party_id.clone(), prep_poly_eval)
            })
            .collect();
        Self::validate_bitwise_shares_internal(element_count, party_shares)?;
        Ok(())
    }

    /// Validates the validity of bitwise shares
    fn validate_bitwise_shares_internal(
        element_count: usize,
        party_shares: PartyShares<Vec<BitwiseNumberShares<T>>>,
    ) -> Result<(), Error> {
        let prime_bits = T::Normal::BITS;
        let mapper = PartyMapper::<PrimeField<T>>::new(party_shares.keys().cloned().collect())?;

        // We have N vecs one per element count, which has M vecs one per bit, which is a point
        // sequence with a point per party.
        let mut point_sequences = vec![vec![PointSequence::<PrimeField<T>>::default(); prime_bits]; element_count];
        for (party_id, party_shares) in party_shares {
            if party_shares.len() != element_count {
                return Err(anyhow!("unexpected element share count"));
            }

            let x =
                *mapper.abscissa(&party_id).ok_or_else(|| anyhow!("failed to find abscissa for party {party_id:?}"))?;
            for (element_index, element_shares) in party_shares.into_iter().enumerate() {
                let element_shares = Vec::from(element_shares);
                if element_shares.len() != prime_bits {
                    return Err(anyhow!("unexpected bit share count"));
                }
                for (index, share) in element_shares.into_iter().enumerate() {
                    point_sequences[element_index][index].push(Point::new(x, ModularNumber::from(share)));
                }
            }
        }

        for number_sequences in point_sequences {
            for point_sequence in number_sequences {
                // We can't really check anything here besides that interpolation doesn't fail.
                point_sequence.lagrange_interpolate().expect("interpolation failed");
            }
        }

        Ok(())
    }

    /// Validates lagrange polynomial for all users
    fn validate_lagrange_polynomial(
        party_shares: PartyShares<Vec<PrepPrivateOutputEqualityShares<T>>>,
        poly_eval_degree: u64,
    ) -> Result<(), Error> {
        let party_shares: HashMap<PartyId, Vec<Polynomial<PrimeField<T>>>> = party_shares
            .iter()
            .map(|(party_id, vec)| {
                let prep_poly_eval = vec
                    .iter()
                    .map(|prep_private_equality_output| prep_private_equality_output.lagrange_polynomial.clone())
                    .collect();
                (party_id.clone(), prep_poly_eval)
            })
            .collect();
        for (_, polynomials) in party_shares.iter() {
            for polynomial in polynomials.iter() {
                for x in 0..poly_eval_degree + 1 {
                    let num = if x == 1 { ModularNumber::ONE } else { ModularNumber::ZERO };
                    let result = polynomial.eval(&ModularNumber::from_u64(x))?;
                    assert_eq!(result, num, "Lagrange polynomial failed for x = {x}");
                }
            }
        }
        Ok(())
    }

    /// Validates the output of PrepPrivateOutputEquality run for all parties.
    pub fn validate(
        &self,
        element_count: usize,
        party_shares: PartyShares<Vec<PrepPrivateOutputEqualityShares<T>>>,
        polynomial_degree: u64,
        poly_eval_degree: u64,
    ) -> Result<(), Error> {
        let mut parties: Vec<PartyId> = party_shares.keys().cloned().collect();
        parties.sort();
        Self::validate_prep_poly_eval(element_count, party_shares.clone(), polynomial_degree, poly_eval_degree)?;
        Self::validate_bitwise_shares(element_count, party_shares.clone())?;
        Self::validate_lagrange_polynomial(party_shares.clone(), poly_eval_degree)?;
        Ok(())
    }
}

/// Builder that creates PREP PRIVATE OUTPUT EQUALITY shares without running PREP PRIVATE OUTPUT EQUALITY.
///
/// **This is meant to be used for testing purposes only**.
pub struct PrepPrivateOutputEqualitySharesBuilder<'a, R, T: SafePrime> {
    secret_sharer: &'a ShamirSecretSharer<T>,
    rng: R,
    poly_eval_degree: u64,
}

impl<'a, R, T> PrepPrivateOutputEqualitySharesBuilder<'a, R, T>
where
    R: CryptoRngCore + Clone,
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Construct a new shares builder.
    pub fn new(secret_sharer: &'a ShamirSecretSharer<T>, rng: R) -> Result<Self, Error> {
        Ok(Self { secret_sharer, rng, poly_eval_degree: POLY_EVAL_DEGREE })
    }

    /// Build `count` PREP PRIVATE OUTPUT EQUALITY shares.
    pub fn build(mut self, count: usize) -> Result<PartyShares<Vec<PrepPrivateOutputEqualityShares<T>>>, Error> {
        let mut party_shares: PartyShares<Vec<PrepPrivateOutputEqualityShares<T>>> = PartyShares::default();

        for _ in 0..count {
            let shares = self.build_one(self.poly_eval_degree)?;
            for (party_id, shares) in shares {
                party_shares.entry(party_id).or_default().push(shares);
            }
        }
        Ok(party_shares)
    }

    fn build_one(&mut self, poly_eval_degree: u64) -> Result<PartyShares<PrepPrivateOutputEqualityShares<T>>, Error> {
        let builder = PrepPolyEvalBuilder::new(self.secret_sharer, self.rng.clone())?;
        let prep_poly_eval_build = builder.build(1, poly_eval_degree)?;
        let prep_poly_eval: HashMap<PartyId, PrepPolyEvalShares<T>> =
            prep_poly_eval_build.into_iter().map(|(party_id, vec)| (party_id, vec[0].clone())).collect();
        let bitwise_shares = self.generate_bitwise_shares();
        // Build lagrange polynomial
        let polynomial = Self::build_lagrange_polynomial(poly_eval_degree)?;

        Ok(prep_poly_eval
            .into_iter()
            .map(|(party_id, prep_poly_eval)| {
                let bitwise = bitwise_shares.get(&party_id).expect("missing party");
                (
                    party_id,
                    PrepPrivateOutputEqualityShares {
                        prep_poly_eval,
                        bitwise_number_shares: bitwise.clone(),
                        lagrange_polynomial: polynomial.clone(),
                    },
                )
            })
            .collect())
    }

    fn generate_bitwise_shares(&mut self) -> HashMap<PartyId, BitwiseNumberShares<T>> {
        // Generate a vector of random bit shares.
        let num_bit_shares = T::Normal::BITS;
        let s = ModularNumber::<T>::gen_random_with_rng(&mut self.rng);
        let mut bit_shares = HashMap::<PartyId, Vec<BitShare<T>>>::new();
        for i in 0..num_bit_shares {
            let bit = if s.into_value().bit(i) { ModularNumber::<T>::ONE } else { ModularNumber::<T>::ZERO };
            let shares: PartyShares<ModularNumber<T>> = self
                .secret_sharer
                .generate_shares(&bit, PolyDegree::T)
                .expect("Secret sharer could not generate shares");
            let reconstructed =
                self.secret_sharer.recover(shares.clone()).expect("Secret sharer could not recover shares");
            assert_eq!(reconstructed, bit);
            for (party_id, share) in shares {
                bit_shares.entry(party_id).or_default().push(BitShare::from(share));
            }
        }
        bit_shares
            .into_iter()
            .map(|(party_id, shares)| (party_id, BitwiseNumberShares::from(shares)))
            .collect::<HashMap<PartyId, BitwiseNumberShares<T>>>()
    }

    fn build_lagrange_polynomial(poly_eval_degree: u64) -> Result<Polynomial<PrimeField<T>>, Error> {
        let mut point_sequence = PointSequence::<PrimeField<T>>::default();
        for x in 0..poly_eval_degree + 1 {
            let num = if x == 1 { ModularNumber::ONE } else { ModularNumber::ZERO };
            point_sequence.push(Point::new(x.into(), num));
        }

        let lagrange_poly =
            lagrange_polynomial(&point_sequence).map_err(|e| anyhow!("Lagrange Interpolation Error: {e}"))?;
        Ok(lagrange_poly)
    }
}

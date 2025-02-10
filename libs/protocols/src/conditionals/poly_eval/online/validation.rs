//! Validator for the PolyEval protocol.

// This is only meant to be used for testing so panic'ing is fine.
#![allow(clippy::indexing_slicing, clippy::arithmetic_side_effects, clippy::panic, clippy::expect_used)]
use anyhow::{anyhow, Error};
use math_lib::{
    fields::PrimeField,
    modular::{ModularNumber, SafePrime},
    polynomial::Polynomial,
};
use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, SecretSharer, ShamirSecretSharer};

use super::output::PolyEvalShares;
use shamir_sharing::party::PartyId;
use std::collections::HashMap;
/// A validator for the output of the PolyEval protocol.
pub struct PolyEvalValidator;

impl PolyEvalValidator {
    /// Validates the output of PolyEval run for all parties.
    pub fn validate<T>(
        &self,
        party_shares: HashMap<PartyId, Vec<PolyEvalShares<T>>>,
        polynomial_degree: u64,
        polynomials: Vec<Polynomial<PrimeField<T>>>,
        x: Vec<ModularNumber<T>>,
    ) -> Result<(), Error>
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        let mut parties: Vec<PartyId> = party_shares.keys().cloned().collect();
        parties.sort();

        let element_count = polynomials.len();
        let secret_sharer: ShamirSecretSharer<T> =
            ShamirSecretSharer::<T>::new(parties[0].clone(), polynomial_degree, parties.clone())?;

        let mut poly_reconstruct = Vec::new();
        for i in 0..element_count {
            let mut poly_shares_i: HashMap<PartyId, ModularNumber<T>> = HashMap::new();

            for (party_id, vec) in party_shares.iter() {
                let PolyEvalShares { poly_x } = vec.get(i).ok_or(anyhow!("Invalid index"))?;

                poly_shares_i.insert(party_id.clone(), *poly_x);
            }

            let secret_poly_x = secret_sharer.recover(poly_shares_i)?;

            poly_reconstruct.push(secret_poly_x);
        }

        // evaluate the polynomials at the given x values
        let expected_outputs = polynomials
            .iter()
            .zip(x.iter())
            .map(|(poly, x)| poly.eval(x))
            .collect::<Result<Vec<ModularNumber<T>>, _>>()?;

        assert_eq!(
            poly_reconstruct.len(),
            expected_outputs.len(),
            "Different lenghts of reconstructed polynomials and expected outputs"
        );
        // validate that the reconstructed polynomials are the same as the expected outputs
        for (reconstructed, expected) in poly_reconstruct.iter().zip(expected_outputs.iter()) {
            assert_eq!(reconstructed, expected);
        }

        Ok(())
    }
}

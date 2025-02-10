//! Validator for the PrivateOutputEquality protocol.

// This is only meant to be used for testing so panic'ing is fine.
#![allow(clippy::indexing_slicing, clippy::arithmetic_side_effects, clippy::panic, clippy::expect_used)]
use anyhow::{anyhow, Error};
use math_lib::modular::{ModularNumber, SafePrime};
use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, SecretSharer, ShamirSecretSharer};

use super::output::PrivateOutputEqualityShares;
use shamir_sharing::party::PartyId;
use std::collections::HashMap;
/// A validator for the output of the PrivateOutputEquality protocol.
pub struct PrivateOutputEqualityValidator;

impl PrivateOutputEqualityValidator {
    /// Validates the output of PrivateOutputEquality run for all parties.
    pub fn validate<T>(
        &self,
        party_shares: HashMap<PartyId, Vec<PrivateOutputEqualityShares<T>>>,
        polynomial_degree: u64,
        x: Vec<ModularNumber<T>>,
        y: Vec<ModularNumber<T>>,
    ) -> Result<(), Error>
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        let expected_outputs = x.iter().zip(y.iter()).map(|(x, y)| x == y).collect::<Vec<bool>>();
        let mut parties: Vec<PartyId> = party_shares.keys().cloned().collect();
        parties.sort();

        let element_count = x.len();

        assert_eq!(x.len(), y.len(), "x and y must have the same length");

        let secret_sharer: ShamirSecretSharer<T> =
            ShamirSecretSharer::<T>::new(parties[0].clone(), polynomial_degree, parties.clone())?;

        let mut equality_reconstruct = Vec::new();
        for i in 0..element_count {
            let mut equality_shares_i: HashMap<PartyId, ModularNumber<T>> = HashMap::new();

            for (party_id, vec) in party_shares.iter() {
                let PrivateOutputEqualityShares { equality_output } = vec.get(i).ok_or(anyhow!("Invalid index"))?;

                equality_shares_i.insert(party_id.clone(), *equality_output);
            }

            let secret_equality_output = secret_sharer.recover(equality_shares_i)?;

            equality_reconstruct.push(secret_equality_output);
        }

        assert_eq!(equality_reconstruct.len(), expected_outputs.len(), "Not the same number of expected outputs");

        // validate that the reconstructed polynomials are the same as the expected outputs
        for (reconstructed, expected) in equality_reconstruct.iter().zip(expected_outputs.iter()) {
            assert_eq!(
                reconstructed,
                &ModularNumber::<T>::from_u32(*expected as u32),
                "Reconstructed output does not match expected output for equality check"
            );
        }
        Ok(())
    }
}

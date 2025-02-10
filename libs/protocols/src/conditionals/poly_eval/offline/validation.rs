//! Validator for the PrepPolyEval protocol.

// This is only meant to be used for testing so panic'ing is fine.
#![allow(clippy::indexing_slicing, clippy::arithmetic_side_effects, clippy::panic, clippy::expect_used)]
use anyhow::{anyhow, Error, Ok};
use math_lib::{
    fields::Inv,
    modular::{CryptoRngCore, ModularNumber, SafePrime},
};
use num_bigint::BigUint;
use shamir_sharing::secret_sharer::{PartyShares, SafePrimeSecretSharer, SecretSharer, ShamirSecretSharer};

use super::output::PrepPolyEvalShares;
use shamir_sharing::{party::PartyId, protocol::PolyDegree};
use std::collections::HashMap;

/// A validator for the output of the PrepPolyEval protocol.
pub struct PrepPolyEvalValidator;

impl PrepPolyEvalValidator {
    /// Validates the output of PrepPolyEval run for all parties.
    pub fn validate<T>(
        &self,
        element_count: usize,
        party_shares: PartyShares<Vec<PrepPolyEvalShares<T>>>,
        polynomial_degree: u64,
        poly_eval_degree: u64,
    ) -> Result<(), Error>
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        let mut parties: Vec<PartyId> = party_shares.keys().cloned().collect();
        parties.sort();

        let secret_sharer: ShamirSecretSharer<T> =
            ShamirSecretSharer::<T>::new(parties[0].clone(), polynomial_degree, parties.clone())?;

        let mut power_reconstruct: Vec<Vec<ModularNumber<T>>> = Vec::new();
        let mut zero_reconstruct = Vec::new();
        let mut invertible_reconstruct = Vec::new();

        for i in 0..element_count {
            let mut zero_shares_i: HashMap<PartyId, ModularNumber<T>> = HashMap::new();
            let mut power_shares_i: Vec<HashMap<PartyId, ModularNumber<T>>> =
                vec![HashMap::new(); (poly_eval_degree + 1) as usize];
            let mut invertible_numbers_i: HashMap<PartyId, ModularNumber<T>> = HashMap::new();

            for (party_id, vec) in party_shares.iter() {
                let PrepPolyEvalShares { invertible_number, powers, zero_share } =
                    vec.get(i).ok_or(anyhow!("Invalid index"))?;

                zero_shares_i.insert(party_id.clone(), *zero_share);
                invertible_numbers_i.insert(party_id.clone(), *invertible_number);

                for (mod_num, power_shares_i_j) in powers.iter().zip(power_shares_i.iter_mut()) {
                    power_shares_i_j.insert(party_id.clone(), *mod_num);
                }
            }

            let secret_zero = secret_sharer.recover(zero_shares_i)?;
            let secret_invertible = secret_sharer.recover(invertible_numbers_i)?;

            let mut secret_powers = Vec::new();
            for power_share_i_j in power_shares_i.into_iter() {
                let secret_power = secret_sharer.recover(power_share_i_j)?;
                secret_powers.push(secret_power);
            }

            zero_reconstruct.push(BigUint::from(&secret_zero));
            invertible_reconstruct.push(secret_invertible);
            power_reconstruct.push(secret_powers);
        }

        // Check that zero_reconstruct is all 0s
        for zero in zero_reconstruct.iter() {
            if zero != &BigUint::from(0u64) {
                return Err(anyhow!("zero share is not 0"));
            }
        }

        // Check that invertible_reconstruct multiplied by power_reconstruct[1] is 1
        for (inv_rec, powers_reconstruct) in invertible_reconstruct.iter().zip(power_reconstruct.iter()) {
            let power_1_i = &powers_reconstruct[1];
            let check = inv_rec * power_1_i;
            if BigUint::from(1u64) != BigUint::from(&check) {
                return Err(anyhow!("invertible share is not correct"));
            }
        }

        // Check that power_reconstruct equals the previous * power_reconstruct[1]
        for powers in power_reconstruct.iter() {
            let power_0 = powers[1];
            for (prev_power, power) in powers.iter().skip(1).zip(powers.iter().skip(2)) {
                if power != &(prev_power * &power_0) {
                    return Err(anyhow!("Power share is not correct"));
                }
            }
        }
        Ok(())
    }
}

/// Builder that creates PREP POLY EVAL shares without running PREP POLY EVAL.
///
/// **This is meant to be used for testing purposes only**.
pub struct PrepPolyEvalBuilder<'a, R, T: SafePrime> {
    secret_sharer: &'a ShamirSecretSharer<T>,
    rng: R,
}

impl<'a, R, T> PrepPolyEvalBuilder<'a, R, T>
where
    R: CryptoRngCore,
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Construct a new shares builder.
    pub fn new(secret_sharer: &'a ShamirSecretSharer<T>, rng: R) -> Result<Self, Error> {
        Ok(Self { secret_sharer, rng })
    }

    /// Build `count` PREP POLY EVAL shares.
    pub fn build(
        mut self,
        count: usize,
        eval_polynomial_degree: u64,
    ) -> Result<PartyShares<Vec<PrepPolyEvalShares<T>>>, Error> {
        let mut party_shares: PartyShares<Vec<PrepPolyEvalShares<T>>> = PartyShares::default();

        for _ in 0..count {
            let shares = self.build_one(eval_polynomial_degree)?;
            for (party_id, shares) in shares {
                party_shares.entry(party_id).or_default().push(shares);
            }
        }
        Ok(party_shares)
    }

    fn build_one(&mut self, eval_polynomial_degree: u64) -> Result<PartyShares<PrepPolyEvalShares<T>>, Error> {
        let mut shares: PartyShares<PrepPolyEvalShares<T>> = PartyShares::default();

        let (r, r_inv) = self.inv_rand()?;

        let powers: Vec<ModularNumber<T>> = (0..=eval_polynomial_degree)
            .scan(ModularNumber::ONE, |state, _| {
                let result = *state;
                *state = *state * &r;
                Some(result)
            })
            .collect();

        let zero_shares = self.share_modular_number_2_t(ModularNumber::ZERO);
        let r_inv_shares = self.share_modular_number(r_inv);
        let power_shares: Vec<PartyShares<ModularNumber<T>>> =
            powers.into_iter().map(|p| self.share_modular_number(p)).collect();

        for (party_id, zero_share) in zero_shares {
            let invertible_number = r_inv_shares.get(&party_id).expect("Missing invertible number");
            let powers = power_shares.iter().filter_map(|p| p.get(&party_id).cloned()).collect();
            let output = PrepPolyEvalShares { invertible_number: *invertible_number, powers, zero_share };
            shares.insert(party_id, output);
        }

        Ok(shares)
    }

    fn share_modular_number(&mut self, value: ModularNumber<T>) -> PartyShares<ModularNumber<T>> {
        self.secret_sharer.generate_shares(&value, PolyDegree::T).expect("Could not create modular number")
    }

    fn share_modular_number_2_t(&mut self, value: ModularNumber<T>) -> PartyShares<ModularNumber<T>> {
        self.secret_sharer.generate_shares(&value, PolyDegree::TwoT).expect("Could not create modular number in 2T")
    }

    fn inv_rand(&mut self) -> Result<(ModularNumber<T>, ModularNumber<T>), Error> {
        let mut value = ModularNumber::gen_random_with_rng(&mut self.rng);
        while value == ModularNumber::ONE {
            value = ModularNumber::gen_random_with_rng(&mut self.rng);
        }
        let inverse = value.inv()?;
        Ok((value, inverse))
    }
}

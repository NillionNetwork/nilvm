//! End-to-end tests for the MULTIPLICATION-AND-TRUNCATION protocol.

#![allow(clippy::arithmetic_side_effects, clippy::indexing_slicing)]

use super::protocol::MultTruncProtocol;
use crate::simulator::symmetric::SymmetricProtocolSimulator;
use anyhow::{anyhow, Error};
use math_lib::{
    fields::PrimeField,
    modular::{ModularNumber, SafePrime, U64SafePrime},
    polynomial::{point::Point, point_sequence::PointSequence},
};
use num_bigint::BigInt;
use shamir_sharing::{
    party::PartyMapper,
    secret_sharer::{PartyShares, SafePrimeSecretSharer, ShamirSecretSharer},
};

impl<T> MultTruncProtocol<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Validates the output of MULTIPLICATION-AND-TRUNCATION protocol.
    pub fn validate_output(&self, party_shares: PartyShares<Vec<ModularNumber<T>>>) -> Result<(), Error> {
        let mapper = PartyMapper::<PrimeField<T>>::new(party_shares.keys().cloned().collect())?;
        let mut point_sequences = vec![PointSequence::<PrimeField<T>>::default(); self.operands.len()];
        for (party_id, party_shares) in party_shares {
            if party_shares.len() != self.operands.len() {
                return Err(anyhow!(
                    "unexpected element share count: expected {}, got {}",
                    self.operands.len(),
                    party_shares.len()
                ));
            }
            let x =
                *mapper.abscissa(&party_id).ok_or_else(|| anyhow!("failed to find abscissa for party {party_id:?}"))?;

            for (element_index, share) in party_shares.into_iter().enumerate() {
                point_sequences[element_index].push(Point::new(x, share));
            }
        }

        let zipped = point_sequences.into_iter().zip(self.operands.iter());
        for (point_sequence, (left, right)) in zipped {
            let mult_trunc_output = point_sequence.lagrange_interpolate()?;

            // The result of MULTIPLICATION-AND-TRUNCATION is multiplication followed by truncation

            let expected_value: i64 = i64::try_from(BigInt::from(left) * BigInt::from(right))?;

            let exponent = 2_u64.pow(self.trunc_exponent as u32) as f64;
            let expected_value = expected_value as f64 / exponent;
            let expected_value = BigInt::from(expected_value.floor() as i64);
            let expected_value = ModularNumber::<T>::try_from(&expected_value).unwrap();

            assert!(
                (expected_value - &mult_trunc_output) <= ModularNumber::ONE,
                "failed for {} * {}, truncating by {}, expected {}, actual {}",
                BigInt::from(left),
                BigInt::from(right),
                self.trunc_exponent,
                BigInt::from(&expected_value),
                BigInt::from(&mult_trunc_output),
            );
        }

        Ok(())
    }
}

#[test]
fn end_to_end() {
    let max_rounds = 100;
    let polynomial_degree = 1;
    let network_size = 5;
    let kappa = 40;
    let k = 20;

    // The numbers we'll apply MULTIPLICATION-AND-TRUNCATION
    let neg_1093 = ModularNumber::try_from(&BigInt::from(-1093)).unwrap();
    let neg_293 = ModularNumber::try_from(&BigInt::from(-293)).unwrap();
    let numbers = vec![
        (ModularNumber::from_u32(10), ModularNumber::from_u32(10)),
        (ModularNumber::from_u32(10), ModularNumber::from_u32(7)),
        (ModularNumber::from_u32(5), ModularNumber::from_u32(17)),
        (ModularNumber::from_u32(38), ModularNumber::from_u32(3)),
        (ModularNumber::from_u32(109), ModularNumber::from_u32(35)),
        (ModularNumber::from_u32(1093), ModularNumber::from_u32(293)),
        (neg_1093, ModularNumber::from_u32(293)),
        (ModularNumber::from_u32(1093), neg_293),
        (neg_1093, neg_293),
    ];
    let simulator = SymmetricProtocolSimulator::new(network_size, max_rounds);
    let protocol = MultTruncProtocol::<U64SafePrime>::new(numbers, polynomial_degree, kappa, k);
    let outputs = simulator.run_protocol(&protocol).expect("protocol run failed");
    let mut party_shares = PartyShares::default();
    for output in outputs {
        party_shares.insert(output.party_id, output.output);
    }

    protocol.validate_output(party_shares).expect("validation failed");
}

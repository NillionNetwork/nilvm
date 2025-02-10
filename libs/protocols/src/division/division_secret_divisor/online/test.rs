//! End-to-end tests for the DIV-INT-SECRET protocol.

#![allow(clippy::arithmetic_side_effects, clippy::indexing_slicing)]

use super::protocol::DivisionIntegerSecretDivisorProtocol;
use crate::simulator::symmetric::SymmetricProtocolSimulator;
use anyhow::{anyhow, Error};
use math_lib::{
    fields::PrimeField,
    modular::{FloorMod, ModularNumber, SafePrime, U128SafePrime, U256SafePrime, U64SafePrime},
    polynomial::{point::Point, point_sequence::PointSequence},
};
use num_bigint::BigInt;
use rstest::rstest;
use shamir_sharing::{
    party::PartyMapper,
    secret_sharer::{PartyShares, SafePrimeSecretSharer, ShamirSecretSharer},
};

impl<T> DivisionIntegerSecretDivisorProtocol<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Validates the output of DIV-INT-SECRET protocol.
    pub fn validate_output(&self, party_shares: PartyShares<Vec<ModularNumber<T>>>) -> Result<(), Error> {
        let mapper = PartyMapper::<PrimeField<T>>::new(party_shares.keys().cloned().collect())?;
        let mut point_sequences = vec![PointSequence::<PrimeField<T>>::default(); self.dividend_divisor.len()];
        for (party_id, party_shares) in party_shares {
            if party_shares.len() != self.dividend_divisor.len() {
                return Err(anyhow!(
                    "unexpected element share count: expected {}, got {}",
                    self.dividend_divisor.len(),
                    party_shares.len()
                ));
            }
            let x =
                *mapper.abscissa(&party_id).ok_or_else(|| anyhow!("failed to find abscissa for party {party_id:?}"))?;

            for (element_index, share) in party_shares.into_iter().enumerate() {
                point_sequences[element_index].push(Point::new(x, share));
            }
        }

        let zipped = point_sequences.into_iter().zip(self.dividend_divisor.iter());
        for (point_sequence, (dividend, divisor)) in zipped {
            let division_output = point_sequence.lagrange_interpolate()?;
            // The division is the result of (dividend - remainder) / divisor.
            // Integer division of modular numbers requires substracting the remainder. See feature docs
            let remainder = dividend.fmod(divisor).unwrap();
            let expected_value = ((dividend - &remainder) / &divisor).expect("failed to compute clear division");

            println!(
                "dividend: {}, divisor: {}, remainder: {}, actual: {}, expected: {}",
                BigInt::from(dividend),
                BigInt::from(divisor),
                BigInt::from(&remainder),
                BigInt::from(&division_output),
                BigInt::from(&expected_value)
            );
            assert_eq!(
                division_output,
                expected_value,
                "failed for {} / {}",
                dividend.into_value(),
                divisor.into_value()
            );
        }

        Ok(())
    }
}

#[rstest]
#[case::u64(U64SafePrime)]
#[case::u128(U128SafePrime)]
#[case::u256(U256SafePrime)]
fn end_to_end<T: SafePrime>(#[case] _prime: T)
where
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    let max_rounds = 100;
    let polynomial_degree = 1;
    let network_size = 5;
    let kappa = 40;
    let k = 20;

    // The numbers we'll apply division
    let neg_1093 = ModularNumber::try_from(&BigInt::from(-1093)).unwrap();
    let neg_293 = ModularNumber::try_from(&BigInt::from(-293)).unwrap();
    let neg_196 = ModularNumber::try_from(&BigInt::from(-196)).unwrap();
    let neg_14 = ModularNumber::try_from(&BigInt::from(-14)).unwrap();
    let numbers = vec![
        (ModularNumber::two(), ModularNumber::ONE),
        (ModularNumber::from_u32(6), ModularNumber::from_u32(3)),
        (ModularNumber::from_u32(10), ModularNumber::from_u32(7)),
        (ModularNumber::from_u32(5), ModularNumber::from_u32(17)),
        (ModularNumber::from_u32(38), ModularNumber::from_u32(3)),
        (ModularNumber::from_u32(109), ModularNumber::from_u32(35)),
        (ModularNumber::from_u32(1093), ModularNumber::from_u32(293)),
        (neg_1093, ModularNumber::from_u32(293)),
        (ModularNumber::from_u32(1093), neg_293),
        (neg_1093, neg_293),
        (neg_196, ModularNumber::from_u32(3)),
        (ModularNumber::from_u32(251), neg_14),
        (ModularNumber::from_u32(683), ModularNumber::from_u32(682)),
    ];
    let simulator = SymmetricProtocolSimulator::new(network_size, max_rounds);
    let protocol = DivisionIntegerSecretDivisorProtocol::<T>::new(numbers, polynomial_degree, kappa, k);
    let outputs = simulator.run_protocol(&protocol).expect("protocol run failed");
    let mut party_shares = PartyShares::default();
    for output in outputs {
        party_shares.insert(output.party_id, output.output);
    }

    protocol.validate_output(party_shares).expect("validation failed");
}

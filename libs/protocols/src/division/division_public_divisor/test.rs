//! End-to-end tests for the DIVISION protocol.

#![allow(clippy::arithmetic_side_effects, clippy::indexing_slicing)]

use crate::simulator::symmetric::SymmetricProtocolSimulator;
use anyhow::{anyhow, Error};
use math_lib::{
    fields::PrimeField,
    modular::{FloorMod, ModularNumber, SafePrime, U64SafePrime},
    polynomial::{point::Point, point_sequence::PointSequence},
};
use num_bigint::BigInt;
use shamir_sharing::{
    party::PartyMapper,
    secret_sharer::{PartyShares, SafePrimeSecretSharer, ShamirSecretSharer},
};

use super::protocol::DivisionIntegerPublicDivisorProtocol;

impl<T> DivisionIntegerPublicDivisorProtocol<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Validates the output of DIVISION protocol.
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

#[test]
fn end_to_end() {
    let max_rounds = 100;
    let polynomial_degree = 1;
    let network_size = 5;
    let kappa = 40;
    let k = 20;

    // The numbers we'll apply division
    let neg_1093 = ModularNumber::try_from(&BigInt::from(-1093)).unwrap();
    let neg_293 = ModularNumber::try_from(&BigInt::from(-293)).unwrap();
    let neg_4 = ModularNumber::try_from(&BigInt::from(-4)).unwrap();
    let neg_2 = ModularNumber::try_from(&BigInt::from(-2)).unwrap();
    let numbers = vec![
        (neg_4, ModularNumber::two()),
        (ModularNumber::from_u32(4), neg_2),
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
    let protocol = DivisionIntegerPublicDivisorProtocol::<U64SafePrime>::new(numbers, polynomial_degree, kappa, k);
    let outputs = simulator.run_protocol(&protocol).expect("protocol run failed");
    let mut party_shares = PartyShares::default();
    for output in outputs {
        party_shares.insert(output.party_id, output.output);
    }

    protocol.validate_output(party_shares).expect("validation failed");
}

#[test]
fn end_to_end_large_divisor_error() {
    let max_rounds = 100;
    let polynomial_degree = 1;
    let network_size = 5;
    let kappa = 40;
    let k = 20;
    let divisor = 1047574;
    let mod_divisor = ModularNumber::<U64SafePrime>::from_u32(divisor);
    let m = mod_divisor.into_value().bits();

    // The numbers we'll apply modulo
    let numbers = vec![(ModularNumber::from_u32(1047579), ModularNumber::from_u32(divisor))];

    let simulator = SymmetricProtocolSimulator::new(network_size, max_rounds);
    let protocol = DivisionIntegerPublicDivisorProtocol::<U64SafePrime>::new(numbers, polynomial_degree, kappa, k);
    match simulator.run_protocol(&protocol) {
        Ok(_) => {
            panic!(
                "Test failed: Expected validation to panic at 'protocol run failed: failed to initialize protocol: Statistical parameter kappa and k are too large for current field size'"
            );
        }
        Err(err) => {
            if m >= k {
                assert_eq!(
                    err.to_string(),
                    anyhow!(
                        "failed to initialize protocol: MODULO: The size of divisor (m) is larger than the allowed size (k)"
                    )
                    .to_string()
                );
            } else {
                panic!("protocol run failed: {}", err);
            }
        }
    }
}

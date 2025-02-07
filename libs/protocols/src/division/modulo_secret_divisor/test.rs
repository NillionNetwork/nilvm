//! End-to-end tests for the Modulo with secret divisor protocol.

#![allow(clippy::arithmetic_side_effects, clippy::indexing_slicing)]

use super::protocol::ModuloIntegerSecretDivisorProtocol;
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

impl<T> ModuloIntegerSecretDivisorProtocol<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Validates the output of Modulo with secret divisor protocol.
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
            let modulo_output = point_sequence.lagrange_interpolate()?;

            let expected_value = dividend.fmod(divisor).unwrap();

            println!(
                "dividend: {}, divisor: {}, actual: {}, expected: {}",
                BigInt::from(dividend),
                BigInt::from(divisor),
                BigInt::from(&modulo_output),
                BigInt::from(&expected_value)
            );
            assert_eq!(
                modulo_output,
                expected_value,
                "failed for {} % {}",
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

    // The numbers we'll apply modulo
    // Note: we are using the divison with secret divisor protocol. For this reason we are only guaranteed correctness
    // for numbers <= 2^(k/2).
    let neg_1093 = ModularNumber::try_from(&BigInt::from(-1093)).unwrap();
    let neg_751 = ModularNumber::try_from(&BigInt::from(-751)).unwrap();
    let neg_51 = ModularNumber::try_from(&BigInt::from(-51)).unwrap();
    let neg_293 = ModularNumber::try_from(&BigInt::from(-293)).unwrap();
    let numbers = vec![
        (ModularNumber::ZERO, ModularNumber::from_u32(10)),
        (ModularNumber::from_u32(5), ModularNumber::from_u32(10)),
        (ModularNumber::from_u32(134), ModularNumber::from_u32(133)),
        (ModularNumber::from_u32(133), ModularNumber::from_u32(133)),
        (neg_1093, ModularNumber::from_u32(293)),
        (neg_1093, neg_293),
        (neg_751, neg_51),
        (ModularNumber::from_u32(1025), ModularNumber::from_u32(1578)),
    ];
    let simulator = SymmetricProtocolSimulator::new(network_size, max_rounds);
    let protocol = ModuloIntegerSecretDivisorProtocol::<U64SafePrime>::new(numbers, polynomial_degree, kappa, k);
    let outputs = simulator.run_protocol(&protocol).expect("protocol run failed");
    let mut party_shares = PartyShares::default();
    for output in outputs {
        party_shares.insert(output.party_id, output.output);
    }

    protocol.validate_output(party_shares).expect("validation failed");
}

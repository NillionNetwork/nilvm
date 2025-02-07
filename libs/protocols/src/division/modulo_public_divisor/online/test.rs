//! End-to-end tests for the MODULO protocol.

#![allow(clippy::arithmetic_side_effects, clippy::indexing_slicing)]

use super::protocol::ModuloProtocol;
use crate::simulator::symmetric::SymmetricProtocolSimulator;
use anyhow::anyhow;
use math_lib::modular::{ModularNumber, U64SafePrime};
use num_bigint::BigInt;
use shamir_sharing::secret_sharer::PartyShares;

#[test]
fn end_to_end() {
    let max_rounds = 100;
    let polynomial_degree = 1;
    let network_size = 5;
    let kappa = 40;
    let k = 20;

    // The numbers we'll apply modulo
    let neg_293 = ModularNumber::try_from(&BigInt::from(-293)).unwrap();
    let neg_109 = ModularNumber::try_from(&BigInt::from(-109)).unwrap();
    let neg_74 = ModularNumber::try_from(&BigInt::from(-74)).unwrap();
    let neg_7517 = ModularNumber::try_from(&BigInt::from(-7517)).unwrap();
    let numbers = vec![
        (ModularNumber::from_u32(10), ModularNumber::from_u32(7)),
        (ModularNumber::from_u32(5), ModularNumber::from_u32(17)),
        (ModularNumber::from_u32(38), ModularNumber::from_u32(3)),
        (ModularNumber::from_u32(109), ModularNumber::from_u32(35)),
        (ModularNumber::from_u32(1093), ModularNumber::from_u32(293)),
        (neg_293, ModularNumber::from_u32(35)),
        (neg_109, ModularNumber::from_u32(13)),
        (neg_74, ModularNumber::from_u32(12)),
        (ModularNumber::from_u32(357), neg_293),
        (ModularNumber::from_u32(1093), neg_293),
        (neg_7517, ModularNumber::from_u32(1557)),
        (ModularNumber::from_u32(1093), neg_293),
        (neg_293, neg_109),
    ];
    let simulator = SymmetricProtocolSimulator::new(network_size, max_rounds);
    let protocol = ModuloProtocol::<U64SafePrime>::new(numbers, polynomial_degree, kappa, k);
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
    let protocol = ModuloProtocol::<U64SafePrime>::new(numbers, polynomial_degree, kappa, k);
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
                        "failed to initialize protocol: The size of divisor (m) is larger than the allowed size (k)"
                    )
                    .to_string()
                );
            } else {
                panic!("protocol run failed: {}", err);
            }
        }
    }
}

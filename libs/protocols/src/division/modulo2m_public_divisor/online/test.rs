//! End-to-end tests for the MODULO protocol.

#![allow(clippy::arithmetic_side_effects, clippy::indexing_slicing)]

use super::protocol::TruncProtocol;
use crate::{
    division::modulo2m_public_divisor::online::protocol::Modulo2mProtocol,
    simulator::symmetric::SymmetricProtocolSimulator,
};
use anyhow::anyhow;
use math_lib::modular::{ModularNumber, U64SafePrime};
use shamir_sharing::secret_sharer::PartyShares;

#[test]
fn end_to_end() {
    let max_rounds = 100;
    let polynomial_degree = 1;
    let network_size = 5;
    let kappa = 40;
    let k = 20;

    // The numbers we'll apply modulo: (dividend, m) for dividend mod 2^m
    let numbers = vec![
        (ModularNumber::from_u32(10), ModularNumber::two()),
        (ModularNumber::from_u32(5), ModularNumber::from_u32(4)),
        (ModularNumber::from_u32(38), ModularNumber::from_u32(6)),
        (ModularNumber::from_u32(109), ModularNumber::from_u32(12)),
        (ModularNumber::from_u32(1093), ModularNumber::from_u32(15)),
        (ModularNumber::from_u32(1093), ModularNumber::from_u32(19)),
    ];
    let simulator = SymmetricProtocolSimulator::new(network_size, max_rounds);
    let protocol = Modulo2mProtocol::<U64SafePrime>::new(numbers, polynomial_degree, kappa, k);
    let outputs = simulator.run_protocol(&protocol).expect("protocol run failed");
    let mut party_shares = PartyShares::default();
    for output in outputs {
        party_shares.insert(output.party_id, output.output);
    }

    protocol.validate_output(party_shares).expect("validation failed");
}

#[test]
fn end_to_end_trunc() {
    let max_rounds = 100;
    let polynomial_degree = 1;
    let network_size = 5;
    let kappa = 40;
    let k = 20;

    // The numbers we'll apply modulo: (dividend, m) for dividend mod 2^m
    let numbers = vec![
        (ModularNumber::from_u32(10), ModularNumber::two()),
        (ModularNumber::from_u32(5), ModularNumber::from_u32(4)),
        (ModularNumber::from_u32(38), ModularNumber::from_u32(4)),
        (ModularNumber::from_u32(109), ModularNumber::from_u32(6)),
        (ModularNumber::from_u32(1093), ModularNumber::from_u32(10)),
        (ModularNumber::from_u32(1093), ModularNumber::from_u32(5)),
    ];
    let simulator = SymmetricProtocolSimulator::new(network_size, max_rounds);
    let protocol = TruncProtocol::<U64SafePrime>::new(numbers, polynomial_degree, kappa, k);
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
    let divexpm = 20;

    // The numbers we'll apply modulo
    let numbers = vec![(ModularNumber::from_u32(1047579), ModularNumber::from_u32(divexpm as u32))];

    let simulator = SymmetricProtocolSimulator::new(network_size, max_rounds);
    let protocol = Modulo2mProtocol::<U64SafePrime>::new(numbers, polynomial_degree, kappa, k);
    match simulator.run_protocol(&protocol) {
        Ok(_) => {
            panic!(
                "Test failed: Expected validation to panic at 'protocol run failed: failed to initialize protocol: Statistical parameter kappa and k are too large for current field size'"
            );
        }
        Err(err) => {
            if divexpm >= k {
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

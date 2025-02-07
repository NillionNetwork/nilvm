//! End-to-end tests for the COMPARE protocol.

#![allow(clippy::arithmetic_side_effects, clippy::indexing_slicing)]

use super::protocol::CompareProtocol;
use crate::simulator::symmetric::SymmetricProtocolSimulator;
use math_lib::modular::{ModularNumber, U64SafePrime};
use shamir_sharing::secret_sharer::PartyShares;

#[test]
fn end_to_end() {
    let max_rounds = 100;
    let polynomial_degree = 2;
    let network_size = 5;

    // The numbers we'll compare
    let numbers = vec![
        (ModularNumber::from_u32(100), ModularNumber::from_u32(100)),
        (ModularNumber::from_u32(18), ModularNumber::ZERO),
        (ModularNumber::from_u32(100), ModularNumber::from_u32(50)),
        (ModularNumber::from_u32(200), ModularNumber::from_u32(201)),
        (ModularNumber::from_u32(201), ModularNumber::from_u32(200)),
    ];
    let simulator = SymmetricProtocolSimulator::new(network_size, max_rounds);
    let protocol = CompareProtocol::<U64SafePrime>::new(numbers, polynomial_degree);
    let outputs = simulator.run_protocol(&protocol).expect("protocol run failed");
    let mut party_shares = PartyShares::default();
    for output in outputs {
        party_shares.insert(output.party_id, output.output);
    }

    protocol.validate_output(party_shares).expect("validation failed");
}

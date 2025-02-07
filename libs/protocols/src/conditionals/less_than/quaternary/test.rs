//! End-to-end tests for the QUATERNARY-LESS-THAN protocol.

#![allow(clippy::arithmetic_side_effects, clippy::indexing_slicing)]

use super::protocol::QuatLessProtocol;
use crate::simulator::symmetric::SymmetricProtocolSimulator;
use math_lib::modular::{ModularNumber, U64SafePrime};
use shamir_sharing::secret_sharer::PartyShares;

#[test]
fn end_to_end() {
    let max_rounds = 100;
    let polynomial_degree = 1;
    let network_size = 3;

    // The numbers we'll compare
    let numbers = vec![
        (ModularNumber::from_u32(100), ModularNumber::from_u32(100)),
        (ModularNumber::from_u32(51), ModularNumber::from_u32(102)),
        (ModularNumber::ONE, ModularNumber::ZERO),
        (ModularNumber::from_u32(200), ModularNumber::from_u32(201)),
        (ModularNumber::from_u32(201), ModularNumber::from_u32(200)),
        (ModularNumber::from_u64(100), ModularNumber::from_u64(18442014072637906212)),
        (ModularNumber::from_u64(18442014072637906945), ModularNumber::from_u64(200)),
        (ModularNumber::from_u64(18442014072637906212), ModularNumber::from_u64(2021)),
        (ModularNumber::from_u64(18442014072637906945), ModularNumber::from_u64(18442014022637906212)),
        (ModularNumber::from_u64(18442014072637906212), ModularNumber::from_u64(18442014072637926927)),
    ];
    let simulator = SymmetricProtocolSimulator::new(network_size, max_rounds);
    let protocol = QuatLessProtocol::<U64SafePrime>::new(numbers, polynomial_degree);
    let outputs = simulator.run_protocol(&protocol).expect("protocol run failed");
    let mut party_shares = PartyShares::default();
    for output in outputs {
        party_shares.insert(output.party_id, output.output);
    }

    protocol.validate_output(party_shares).expect("validation failed");
}

//! End-to-end tests for the RAN-QUATERNARY protocol.

#![allow(clippy::arithmetic_side_effects, clippy::panic, clippy::indexing_slicing)]

use super::{protocol::RanQuaternaryProtocol, state::RanQuaternaryStateOutput};
use crate::simulator::symmetric::SymmetricProtocolSimulator;
use math_lib::modular::U64SafePrime;
use shamir_sharing::secret_sharer::PartyShares;

#[test]
fn end_to_end() {
    let max_rounds = 100;
    let polynomial_degree = 1;
    let network_size = 5;
    let output_numbers = 5;

    let protocol = RanQuaternaryProtocol::<U64SafePrime>::new(output_numbers, polynomial_degree);
    let simulator = SymmetricProtocolSimulator::new(network_size, max_rounds);
    let outputs = simulator.run_protocol(&protocol).expect("protocol run failed");
    let mut party_shares = PartyShares::default();
    for output in outputs {
        match output.output {
            RanQuaternaryStateOutput::Success { shares } => {
                party_shares.insert(output.party_id, shares);
            }
            // These two can happen by chance and should be retried. Once we have deterministic tests that are
            // guaranteed not to fail, this should be a test failure
            RanQuaternaryStateOutput::RanBitwiseAbort => {
                println!("RANDOM-BITWISE abort!");
                return;
            }
        };
    }

    protocol.validate_output(party_shares).expect("validation failed");
}

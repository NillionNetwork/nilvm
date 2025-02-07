//! End-to-end tests for the PREP-COMPARE protocol.

use crate::{
    conditionals::less_than::offline::{
        protocol::PrepCompareProtocol, validation::PrepCompareValidator, PrepCompareStateOutput,
    },
    simulator::symmetric::SymmetricProtocolSimulator,
};
use math_lib::modular::U64SafePrime;
use shamir_sharing::secret_sharer::PartyShares;

#[allow(clippy::panic)]
#[test]
fn end_to_end() {
    let max_rounds = 100;
    let polynomial_degree = 1;
    let network_size = 5;
    let output_elements = 2;

    let protocol = PrepCompareProtocol::<U64SafePrime>::new(output_elements, polynomial_degree);
    let simulator = SymmetricProtocolSimulator::new(network_size, max_rounds);
    let outputs = simulator.run_protocol(&protocol).expect("protocol run failed");
    let mut party_shares = PartyShares::default();
    for output in outputs {
        match output.output {
            PrepCompareStateOutput::Success { shares } => {
                party_shares.insert(output.party_id, shares);
            }
            // This can happen by chance and should be retried. Once we have deterministic tests that are
            // guaranteed not to fail, this should be a test failure
            PrepCompareStateOutput::Abort => {
                println!("Protocol aborted");
                return;
            }
            PrepCompareStateOutput::RanBitwiseAbort => panic!("RANDOM-BITWISE aborted"),
        };
    }

    let validator = PrepCompareValidator;
    validator.validate(output_elements, party_shares).expect("validation failed");
}

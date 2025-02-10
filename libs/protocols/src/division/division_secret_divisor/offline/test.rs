//! End-to-end tests for the PREP-DIV-INT-SECRET protocol.

#![allow(clippy::panic)]

use super::{
    protocol::PrepDivisionIntegerSecretProtocol, validation::PrepDivisionIntegerSecretValidator,
    PrepDivisionIntegerSecretStateOutput,
};
use crate::simulator::symmetric::SymmetricProtocolSimulator;
use anyhow::anyhow;
use math_lib::modular::U64SafePrime;
use rstest::rstest;
use shamir_sharing::secret_sharer::PartyShares;

#[rstest]
#[case::kappa_plus_k_under_size_of_prime(40, 10)]
#[case::kappa_plus_k_over_size_of_prime(40, 30)]
fn end_to_end(#[case] kappa: usize, #[case] k: usize) {
    let max_rounds = 100;
    let polynomial_degree = 2;
    let network_size = 5;
    let output_elements = 2;

    let protocol = PrepDivisionIntegerSecretProtocol::<U64SafePrime>::new(output_elements, polynomial_degree, kappa, k);
    let simulator = SymmetricProtocolSimulator::new(network_size, max_rounds);
    let outputs = match simulator.run_protocol(&protocol) {
        Ok(result) => {
            if kappa + 2 * k > 64 {
                panic!(
                    "Test failed: Expected validation to panic at 'protocol run failed: failed to initialize protocol: Statistical parameter kappa and k are too large for current field size'"
                );
            } else {
                result
            }
        }
        Err(err) => {
            if kappa + 2 * k > 64 {
                assert_eq!(err.to_string(), anyhow!("failed to initialize protocol: Statistical parameter kappa and k are too large for current field size").to_string());
                return;
            } else {
                panic!("protocol run failed: {}", err);
            }
        }
    };

    let mut party_shares = PartyShares::default();
    for output in outputs {
        match output.output {
            PrepDivisionIntegerSecretStateOutput::Success { shares } => {
                party_shares.insert(output.party_id, shares);
            }
            // This can happen by chance and should be retried. Once we have deterministic tests that are
            // guaranteed not to fail, this should be a test failure
            PrepDivisionIntegerSecretStateOutput::Abort => {
                println!("Protocol aborted");
                return;
            }
            PrepDivisionIntegerSecretStateOutput::PrepTruncAbort => panic!("PREP-TRUNC aborted"),
            PrepDivisionIntegerSecretStateOutput::PrepTruncPrAbort => panic!("PREP-TRUNCPR aborted"),
            PrepDivisionIntegerSecretStateOutput::PrepCompareAbort => panic!("PREP-COMPARE aborted"),
            PrepDivisionIntegerSecretStateOutput::RanBitwiseAbort => panic!("RANDOM-BITWISE aborted"),
        };
    }

    let validator = PrepDivisionIntegerSecretValidator::default();
    validator.validate(output_elements, kappa, k, party_shares).expect("validation failed");
}

//! End-to-end tests for the PREP-MODULO protocol.

#![allow(clippy::panic)]

use super::{super::offline::PrepModuloStateOutput, protocol::PrepModuloProtocol, validation::PrepModuloValidator};
use crate::simulator::symmetric::SymmetricProtocolSimulator;
use anyhow::anyhow;
use math_lib::modular::U64SafePrime;
use rstest::rstest;
use std::collections::HashMap;

#[rstest]
#[case::kappa_plus_k_under_size_of_prime(40, 20)]
#[case::kappa_plus_k_over_size_of_prime(40, 30)]
fn end_to_end(#[case] kappa: usize, #[case] k: usize) {
    let max_rounds = 100;
    let polynomial_degree = 1;
    let network_size = 5;
    let output_elements = 2;

    let protocol = PrepModuloProtocol::<U64SafePrime>::new(output_elements, polynomial_degree, kappa, k);
    let simulator = SymmetricProtocolSimulator::new(network_size, max_rounds);
    let outputs = match simulator.run_protocol(&protocol) {
        Ok(result) => {
            if kappa + k > 64 {
                panic!(
                    "Test failed: Expected validation to panic at 'protocol run failed: failed to initialize protocol: Statistical parameter kappa and k are too large for current field size'"
                );
            } else {
                result
            }
        }
        Err(err) => {
            if kappa + k > 64 {
                assert_eq!(err.to_string(), anyhow!("failed to initialize protocol: Statistical parameter kappa and k are too large for current field size").to_string());
                return;
            } else {
                panic!("protocol run failed: {}", err);
            }
        }
    };

    let mut party_shares = HashMap::new();
    for output in outputs {
        match output.output {
            PrepModuloStateOutput::Success { shares } => {
                party_shares.insert(output.party_id, shares);
            }
            // This can happen by chance and should be retried. Once we have deterministic tests that are
            // guaranteed not to fail, this should be a test failure
            PrepModuloStateOutput::Abort => {
                println!("Protocol aborted");
                return;
            }
            PrepModuloStateOutput::RanAbort => panic!("RAN-BIT aborted"),
            PrepModuloStateOutput::PrepCompareAbort => panic!("PREP-COMPARE aborted"),
        };
    }

    let validator = PrepModuloValidator::default();
    validator.validate(output_elements, kappa, k, party_shares).expect("validation failed");
}

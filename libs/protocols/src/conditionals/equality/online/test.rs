#![allow(clippy::panic)]

use super::{
    output::{PrivateOutputEqualityShares, PrivateOutputEqualityStateOutput},
    protocol::PrivateOutputEqualityProtocol,
    validation::PrivateOutputEqualityValidator,
};

use crate::simulator::symmetric::SymmetricProtocolSimulator;
use basic_types::PartyId;
use math_lib::modular::ModularNumber;
use num_bigint::BigInt;
use std::collections::HashMap;

#[test]
fn end_to_end() {
    let network_size = 5;
    let max_rounds = 100;
    let polynomial_degree = 2;
    type Prime = math_lib::modular::U64SafePrime;

    let x = vec![0, 1, 2, 1, -3, -1, 4]
        .iter()
        .map(|c| ModularNumber::try_from(&BigInt::from(*c)))
        .collect::<Result<Vec<ModularNumber<Prime>>, _>>()
        .unwrap();
    let y = vec![0, 1, 1, 0, -3, -1, 4]
        .iter()
        .map(|c| ModularNumber::try_from(&BigInt::from(*c)))
        .collect::<Result<Vec<ModularNumber<Prime>>, _>>()
        .unwrap();

    let simulator = SymmetricProtocolSimulator::new(network_size, max_rounds);
    let protocol = PrivateOutputEqualityProtocol::<Prime>::new(polynomial_degree, x.clone(), y.clone());
    let outputs = simulator.run_protocol(&protocol).expect("protocol run failed");
    let mut party_shares: HashMap<PartyId, Vec<PrivateOutputEqualityShares<Prime>>> = HashMap::new();
    for party_output in outputs {
        match party_output.output {
            PrivateOutputEqualityStateOutput { outputs } => {
                party_shares.insert(party_output.party_id, outputs);
            }
        };
    }
    let validator = PrivateOutputEqualityValidator;
    validator.validate::<Prime>(party_shares, polynomial_degree, x, y).expect("validation failed");
}

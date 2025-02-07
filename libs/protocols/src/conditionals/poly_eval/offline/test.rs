#![allow(clippy::panic)]

use super::{
    output::{PrepPolyEvalShares, PrepPolyEvalStateOutput},
    protocol::PrepPolyEvalProtocol,
    validation::{PrepPolyEvalBuilder, PrepPolyEvalValidator},
};
use crate::simulator::symmetric::SymmetricProtocolSimulator;
use basic_types::PartyId;
use shamir_sharing::secret_sharer::{PartyShares, ShamirSecretSharer};

#[test]
fn end_to_end() {
    let element_count = 15;
    let network_size = 5;
    let max_rounds = 100;
    let polynomial_degree = 2;
    let poly_eval_degree = 5;
    type Prime = math_lib::modular::U64SafePrime;

    let simulator = SymmetricProtocolSimulator::new(network_size, max_rounds);
    let protocol = PrepPolyEvalProtocol::<Prime>::new(element_count, polynomial_degree, poly_eval_degree);
    let outputs = simulator.run_protocol(&protocol).expect("protocol run failed");
    let mut party_shares = PartyShares::default();
    for party_output in outputs {
        match party_output.output {
            PrepPolyEvalStateOutput::Success { outputs } => {
                party_shares.insert(party_output.party_id, outputs);
            }
            _ => panic!("PrepPolyEvalProtocol Aborted"),
        };
    }
    let validator = PrepPolyEvalValidator;
    validator
        .validate::<Prime>(element_count, party_shares, polynomial_degree, poly_eval_degree)
        .expect("validation failed");
}

#[test]
fn end_to_end_builder() {
    let element_count = 1;
    let network_size = 5;
    let polynomial_degree = 2;
    let poly_eval_degree = 5;
    type Prime = math_lib::modular::U64SafePrime;

    let parties: Vec<PartyId> = (0..network_size).into_iter().map(|i| PartyId::from(i)).collect();
    let sharer = ShamirSecretSharer::<Prime>::new(parties[0].clone(), polynomial_degree, parties.to_vec()).unwrap();
    let builder = PrepPolyEvalBuilder::new(&sharer, rand::thread_rng()).unwrap();
    let party_shares: PartyShares<Vec<PrepPolyEvalShares<Prime>>> =
        builder.build(element_count, poly_eval_degree).unwrap();

    let validator = PrepPolyEvalValidator;
    validator
        .validate::<Prime>(element_count, party_shares, polynomial_degree, poly_eval_degree)
        .expect("validation failed");
}

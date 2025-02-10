#![allow(clippy::panic)]

use super::{
    output::{PrepPrivateOutputEqualityShares, PrepPrivateOutputEqualityStateOutput},
    protocol::PrepPrivateOutputEqualityProtocol,
    validation::{PrepPrivateOutputEqualitySharesBuilder, PrepPrivateOutputEqualityValidator},
};
use crate::{conditionals::equality::POLY_EVAL_DEGREE, simulator::symmetric::SymmetricProtocolSimulator};
use basic_types::PartyId;
use shamir_sharing::secret_sharer::{PartyShares, ShamirSecretSharer};

#[test]
fn end_to_end() {
    let element_count = 1;
    let network_size = 5;
    let max_rounds = 100;
    let polynomial_degree = 2;
    let poly_eval_degree = POLY_EVAL_DEGREE;
    type Prime = math_lib::modular::U64SafePrime;

    let simulator = SymmetricProtocolSimulator::new(network_size, max_rounds);
    let protocol = PrepPrivateOutputEqualityProtocol::<Prime>::new(element_count, polynomial_degree);
    let outputs = simulator.run_protocol(&protocol).expect("protocol run failed");
    let mut party_shares = PartyShares::default();
    for party_output in outputs {
        match party_output.output {
            PrepPrivateOutputEqualityStateOutput::Success { shares: outputs } => {
                party_shares.insert(party_output.party_id, outputs);
            }
            _ => panic!("PrepPrivateOutputEqualityProtocol Aborted"),
        };
    }
    let validator = PrepPrivateOutputEqualityValidator::<Prime>::new();
    validator.validate(element_count, party_shares, polynomial_degree, poly_eval_degree).expect("validation failed");
}

#[test]
fn end_to_end_builder() {
    let element_count = 1;
    let network_size = 5;
    let polynomial_degree = 2;
    let poly_eval_degree = POLY_EVAL_DEGREE;
    type Prime = math_lib::modular::U64SafePrime;

    let parties: Vec<PartyId> = (0..network_size).into_iter().map(|i| PartyId::from(i)).collect();
    let sharer = ShamirSecretSharer::<Prime>::new(parties[0].clone(), polynomial_degree, parties.to_vec()).unwrap();
    let builder = PrepPrivateOutputEqualitySharesBuilder::new(&sharer, rand::thread_rng()).unwrap();
    let party_shares: PartyShares<Vec<PrepPrivateOutputEqualityShares<Prime>>> = builder.build(element_count).unwrap();

    let validator = PrepPrivateOutputEqualityValidator::<Prime>::new();
    validator.validate(element_count, party_shares, polynomial_degree, poly_eval_degree).expect("validation failed");
}

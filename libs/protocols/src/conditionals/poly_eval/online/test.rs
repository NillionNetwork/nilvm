#![allow(clippy::panic)]

use super::{
    output::{PolyEvalShares, PolyEvalStateOutput},
    protocol::PolyEvalProtocol,
    validation::PolyEvalValidator,
};

use crate::simulator::symmetric::SymmetricProtocolSimulator;
use basic_types::PartyId;
use math_lib::{
    fields::PrimeField,
    modular::{ModularNumber, Prime},
    polynomial::Polynomial,
};
use num_bigint::BigInt;
use rand::Rng;
use std::collections::HashMap;

/// Creates a polynomial with the given coefficients.
fn make_polynomial<T: Prime>(coefficients: &[i32]) -> Polynomial<PrimeField<T>> {
    let coefs = coefficients
        .into_iter()
        .map(|c| ModularNumber::try_from(&BigInt::from(*c)))
        .collect::<Result<Vec<ModularNumber<T>>, _>>()
        .unwrap();
    assert!(!coefs.is_empty());
    Polynomial::new(coefs)
}

fn generate_random_coefficients(degree: usize) -> Vec<i32> {
    let mut rng = rand::thread_rng();
    (0..=degree).map(|_| rng.gen_range(-100..=100)).collect()
}

#[test]
fn end_to_end() {
    let network_size = 5;
    let max_rounds = 100;
    let polynomial_degree = 2;
    let eval_polynomial_degree = 5;
    type Prime = math_lib::modular::U64SafePrime;

    let coeff1 = (0..eval_polynomial_degree + 1).map(|i| i as i32).collect::<Vec<i32>>();
    let coeff2 = (0..eval_polynomial_degree + 1).map(|i| -(i as i32)).collect::<Vec<i32>>();
    let coeff3 = generate_random_coefficients(eval_polynomial_degree as usize);
    let coeff4 = generate_random_coefficients(eval_polynomial_degree as usize);
    let polynomials = vec![
        make_polynomial::<Prime>(&coeff1),
        make_polynomial::<Prime>(&coeff2),
        make_polynomial::<Prime>(&coeff3),
        make_polynomial::<Prime>(&coeff4),
    ];
    let x = vec![
        ModularNumber::<Prime>::ONE,
        ModularNumber::<Prime>::from_u32(2),
        ModularNumber::<Prime>::from_u32(3),
        ModularNumber::<Prime>::from_u32(4),
    ];
    let simulator = SymmetricProtocolSimulator::new(network_size, max_rounds);
    let protocol =
        PolyEvalProtocol::<Prime>::new(polynomial_degree, x.clone(), polynomials.clone(), eval_polynomial_degree);
    let outputs = simulator.run_protocol(&protocol).expect("protocol run failed");
    let mut party_shares: HashMap<PartyId, Vec<PolyEvalShares<Prime>>> = HashMap::new();
    for party_output in outputs {
        match party_output.output {
            PolyEvalStateOutput::Success { outputs } => {
                party_shares.insert(party_output.party_id, outputs);
            }
        };
    }
    let validator = PolyEvalValidator;
    validator.validate::<Prime>(party_shares, polynomial_degree, polynomials, x).expect("validation failed");
}

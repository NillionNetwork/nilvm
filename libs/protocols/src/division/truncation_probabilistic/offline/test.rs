//! End-to-end tests for the PREP-TRUNCPR protocol.

#![allow(clippy::panic)]

use super::{
    super::offline::{state::PrepTruncPrState, PrepTruncPrStateOutput},
    validation::PrepTruncPrValidator,
};
use crate::simulator::symmetric::{InitializedProtocol, Protocol, SymmetricProtocolSimulator};
use anyhow::{anyhow, Error};
use math_lib::modular::{SafePrime, U64SafePrime};
use rstest::rstest;
use shamir_sharing::{
    party::PartyId,
    secret_sharer::{PartyShares, SafePrimeSecretSharer, ShamirSecretSharer},
};
use std::{marker::PhantomData, sync::Arc};

pub(crate) struct PrepTruncPrProtocol<T> {
    element_count: usize,
    polynomial_degree: u64,
    kappa: usize,
    k: usize,
    _unused: PhantomData<T>,
}

impl<T> PrepTruncPrProtocol<T> {
    pub fn new(element_count: usize, polynomial_degree: u64, kappa: usize, k: usize) -> Self {
        Self { element_count, polynomial_degree, kappa, k, _unused: Default::default() }
    }
}

impl<T> Protocol for PrepTruncPrProtocol<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    type State = PrepTruncPrState<T>;
    type PrepareOutput = PrepTruncPrConfig;

    fn prepare(&self, parties: &[PartyId]) -> Result<Self::PrepareOutput, Error> {
        let parties = parties.to_vec();
        Ok(PrepTruncPrConfig { parties })
    }

    fn initialize(
        &self,
        party_id: PartyId,
        config: &Self::PrepareOutput,
    ) -> Result<InitializedProtocol<Self::State>, Error> {
        let secret_sharer = ShamirSecretSharer::new(party_id, self.polynomial_degree, config.parties.clone())?;
        let (state, initial_messages) =
            PrepTruncPrState::new(self.element_count, self.kappa, self.k, Arc::new(secret_sharer))?;
        Ok(InitializedProtocol::new(state, initial_messages))
    }
}

pub(crate) struct PrepTruncPrConfig {
    parties: Vec<PartyId>,
}

#[rstest]
#[case::kappa_plus_k_under_size_of_prime(40, 20)]
#[case::kappa_plus_k_over_size_of_prime(40, 30)]
fn end_to_end(#[case] kappa: usize, #[case] k: usize) {
    let max_rounds = 100;
    let polynomial_degree = 1;
    let network_size = 5;
    let output_elements = 2;

    let protocol = PrepTruncPrProtocol::<U64SafePrime>::new(output_elements, polynomial_degree, kappa, k);
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

    let mut party_shares = PartyShares::default();
    for output in outputs {
        match output.output {
            PrepTruncPrStateOutput::Success { shares } => {
                party_shares.insert(output.party_id, shares);
            }
            // This can happen by chance and should be retried. Once we have deterministic tests that are
            // guaranteed not to fail, this should be a test failure
            PrepTruncPrStateOutput::Abort => {
                println!("Protocol aborted");
                return;
            }
            PrepTruncPrStateOutput::RanAbort => panic!("RAN-BIT aborted"),
        };
    }

    let validator = PrepTruncPrValidator::default();
    validator.validate(output_elements, kappa, k, party_shares).expect("validation failed");
}

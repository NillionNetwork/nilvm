//! End-to-end tests for the PREP-MODULO protocol.

#![allow(clippy::panic)]

use super::{
    super::offline::{state::PrepModulo2mState, PrepModulo2mStateOutput},
    validation::PrepModulo2mValidator,
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

pub(crate) struct PrepModuloProtocol<T> {
    element_count: usize,
    polynomial_degree: u64,
    kappa: usize,
    k: usize,
    _unused: PhantomData<T>,
}

impl<T> PrepModuloProtocol<T> {
    pub fn new(element_count: usize, polynomial_degree: u64, kappa: usize, k: usize) -> Self {
        Self { element_count, polynomial_degree, kappa, k, _unused: Default::default() }
    }
}

impl<T> Protocol for PrepModuloProtocol<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    type State = PrepModulo2mState<T>;
    type PrepareOutput = PrepModuloConfig;

    fn prepare(&self, parties: &[PartyId]) -> Result<Self::PrepareOutput, Error> {
        let parties = parties.to_vec();
        Ok(PrepModuloConfig { parties })
    }

    fn initialize(
        &self,
        party_id: PartyId,
        config: &Self::PrepareOutput,
    ) -> Result<InitializedProtocol<Self::State>, Error> {
        let secret_sharer = ShamirSecretSharer::new(party_id, self.polynomial_degree, config.parties.clone())?;
        let (state, initial_messages) =
            PrepModulo2mState::new(self.element_count, self.kappa, self.k, Arc::new(secret_sharer))?;
        Ok(InitializedProtocol::new(state, initial_messages))
    }
}

pub(crate) struct PrepModuloConfig {
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

    let mut party_shares = PartyShares::default();
    for output in outputs {
        match output.output {
            PrepModulo2mStateOutput::Success { shares } => {
                party_shares.insert(output.party_id, shares);
            }
            // This can happen by chance and should be retried. Once we have deterministic tests that are
            // guaranteed not to fail, this should be a test failure
            PrepModulo2mStateOutput::Abort => {
                println!("Protocol aborted");
                return;
            }
            PrepModulo2mStateOutput::RanAbort => panic!("RAN-BIT aborted"),
            PrepModulo2mStateOutput::PrepCompareAbort => panic!("PREP-COMPARE aborted"),
        };
    }

    let validator = PrepModulo2mValidator::default();
    validator.validate(output_elements, kappa, k, party_shares).expect("validation failed");
}

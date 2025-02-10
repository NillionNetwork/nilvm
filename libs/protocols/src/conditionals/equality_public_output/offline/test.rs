//! End-to-end tests for the PREP-PUBLIC-OUTPUT-EQUALITY protocol.

#![allow(clippy::panic)]

use super::{
    state::PrepPublicOutputEqualityState, validation::PrepPublicOutputEqualityValidator,
    PrepPublicOutputEqualityStateOutput,
};
use crate::simulator::symmetric::{InitializedProtocol, Protocol, SymmetricProtocolSimulator};
use anyhow::Error;
use math_lib::modular::{SafePrime, U64SafePrime};
use shamir_sharing::{
    party::PartyId,
    secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer},
};
use std::{collections::HashMap, marker::PhantomData, sync::Arc};

pub(crate) struct PrepPublicOutputEqualityProtocol<T> {
    element_count: usize,
    polynomial_degree: u64,
    _unused: PhantomData<T>,
}

impl<T> PrepPublicOutputEqualityProtocol<T> {
    pub fn new(element_count: usize, polynomial_degree: u64) -> Self {
        Self { element_count, polynomial_degree, _unused: Default::default() }
    }
}

impl<T> Protocol for PrepPublicOutputEqualityProtocol<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    type State = PrepPublicOutputEqualityState<T>;
    type PrepareOutput = PrepPublicOutputEqualityConfig;

    fn prepare(&self, parties: &[PartyId]) -> Result<Self::PrepareOutput, Error> {
        let parties = parties.to_vec();
        Ok(PrepPublicOutputEqualityConfig { parties })
    }

    fn initialize(
        &self,
        party_id: PartyId,
        config: &Self::PrepareOutput,
    ) -> Result<InitializedProtocol<Self::State>, Error> {
        let secret_sharer = ShamirSecretSharer::new(party_id, self.polynomial_degree, config.parties.clone())?;
        let (state, initial_messages) =
            PrepPublicOutputEqualityState::new(self.element_count, Arc::new(secret_sharer))?;
        Ok(InitializedProtocol::new(state, initial_messages))
    }
}

pub(crate) struct PrepPublicOutputEqualityConfig {
    parties: Vec<PartyId>,
}

#[test]
fn end_to_end() {
    let max_rounds = 100;
    let polynomial_degree = 1;
    let network_size = 5;
    let output_elements = 2;

    let protocol = PrepPublicOutputEqualityProtocol::<U64SafePrime>::new(output_elements, polynomial_degree);
    let simulator = SymmetricProtocolSimulator::new(network_size, max_rounds);
    let outputs = simulator.run_protocol(&protocol).expect("protocol run failed");
    let mut party_shares = HashMap::new();
    for output in outputs {
        match output.output {
            PrepPublicOutputEqualityStateOutput::Success { shares } => {
                party_shares.insert(output.party_id, shares);
            }
        };
    }

    let validator = PrepPublicOutputEqualityValidator::default();
    validator.validate(output_elements, party_shares).expect("validation failed");
}

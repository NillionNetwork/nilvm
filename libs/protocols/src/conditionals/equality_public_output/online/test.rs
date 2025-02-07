#![allow(clippy::panic)]

use super::state::PublicOutputEqualityShares;
use crate::{
    conditionals::equality_public_output::{
        offline::{validation::PrepPublicOutputEqualitySharesBuilder, PrepPublicOutputEqualityShares},
        PublicOutputEqualityState,
    },
    simulator::symmetric::{InitializedProtocol, Protocol, SymmetricProtocolSimulator},
};
use anyhow::{anyhow, Error};
use basic_types::PartyId;
use math_lib::{
    fields::PrimeField,
    modular::{ModularNumber, Prime, SafePrime, U64SafePrime},
};
use shamir_sharing::{
    party::PartyMapper,
    protocol::{PolyDegree, Shamir},
    secret_sharer::{PartyShares, SafePrimeSecretSharer, ShamirSecretSharer},
};
use std::sync::Arc;

pub(crate) struct PublicOutputEqualityProtocol<T: Prime> {
    secrets: Vec<(ModularNumber<T>, ModularNumber<T>)>,
    polynomial_degree: u64,
}

impl<T> PublicOutputEqualityProtocol<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    pub fn new(secrets: Vec<(ModularNumber<T>, ModularNumber<T>)>, polynomial_degree: u64) -> Self {
        Self { secrets, polynomial_degree }
    }

    pub fn validate_output(&self, party_shares: PartyShares<Vec<ModularNumber<T>>>) -> Result<(), Error> {
        let mut all_public_variables = Vec::new();
        for (_, party_shares) in party_shares {
            if party_shares.len() != self.secrets.len() {
                return Err(anyhow!(
                    "unexpected element share count: expected {}, got {}",
                    self.secrets.len(),
                    party_shares.len()
                ));
            }

            all_public_variables.push(party_shares.into_iter().collect::<Vec<_>>());
        }
        assert!(!all_public_variables.is_empty());
        for public_variable in all_public_variables.iter() {
            assert_eq!(
                &all_public_variables[0], public_variable,
                "expected public variables from all parties to be the same, but {:?} is not equal to {:?}",
                public_variable, all_public_variables[0]
            );
        }
        let zipped = all_public_variables[0].clone().into_iter().zip(self.secrets.iter());
        for (first_public_variable, (left, right)) in zipped {
            let expected_value = if left == right { ModularNumber::ONE } else { ModularNumber::ZERO };
            assert_eq!(first_public_variable, expected_value, "failed for {}", first_public_variable.into_value());
        }
        Ok(())
    }

    fn create_prep_shares(
        &self,
        parties: &[PartyId],
        count: usize,
    ) -> Result<PartyShares<Vec<PrepPublicOutputEqualityShares<T>>>, Error> {
        let sharer = ShamirSecretSharer::new(parties[0].clone(), self.polynomial_degree, parties.to_vec())?;
        let builder = PrepPublicOutputEqualitySharesBuilder::new(&sharer, rand::thread_rng())?;
        builder.build(count)
    }
}

impl<T> Protocol for PublicOutputEqualityProtocol<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    type State = PublicOutputEqualityState<T>;
    type PrepareOutput = PublicOutputEqualityConfig<T>;

    fn prepare(&self, parties: &[PartyId]) -> Result<Self::PrepareOutput, Error> {
        let parties = parties.to_vec();
        let mapper = PartyMapper::<PrimeField<T>>::new(parties.clone())?;
        // Note: the party id doesn't matter in this context
        let shamir = Shamir::<PrimeField<T>>::new(PartyId::from(0), self.polynomial_degree, parties.clone())?;

        // We're using 2 sets of parties here: _ours_ and the ones that PREP-PUBLIC-OUTPUT-EQUALITY ran with.
        // Because of this, we need to map our parties into an abscissa and into a party id in the
        // PREP-PUBLIC-OUTPUT-EQUALITY party set.
        let prep_shares = self
            .create_prep_shares(&parties, self.secrets.len())
            .map_err(|e| anyhow!("PREP-PUBLIC-OUTPUT-EQUALITY share creation failed: {e}"))?;
        let mut party_shares: PartyShares<Vec<PublicOutputEqualityShares<T>>> = PartyShares::default();
        for (index, (left, right)) in self.secrets.iter().enumerate() {
            let left_shares = shamir.generate_shares(left, PolyDegree::T)?;
            let right_shares = shamir.generate_shares(right, PolyDegree::T)?;
            for (left_point, right_point) in
                left_shares.into_points().into_iter().zip(right_shares.into_points().into_iter())
            {
                let (left_x, left_share) = left_point.into_coordinates();
                let (right_x, right_share) = right_point.into_coordinates();
                assert_eq!(left_x, right_x);

                let party_id = mapper.party(&right_x).ok_or_else(|| anyhow!("party id for {right_x:?} not found"))?;

                let prep_elements =
                    prep_shares.get(party_id).ok_or_else(|| anyhow!("shares for {party_id} not found"))?;

                let public_output_equality_shares = PublicOutputEqualityShares {
                    left: left_share,
                    right: right_share,
                    prep_shares: prep_elements
                        .get(index)
                        .ok_or_else(|| anyhow!("elements for {index} not found"))?
                        .clone(),
                };

                party_shares.entry(party_id.clone()).or_default().push(public_output_equality_shares);
            }
        }
        Ok(PublicOutputEqualityConfig { parties, party_shares })
    }

    fn initialize(
        &self,
        party_id: PartyId,
        config: &Self::PrepareOutput,
    ) -> Result<InitializedProtocol<Self::State>, Error> {
        let party_shares =
            config.party_shares.get(&party_id).cloned().ok_or_else(|| anyhow!("shares for party {party_id:?}"))?;
        let secret_sharer = ShamirSecretSharer::new(party_id, self.polynomial_degree, config.parties.clone())?;
        let (state, initial_messages) = PublicOutputEqualityState::new(party_shares, Arc::new(secret_sharer))?;
        Ok(InitializedProtocol::new(state, initial_messages))
    }
}

pub(crate) struct PublicOutputEqualityConfig<T: SafePrime> {
    parties: Vec<PartyId>,
    party_shares: PartyShares<Vec<PublicOutputEqualityShares<T>>>,
}

#[test]
fn end_to_end() {
    let max_rounds = 100;
    let polynomial_degree = 1;
    let network_size = 5;

    // transform into vec of tuple
    let secrets = vec![
        (ModularNumber::from_u32(15), ModularNumber::from_u32(15)),
        (ModularNumber::from_u32(100), ModularNumber::from_u32(101)),
    ];

    let simulator = SymmetricProtocolSimulator::new(network_size, max_rounds);
    let protocol = PublicOutputEqualityProtocol::<U64SafePrime>::new(secrets, polynomial_degree);
    let outputs = simulator.run_protocol(&protocol).expect("protocol run failed");
    let mut party_shares = PartyShares::default();
    for output in outputs {
        party_shares.insert(output.party_id, output.output);
    }
    protocol.validate_output(party_shares).expect("validation failed");
}

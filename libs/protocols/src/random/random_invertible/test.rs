#![allow(clippy::indexing_slicing, clippy::panic, clippy::arithmetic_side_effects)]

use super::state::{InvRanState, InvRanStateOutput, InvertibleElement};
use crate::simulator::symmetric::{InitializedProtocol, Protocol, SymmetricProtocolSimulator};
use anyhow::{anyhow, Error, Result};
use basic_types::PartyId;
use math_lib::{
    fields::PrimeField,
    modular::{ModularInverse, ModularNumber, SafePrime, U256SafePrime},
    polynomial::{point::Point, point_sequence::PointSequence},
};
use shamir_sharing::{
    party::PartyMapper,
    secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer},
};
use std::{collections::HashMap, marker::PhantomData, sync::Arc};

struct InvRanProtocol<T: SafePrime> {
    invertibles_count: usize,
    polynomial_degree: u64,
    _unused: PhantomData<T>,
}

impl<T: SafePrime> InvRanProtocol<T> {
    fn new(invertibles_count: usize, polynomial_degree: u64) -> Self {
        Self { invertibles_count, polynomial_degree, _unused: Default::default() }
    }

    fn validate_output(self, party_shares: HashMap<PartyId, Vec<InvertibleElement<T>>>) -> Result<()> {
        let mut element_point_sequences = vec![PointSequence::<PrimeField<T>>::default(); self.invertibles_count];
        let mut inverse_point_sequences = vec![PointSequence::<PrimeField<T>>::default(); self.invertibles_count];
        let mapper = PartyMapper::<PrimeField<T>>::new(party_shares.keys().cloned().collect())?;
        // Merge the outputs into a single point sequence per invertible.
        for (party_id, invertibles) in party_shares {
            for (index, invertible) in invertibles.into_iter().enumerate() {
                let x = *mapper
                    .abscissa(&party_id)
                    .ok_or_else(|| anyhow!("failed to find abscissa for party {party_id:?}"))?;
                element_point_sequences[index].push(Point::new(x, invertible.element));
                inverse_point_sequences[index].push(Point::new(x, invertible.inverse));
            }
        }
        // Now interpolate them and expect the outputs to be invertibles.
        for (element_points, inverse_points) in element_point_sequences.into_iter().zip(inverse_point_sequences.iter())
        {
            let element: ModularNumber<T> = element_points.lagrange_interpolate()?;
            let inverse = inverse_points.lagrange_interpolate()?;
            assert!(element.is_inverse(&inverse));
        }
        Ok(())
    }
}

impl<T: SafePrime> Protocol for InvRanProtocol<T>
where
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    type State = InvRanState<T>;
    type PrepareOutput = InvRanConfig;

    fn prepare(&self, parties: &[PartyId]) -> Result<Self::PrepareOutput, Error> {
        let parties = parties.to_vec();
        let config = InvRanConfig { parties };
        Ok(config)
    }

    fn initialize(
        &self,
        party_id: PartyId,
        config: &Self::PrepareOutput,
    ) -> Result<InitializedProtocol<Self::State>, anyhow::Error> {
        let secret_sharer = ShamirSecretSharer::new(party_id, self.polynomial_degree, config.parties.clone())?;
        let (state, messages) = InvRanState::new(self.invertibles_count, Arc::new(secret_sharer))?;
        Ok(InitializedProtocol::new(state, messages))
    }
}

struct InvRanConfig {
    parties: Vec<PartyId>,
}

#[test]
fn end_to_end() {
    let network_size = 5;
    let polynomial_degree = 1;
    let max_rounds = 100;
    let invertibles_count = 10;

    let protocol = InvRanProtocol::<U256SafePrime>::new(invertibles_count, polynomial_degree);
    let simulator = SymmetricProtocolSimulator::new(network_size, max_rounds);
    let outputs = simulator.run_protocol(&protocol).expect("protocol run failed");
    let mut party_shares = HashMap::new();
    for output in outputs {
        match output.output {
            InvRanStateOutput::Success { elements } => {
                party_shares.insert(output.party_id, elements);
            }
            InvRanStateOutput::RanAbort => panic!("RAN abort!"),
            InvRanStateOutput::Abort => {
                // This one can happen randomly and is a retriable failure. We will eventually be able to have
                // deterministic tests but given we're using random numbers now, this isn't panicking as it's
                // bound to sporadically fail.
                println!("protocol aborted!");
                return;
            }
        };
    }
    protocol.validate_output(party_shares).expect("validation failed");
}

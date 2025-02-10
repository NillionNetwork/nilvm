#![allow(clippy::indexing_slicing)]

use super::super::online::state::{UnboundedMultState, UnboundedMultStateOutput};
use crate::simulator::symmetric::{InitializedProtocol, Protocol, SymmetricProtocolSimulator};
use anyhow::{anyhow, Error, Result};
use basic_types::{Batches, PartyId};
use math_lib::{
    fields::PrimeField,
    modular::{ModularNumber, SafePrime, U256SafePrime},
    polynomial::{point::Point, point_sequence::PointSequence},
};
use shamir_sharing::{
    party::PartyMapper,
    protocol::{PolyDegree, Shamir},
    secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer},
};
use std::{collections::HashMap, marker::PhantomData, sync::Arc};

struct UnboundedMultProtocol<T: SafePrime> {
    polynomial_degree: u64,
    batches: Batches<ModularNumber<T>>,
    expected_outputs: Vec<ModularNumber<T>>,
    _unused: PhantomData<T>,
}

impl<T: SafePrime> UnboundedMultProtocol<T> {
    fn new(polynomial_degree: u64, batches: Batches<ModularNumber<T>>) -> Self {
        // Compute the expected multiplication output per batch.
        let mut expected_outputs = Vec::new();
        for batch in batches.iter() {
            let mut result = ModularNumber::ONE;
            for number in batch {
                result = result * number;
            }
            expected_outputs.push(result);
        }
        Self { polynomial_degree, batches, expected_outputs, _unused: Default::default() }
    }

    fn validate_output(self, party_shares: HashMap<PartyId, Vec<ModularNumber<T>>>) -> Result<()> {
        let mut point_sequences = vec![PointSequence::<PrimeField<T>>::default(); self.expected_outputs.len()];
        let mapper = PartyMapper::<PrimeField<T>>::new(party_shares.keys().cloned().collect())?;
        for (party_id, shares) in party_shares {
            for (index, share) in shares.into_iter().enumerate() {
                let x = *mapper
                    .abscissa(&party_id)
                    .ok_or_else(|| anyhow!("failed to find abscissa for party {party_id:?}"))?;
                point_sequences[index].push(Point::new(x, share));
            }
        }
        for (point_sequence, expected_secret) in point_sequences.into_iter().zip(self.expected_outputs.iter()) {
            let secret = point_sequence.lagrange_interpolate()?;
            assert_eq!(expected_secret, &secret);
        }
        Ok(())
    }
}

impl<T: SafePrime> Protocol for UnboundedMultProtocol<T>
where
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    type State = UnboundedMultState<T>;
    type PrepareOutput = UnboundedMultConfig<T>;

    fn prepare(&self, parties: &[PartyId]) -> Result<Self::PrepareOutput, Error> {
        let mut party_batches = HashMap::new();
        // Note: the party id doesn't matter in this context
        let shamir = Shamir::<PrimeField<T>>::new(PartyId::from(0), self.polynomial_degree, parties.to_vec())?;
        for (index, batch) in self.batches.iter().enumerate() {
            for number in batch {
                let shares = shamir.generate_shares(number, PolyDegree::T)?;
                for share_point in shares.into_points() {
                    let (x, share) = share_point.into_coordinates();
                    let party_id = shamir.party_mapper().party(&x).ok_or_else(|| anyhow!("party not found"))?.clone();
                    party_batches.entry(party_id).or_insert_with(|| Batches::empty(self.batches.len()))[index]
                        .push(share);
                }
            }
        }
        Ok(UnboundedMultConfig { party_batches })
    }

    fn initialize(
        &self,
        party_id: PartyId,
        config: &Self::PrepareOutput,
    ) -> Result<InitializedProtocol<Self::State>, Error> {
        let batches =
            config.party_batches.get(&party_id).ok_or_else(|| anyhow!("party {party_id:?} batches not found"))?;
        let parties = config.party_batches.keys().cloned().collect();
        let secret_sharer = ShamirSecretSharer::new(party_id, self.polynomial_degree, parties)?;
        let (state, messages) = UnboundedMultState::new(batches.clone(), Arc::new(secret_sharer))?;
        Ok(InitializedProtocol::new(state, messages))
    }
}

struct UnboundedMultConfig<T: SafePrime> {
    party_batches: HashMap<PartyId, Batches<ModularNumber<T>>>,
}

#[test]
fn end_to_end() {
    let polynomial_degree = 1;
    let max_rounds = 100;
    let network_size = 5;
    let multiplication_batches = Batches::from(vec![
        vec![
            ModularNumber::from_u32(5),
            ModularNumber::two(),
            ModularNumber::from_u32(10),
            ModularNumber::from_u32(8),
            ModularNumber::from_u32(3),
            ModularNumber::from_u32(1600),
            ModularNumber::from_u32(98),
        ],
        vec![ModularNumber::from_u32(13), ModularNumber::from_u32(9)],
    ]);

    let simulator = SymmetricProtocolSimulator::new(network_size, max_rounds);
    let protocol = UnboundedMultProtocol::<U256SafePrime>::new(polynomial_degree, multiplication_batches);
    let outputs = simulator.run_protocol(&protocol).expect("protocol run failed");
    let mut party_shares = HashMap::new();
    for output in outputs {
        match output.output {
            UnboundedMultStateOutput::Success { outputs } => {
                party_shares.insert(output.party_id, outputs);
            }
            UnboundedMultStateOutput::InvRanAbort => {
                // This can fail by chance and should be retried. Until we have deterministic tests, this won't
                // be considered a failure condition.
                println!("INV-RAN abort!");
                return;
            }
        };
    }

    protocol.validate_output(party_shares).expect("validation failed");
}

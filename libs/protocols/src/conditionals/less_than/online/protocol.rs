//! Implementation the COMPARE protocol to be run under `simulator::SymmetricProtocolSimulator`

use super::state::{Comparands, CompareState};
use crate::{
    conditionals::less_than::offline::{validation::PrepCompareSharesBuilder, PrepCompareShares},
    simulator::symmetric::{InitializedProtocol, Protocol},
};
use anyhow::{anyhow, Error};
use math_lib::{
    fields::PrimeField,
    modular::{ModularNumber, SafePrime},
    polynomial::{point::Point, point_sequence::PointSequence},
};
use shamir_sharing::{
    party::{PartyId, PartyMapper},
    protocol::{PolyDegree, Shamir},
    secret_sharer::{PartyShares, SafePrimeSecretSharer, ShamirSecretSharer},
};
use std::sync::Arc;

/// The COMPARE protocol.
///
/// This is only meant to be used under a simulator, be it for testing or benchmarking purposes.
pub struct CompareProtocol<T: SafePrime> {
    comparands: Vec<(ModularNumber<T>, ModularNumber<T>)>,
    polynomial_degree: u64,
}

impl<T> CompareProtocol<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Constructs a new COMPARE protocol.
    pub fn new(comparands: Vec<(ModularNumber<T>, ModularNumber<T>)>, polynomial_degree: u64) -> Self {
        Self { comparands, polynomial_degree }
    }

    /// Validates the output of COMPARE protocol.
    pub fn validate_output(&self, party_shares: PartyShares<Vec<ModularNumber<T>>>) -> Result<(), Error> {
        let mapper = PartyMapper::<PrimeField<T>>::new(party_shares.keys().cloned().collect())?;
        let mut point_sequences = vec![PointSequence::<PrimeField<T>>::default(); self.comparands.len()];
        for (party_id, party_shares) in party_shares {
            if party_shares.len() != self.comparands.len() {
                return Err(anyhow!(
                    "unexpected element share count: expected {}, got {}",
                    self.comparands.len(),
                    party_shares.len()
                ));
            }
            let x =
                *mapper.abscissa(&party_id).ok_or_else(|| anyhow!("failed to find abscissa for party {party_id:?}"))?;

            for (element_index, share) in party_shares.into_iter().enumerate() {
                point_sequences[element_index].push(Point::new(x, share));
            }
        }

        let zipped = point_sequences.into_iter().zip(self.comparands.iter());
        for (point_sequence, (left, right)) in zipped {
            let comparison_output = point_sequence.lagrange_interpolate()?;
            let expected_value = (left < right) as u32;
            assert_eq!(
                comparison_output,
                ModularNumber::from_u32(expected_value),
                "failed for {} vs {}",
                left.into_value(),
                right.into_value()
            );
        }

        Ok(())
    }

    fn create_prep_compare_shares(
        &self,
        parties: &[PartyId],
        count: usize,
    ) -> Result<PartyShares<Vec<PrepCompareShares<T>>>, Error> {
        let sharer = ShamirSecretSharer::new(parties[0].clone(), self.polynomial_degree, parties.to_vec())?;
        let builder = PrepCompareSharesBuilder::new(&sharer, rand::thread_rng())?;
        builder.build(count)
    }
}

impl<T> Protocol for CompareProtocol<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    type State = CompareState<T>;
    type PrepareOutput = CompareConfig<T>;

    fn prepare(&self, parties: &[PartyId]) -> Result<Self::PrepareOutput, Error> {
        let parties = parties.to_vec();
        let mapper = PartyMapper::<PrimeField<T>>::new(parties.clone())?;
        // Note: the party id doesn't matter in this context
        let shamir = Shamir::<PrimeField<T>>::new(PartyId::from(0), self.polynomial_degree, parties.clone())?;

        // We're using 2 sets of parties here: _ours_ and the ones that PREP-COMPARE ran with.
        // Because of this, we need to map our parties into an abscissa and into a party id in the
        // PREP-COMPARE party set.
        let prep_compare_shares = self
            .create_prep_compare_shares(&parties, self.comparands.len())
            .map_err(|e| anyhow!("PREP-COMPARE share creation failed: {e}"))?;
        let mut party_comparand_shares: PartyShares<Vec<Comparands<T>>> = PartyShares::default();
        for (index, (left, right)) in self.comparands.iter().enumerate() {
            let left_shares = shamir.generate_shares(left, PolyDegree::T)?;
            let right_shares = shamir.generate_shares(right, PolyDegree::T)?;
            let zipped = left_shares.into_points().into_iter().zip(right_shares.into_points().into_iter());
            for (left_share_point, right_share_point) in zipped {
                let (_, left_share) = left_share_point.into_coordinates();
                let (x, right_share) = right_share_point.into_coordinates();
                let party_id = mapper.party(&x).ok_or_else(|| anyhow!("party id for {x:?} not found"))?;
                let prep_elements =
                    prep_compare_shares.get(party_id).ok_or_else(|| anyhow!("shares for {party_id} not found"))?;
                let comparands =
                    Comparands { left: left_share, right: right_share, prep_elements: prep_elements[index].clone() };
                party_comparand_shares.entry(party_id.clone()).or_default().push(comparands);
            }
        }
        Ok(CompareConfig { parties, party_comparand_shares })
    }

    fn initialize(
        &self,
        party_id: PartyId,
        config: &Self::PrepareOutput,
    ) -> Result<InitializedProtocol<Self::State>, Error> {
        let comparands = config
            .party_comparand_shares
            .get(&party_id)
            .cloned()
            .ok_or_else(|| anyhow!("shares for party {party_id:?}"))?;
        let secret_sharer = ShamirSecretSharer::new(party_id, self.polynomial_degree, config.parties.clone())?;
        let (state, initial_messages) = CompareState::new(comparands, Arc::new(secret_sharer))?;
        Ok(InitializedProtocol::new(state, initial_messages))
    }
}

/// The internal configuration of an COMPARE protocol.
pub struct CompareConfig<T: SafePrime> {
    parties: Vec<PartyId>,
    party_comparand_shares: PartyShares<Vec<Comparands<T>>>,
}

//! Implementation of the MULTIPLICATION-AND-TRUNCATION protocol to be run under `simulator::SymmetricProtocolSimulator`

#![allow(clippy::arithmetic_side_effects, clippy::indexing_slicing)]

use super::state::{MultTruncShares, MultTruncState};
use crate::{
    division::truncation_probabilistic::offline::{validation::PrepTruncPrSharesBuilder, PrepTruncPrShares},
    simulator::symmetric::{InitializedProtocol, Protocol},
};
use anyhow::{anyhow, Error};
use math_lib::{
    fields::PrimeField,
    modular::{ModularNumber, SafePrime},
};
use shamir_sharing::{
    party::{PartyId, PartyMapper},
    protocol::{PolyDegree, Shamir},
    secret_sharer::{PartyShares, SafePrimeSecretSharer, ShamirSecretSharer},
};
use std::sync::Arc;

/// The Multiplication-Truncation protocol.
///
/// This is only meant to be used under a simulator, be it for testing or benchmarking purposes.
pub struct MultTruncProtocol<T: SafePrime> {
    pub(crate) operands: Vec<(ModularNumber<T>, ModularNumber<T>)>,
    polynomial_degree: u64,
    kappa: usize,
    k: usize,
    pub(crate) trunc_exponent: usize,
}

impl<T> MultTruncProtocol<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Constructs a new MULTIPLICATION-AND-TRUNCATION protocol.
    pub fn new(
        operands: Vec<(ModularNumber<T>, ModularNumber<T>)>,
        polynomial_degree: u64,
        kappa: usize,
        k: usize,
    ) -> Self {
        Self { operands, polynomial_degree, kappa, k, trunc_exponent: k / 2 }
    }

    /// Utility to create PREP-TRUNCPR shares
    pub(crate) fn create_prep_truncpr_shares(
        &self,
        parties: &[PartyId],
        count: usize,
    ) -> Result<PartyShares<Vec<PrepTruncPrShares<T>>>, Error> {
        #![allow(clippy::indexing_slicing)]
        let sharer = ShamirSecretSharer::new(parties[0].clone(), self.polynomial_degree, parties.to_vec())?;
        let builder = PrepTruncPrSharesBuilder::new(&sharer, self.k, self.kappa)?;
        builder.build(count)
    }
}
impl<T> Protocol for MultTruncProtocol<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    type State = MultTruncState<T>;
    type PrepareOutput = MultTruncConfig<T>;

    fn prepare(&self, parties: &[PartyId]) -> Result<Self::PrepareOutput, Error> {
        let parties = parties.to_vec();
        let mapper = PartyMapper::<PrimeField<T>>::new(parties.clone())?;
        // Note: the party id doesn't matter in this context
        let shamir = Shamir::<PrimeField<T>>::new(PartyId::from(0), self.polynomial_degree, parties.clone())?;

        let prep_truncpr_shares = self
            .create_prep_truncpr_shares(&parties, self.operands.len())
            .map_err(|e| anyhow!("PREP-TRUNCPR share creation failed: {e}"))?;
        let mut party_truncpr_shares: PartyShares<Vec<MultTruncShares<T>>> = PartyShares::default();
        for (index, (left, right)) in self.operands.iter().enumerate() {
            let left_shares = shamir.generate_shares(left, PolyDegree::T)?;
            let zipped = left_shares.into_points().into_iter();
            for left_share_point in zipped {
                let (x, left_share) = left_share_point.into_coordinates();
                let party_id = mapper.party(&x).ok_or_else(|| anyhow!("party id for {x:?} not found"))?;
                let prep_elements =
                    prep_truncpr_shares.get(party_id).ok_or_else(|| anyhow!("shares for {party_id} not found"))?;
                let mult_trunc_shares = MultTruncShares {
                    left: left_share,
                    right: *right,
                    prep_elements: prep_elements[index].clone(),
                    trunc_exponent: ModularNumber::from_u64(self.trunc_exponent as u64),
                };
                party_truncpr_shares.entry(party_id.clone()).or_default().push(mult_trunc_shares);
            }
        }
        Ok(MultTruncConfig { parties, party_shares: party_truncpr_shares })
    }

    fn initialize(
        &self,
        party_id: PartyId,
        config: &Self::PrepareOutput,
    ) -> Result<InitializedProtocol<Self::State>, Error> {
        let mult_trunc_shares =
            config.party_shares.get(&party_id).cloned().ok_or_else(|| anyhow!("shares for party {party_id:?}"))?;
        let secret_sharer = ShamirSecretSharer::new(party_id, self.polynomial_degree, config.parties.clone())?;
        let (state, initial_messages) =
            MultTruncState::new(mult_trunc_shares, Arc::new(secret_sharer), self.kappa, self.k)?;
        Ok(InitializedProtocol::new(state, initial_messages))
    }
}

/// The internal configuration of a MULTIPLICATION-AND-TRUNCATION protocol.
pub struct MultTruncConfig<T: SafePrime> {
    parties: Vec<PartyId>,
    party_shares: PartyShares<Vec<MultTruncShares<T>>>,
}

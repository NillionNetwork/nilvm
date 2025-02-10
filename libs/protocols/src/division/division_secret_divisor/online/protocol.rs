//! Implementation of the DIV-INT-SECRET protocol to be run under `simulator::SymmetricProtocolSimulator`

use super::state::{DivisionIntegerSecretDivisorShares, DivisionIntegerSecretDivisorState};
use crate::{
    division::division_secret_divisor::offline::{
        validation::PrepDivisionIntegerSecretSharesBuilder, PrepDivisionIntegerSecretShares,
    },
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

/// The Division Integer Secret Divisor protocol.
///
/// This is only meant to be used under a simulator, be it for testing or benchmarking purposes.
pub struct DivisionIntegerSecretDivisorProtocol<T: SafePrime> {
    pub(crate) dividend_divisor: Vec<(ModularNumber<T>, ModularNumber<T>)>,
    polynomial_degree: u64,
    kappa: usize,
    k: usize,
}

impl<T> DivisionIntegerSecretDivisorProtocol<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Constructs a new DIV-INT-SECRET protocol.
    pub fn new(
        dividend_divisor: Vec<(ModularNumber<T>, ModularNumber<T>)>,
        polynomial_degree: u64,
        kappa: usize,
        k: usize,
    ) -> Self {
        Self { dividend_divisor, polynomial_degree, kappa, k }
    }

    /// Utility to create PREP-DIV-INT-SECRET shares
    fn create_shares(
        &self,
        parties: &[PartyId],
        count: usize,
    ) -> Result<PartyShares<Vec<PrepDivisionIntegerSecretShares<T>>>, Error> {
        let sharer = ShamirSecretSharer::new(parties[0].clone(), self.polynomial_degree, parties.to_vec())?;
        let builder = PrepDivisionIntegerSecretSharesBuilder::new(&sharer, self.k, self.kappa)?;
        builder.build(count)
    }
}

impl<T> Protocol for DivisionIntegerSecretDivisorProtocol<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    type State = DivisionIntegerSecretDivisorState<T>;
    type PrepareOutput = DivisionConfig<T>;

    fn prepare(&self, parties: &[PartyId]) -> Result<Self::PrepareOutput, Error> {
        let parties = parties.to_vec();
        let mapper = PartyMapper::<PrimeField<T>>::new(parties.clone())?;
        // Note: the party id doesn't matter in this context
        let shamir = Shamir::<PrimeField<T>>::new(PartyId::from(0), self.polynomial_degree, parties.clone())?;

        let prep_element_shares = self
            .create_shares(&parties, self.dividend_divisor.len())
            .map_err(|e| anyhow!("PREP-DIV-INT-SECRET share creation failed: {e}"))?;
        let mut party_division_shares: PartyShares<Vec<DivisionIntegerSecretDivisorShares<T>>> = PartyShares::default();
        for (index, (dividend, divisor)) in self.dividend_divisor.iter().enumerate() {
            let dividend_shares = shamir.generate_shares(dividend, PolyDegree::T)?;
            let divisor_shares = shamir.generate_shares(divisor, PolyDegree::T)?;
            let zipped = dividend_shares.into_points().into_iter().zip(divisor_shares.into_points().into_iter());
            for (dividend_share_point, divisor_share_point) in zipped {
                let (_, dividend_share) = dividend_share_point.into_coordinates();
                let (x, divisor_share) = divisor_share_point.into_coordinates();
                let party_id = mapper.party(&x).ok_or_else(|| anyhow!("party id for {x:?} not found"))?;
                let prep_elements =
                    prep_element_shares.get(party_id).ok_or_else(|| anyhow!("shares for {party_id} not found"))?;
                let division_shares = DivisionIntegerSecretDivisorShares {
                    dividend: dividend_share,
                    divisor: divisor_share,
                    prep_elements: prep_elements[index].clone(),
                };
                party_division_shares.entry(party_id.clone()).or_default().push(division_shares);
            }
        }
        Ok(DivisionConfig { parties, party_division_shares })
    }

    fn initialize(
        &self,
        party_id: PartyId,
        config: &Self::PrepareOutput,
    ) -> Result<InitializedProtocol<Self::State>, Error> {
        let division_shares = config
            .party_division_shares
            .get(&party_id)
            .cloned()
            .ok_or_else(|| anyhow!("shares for party {party_id:?}"))?;
        let secret_sharer = ShamirSecretSharer::new(party_id, self.polynomial_degree, config.parties.clone())?;
        let (state, initial_messages) =
            DivisionIntegerSecretDivisorState::new(division_shares, Arc::new(secret_sharer), self.kappa, self.k)?;
        Ok(InitializedProtocol::new(state, initial_messages))
    }
}

/// The internal configuration of a DIVISION protocol.
pub struct DivisionConfig<T: SafePrime> {
    parties: Vec<PartyId>,
    party_division_shares: PartyShares<Vec<DivisionIntegerSecretDivisorShares<T>>>,
}

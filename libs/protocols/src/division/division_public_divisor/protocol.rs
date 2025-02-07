//! Implementation of the DIVISION protocol to be run under `simulator::SymmetricProtocolSimulator`

use crate::{
    division::modulo_public_divisor::offline::{validation::PrepModuloSharesBuilder, PrepModuloShares},
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

use super::state::{DivisionIntegerPublicDivisorShares, DivisionIntegerPublicDivisorState};

/// The Division Integer Public Divisor protocol.
///
/// This is only meant to be used under a simulator, be it for testing or benchmarking purposes.
pub struct DivisionIntegerPublicDivisorProtocol<T: SafePrime> {
    pub(crate) dividend_divisor: Vec<(ModularNumber<T>, ModularNumber<T>)>,
    polynomial_degree: u64,
    kappa: usize,
    k: usize,
}

impl<T> DivisionIntegerPublicDivisorProtocol<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Constructs a new DIVISION protocol.
    pub fn new(
        dividend_divisor: Vec<(ModularNumber<T>, ModularNumber<T>)>,
        polynomial_degree: u64,
        kappa: usize,
        k: usize,
    ) -> Self {
        Self { dividend_divisor, polynomial_degree, kappa, k }
    }

    /// Utility to create PREP-MODULO shares
    fn create_prep_modulo_shares(
        &self,
        parties: &[PartyId],
        count: usize,
    ) -> Result<PartyShares<Vec<PrepModuloShares<T>>>, Error> {
        let sharer = ShamirSecretSharer::new(parties[0].clone(), self.polynomial_degree, parties.to_vec())?;
        let builder = PrepModuloSharesBuilder::new(&sharer, self.k, self.kappa)?;
        builder.build(count)
    }
}

impl<T> Protocol for DivisionIntegerPublicDivisorProtocol<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    type State = DivisionIntegerPublicDivisorState<T>;
    type PrepareOutput = DivisionConfig<T>;

    fn prepare(&self, parties: &[PartyId]) -> Result<Self::PrepareOutput, Error> {
        let parties = parties.to_vec();
        let mapper = PartyMapper::<PrimeField<T>>::new(parties.clone())?;
        // Note: the party id doesn't matter in this context
        let shamir = Shamir::<PrimeField<T>>::new(PartyId::from(0), self.polynomial_degree, parties.clone())?;

        // We're using 2 sets of parties here: _ours_ and the ones that PREP-COMPARE ran with.
        // Because of this, we need to map our parties into an abscissa and into a party id in the
        // PREP-COMPARE party set.
        let prep_modulo_shares = self
            .create_prep_modulo_shares(&parties, self.dividend_divisor.len())
            .map_err(|e| anyhow!("PREP-MODULO share creation failed: {e}"))?;
        let mut party_modulo_shares: PartyShares<Vec<DivisionIntegerPublicDivisorShares<T>>> = PartyShares::default();
        for (index, (dividend, divisor)) in self.dividend_divisor.iter().enumerate() {
            let dividend_shares = shamir.generate_shares(dividend, PolyDegree::T)?;
            let zipped = dividend_shares.into_points().into_iter();
            for dividend_share_point in zipped {
                let (x, dividend_share) = dividend_share_point.into_coordinates();
                let party_id = mapper.party(&x).ok_or_else(|| anyhow!("party id for {x:?} not found"))?;
                let prep_elements =
                    prep_modulo_shares.get(party_id).ok_or_else(|| anyhow!("shares for {party_id} not found"))?;
                let comparands = DivisionIntegerPublicDivisorShares {
                    dividend: dividend_share,
                    divisor: *divisor,
                    prep_elements: prep_elements[index].clone(),
                };
                party_modulo_shares.entry(party_id.clone()).or_default().push(comparands);
            }
        }
        Ok(DivisionConfig { parties, party_modulo_shares })
    }

    fn initialize(
        &self,
        party_id: PartyId,
        config: &Self::PrepareOutput,
    ) -> Result<InitializedProtocol<Self::State>, Error> {
        let comparands = config
            .party_modulo_shares
            .get(&party_id)
            .cloned()
            .ok_or_else(|| anyhow!("shares for party {party_id:?}"))?;
        let secret_sharer = ShamirSecretSharer::new(party_id, self.polynomial_degree, config.parties.clone())?;
        let (state, initial_messages) =
            DivisionIntegerPublicDivisorState::new(comparands, Arc::new(secret_sharer), self.kappa, self.k)?;
        Ok(InitializedProtocol::new(state, initial_messages))
    }
}

/// The internal configuration of a DIVISION protocol.
pub struct DivisionConfig<T: SafePrime> {
    parties: Vec<PartyId>,
    party_modulo_shares: PartyShares<Vec<DivisionIntegerPublicDivisorShares<T>>>,
}

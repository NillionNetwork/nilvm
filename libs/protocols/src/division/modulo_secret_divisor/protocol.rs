//! Implementation of the MODULO with secret divisor protocol to be run under `simulator::SymmetricProtocolSimulator`

use super::state::{ModuloIntegerSecretDivisorShares, ModuloIntegerSecretDivisorState};
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

/// The Modulo Integer Secret Divisor protocol.
///
/// This is only meant to be used under a simulator, be it for testing or benchmarking purposes.
pub struct ModuloIntegerSecretDivisorProtocol<T: SafePrime> {
    pub(crate) dividend_divisor: Vec<(ModularNumber<T>, ModularNumber<T>)>,
    polynomial_degree: u64,
    kappa: usize,
    k: usize,
}

impl<T> ModuloIntegerSecretDivisorProtocol<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Constructs a new Modulo with secret divisor protocol.
    pub fn new(
        dividend_divisor: Vec<(ModularNumber<T>, ModularNumber<T>)>,
        polynomial_degree: u64,
        kappa: usize,
        k: usize,
    ) -> Self {
        Self { dividend_divisor, polynomial_degree, kappa, k }
    }

    /// Utility to create PREP-DIV-INT-SECRET shares
    fn create_prep_division_shares(
        &self,
        parties: &[PartyId],
        count: usize,
    ) -> Result<PartyShares<Vec<PrepDivisionIntegerSecretShares<T>>>, Error> {
        let sharer = ShamirSecretSharer::new(parties[0].clone(), self.polynomial_degree, parties.to_vec())?;
        let builder = PrepDivisionIntegerSecretSharesBuilder::new(&sharer, self.k, self.kappa)?;
        builder.build(count)
    }
}

impl<T> Protocol for ModuloIntegerSecretDivisorProtocol<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    type State = ModuloIntegerSecretDivisorState<T>;
    type PrepareOutput = ModuloConfig<T>;

    /// This function is used to prepare the configuration of the operation (e.g. Operation) being simulated.
    /// If the protocol does not require any preprocessing element, then it simply outputs something like:
    ///
    /// ```
    /// Ok(OperationConfig { parties: parties.to_vec()})
    /// ```
    ///
    /// Otherwise, in case the protocol requires proprocessing elements, then we need to:
    /// 1. Create preprocessing elements: use a method called `create_prep_<preprocessing_element>_shares`;
    /// 2. Create shares of the inputs;
    /// 3. Map our parties from `parties` into an abscissa and into a party id in the preprocessing element.
    fn prepare(&self, parties: &[PartyId]) -> Result<Self::PrepareOutput, Error> {
        let parties = parties.to_vec();
        let mapper = PartyMapper::<PrimeField<T>>::new(parties.clone())?;
        // Note: the party id doesn't matter in this context
        let shamir = Shamir::<PrimeField<T>>::new(PartyId::from(0), self.polynomial_degree, parties.clone())?;

        // 1. Create preprocessing elements:
        let prep_division_shares = self
            .create_prep_division_shares(&parties, self.dividend_divisor.len())
            .map_err(|e| anyhow!("PREP-DIV-INT-SECRET share creation failed: {e}"))?;

        let mut party_modulo_shares: PartyShares<Vec<ModuloIntegerSecretDivisorShares<T>>> = PartyShares::default();
        for (index, (dividend, divisor)) in self.dividend_divisor.iter().enumerate() {
            // 2. Create shares of the inputs;
            let dividend_shares = shamir.generate_shares(dividend, PolyDegree::T)?;
            let divisor_shares = shamir.generate_shares(divisor, PolyDegree::T)?;
            let zipped = dividend_shares.into_points().into_iter().zip(divisor_shares.into_points().into_iter());
            // 3. Map our parties from `parties` into an abscissa and into a party id in the preprocessing element.
            //
            // We're using 2 sets of parties here: _ours_ and the ones that PREP-DIV-INT-SECRET ran with.
            // Because of this, we need to map our parties into an abscissa and into a party id in the
            // PREP-DIV-INT-SECRET party set.
            for (dividend_share_point, divisor_share_point) in zipped {
                let (_, dividend_share) = dividend_share_point.into_coordinates();
                let (x, divisor_share) = divisor_share_point.into_coordinates();
                let party_id = mapper.party(&x).ok_or_else(|| anyhow!("party id for {x:?} not found"))?;
                let prep_elements =
                    prep_division_shares.get(party_id).ok_or_else(|| anyhow!("shares for {party_id} not found"))?;
                let modulos = ModuloIntegerSecretDivisorShares {
                    dividend: dividend_share,
                    divisor: divisor_share,
                    prep_elements: prep_elements[index].clone(),
                };
                party_modulo_shares.entry(party_id.clone()).or_default().push(modulos);
            }
        }
        Ok(ModuloConfig { parties, party_modulo_shares })
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
            ModuloIntegerSecretDivisorState::new(comparands, Arc::new(secret_sharer), self.kappa, self.k)?;
        Ok(InitializedProtocol::new(state, initial_messages))
    }
}

/// The internal configuration of a Modulo with secret divisior protocol.
pub struct ModuloConfig<T: SafePrime> {
    parties: Vec<PartyId>,
    party_modulo_shares: PartyShares<Vec<ModuloIntegerSecretDivisorShares<T>>>,
}

//! Implementation the MODULO protocol to be run under `simulator::SymmetricProtocolSimulator`

use super::{
    super::offline::{validation::PrepModuloSharesBuilder, PrepModuloShares},
    state::{ModuloShares, ModuloState},
};
use crate::simulator::symmetric::{InitializedProtocol, Protocol};
use anyhow::{anyhow, Error};
use math_lib::{
    fields::PrimeField,
    modular::{FloorMod, ModularNumber, SafePrime},
    polynomial::{point::Point, point_sequence::PointSequence},
};
use num_bigint::BigInt;
use shamir_sharing::{
    party::{PartyId, PartyMapper},
    protocol::{PolyDegree, Shamir},
    secret_sharer::{PartyShares, SafePrimeSecretSharer, ShamirSecretSharer},
};
use std::sync::Arc;

impl<T> ModuloProtocol<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Validates the output of MODULO protocol.
    pub fn validate_output(&self, party_shares: PartyShares<Vec<ModularNumber<T>>>) -> Result<(), Error> {
        let mapper = PartyMapper::<PrimeField<T>>::new(party_shares.keys().cloned().collect())?;
        let mut point_sequences = vec![PointSequence::<PrimeField<T>>::default(); self.dividend_divisor.len()];
        for (party_id, party_shares) in party_shares {
            if party_shares.len() != self.dividend_divisor.len() {
                return Err(anyhow!(
                    "unexpected element share count: expected {}, got {}",
                    self.dividend_divisor.len(),
                    party_shares.len()
                ));
            }
            let x =
                *mapper.abscissa(&party_id).ok_or_else(|| anyhow!("failed to find abscissa for party {party_id:?}"))?;

            for (element_index, share) in party_shares.into_iter().enumerate() {
                point_sequences[element_index].push(Point::new(x, share));
            }
        }

        let zipped = point_sequences.into_iter().zip(self.dividend_divisor.iter());
        for (point_sequence, (dividend, divisor)) in zipped {
            let modulo_output = point_sequence.lagrange_interpolate()?;

            let expected_value = dividend.fmod(divisor).unwrap();
            println!(
                "dividend: {:?} divisor: {:?}, remainder: {:?}, expected: {:?}",
                BigInt::from(dividend),
                BigInt::from(divisor),
                BigInt::from(&modulo_output),
                BigInt::from(&expected_value)
            );
            assert_eq!(
                modulo_output,
                expected_value,
                "failed for {} % {}",
                dividend.into_value(),
                divisor.into_value()
            );
        }

        Ok(())
    }
}

/// The MODULO protocol.
///
/// This is only meant to be used under a simulator, be it for testing or benchmarking purposes.
pub struct ModuloProtocol<T: SafePrime> {
    pub(crate) dividend_divisor: Vec<(ModularNumber<T>, ModularNumber<T>)>,
    polynomial_degree: u64,
    kappa: usize,
    k: usize,
}

impl<T> ModuloProtocol<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Constructs a new MODULO protocol.
    pub fn new(
        dividend_divisor: Vec<(ModularNumber<T>, ModularNumber<T>)>,
        polynomial_degree: u64,
        kappa: usize,
        k: usize,
    ) -> Self {
        Self { dividend_divisor, polynomial_degree, kappa, k }
    }

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

impl<T> Protocol for ModuloProtocol<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    type State = ModuloState<T>;
    type PrepareOutput = ModuloConfig<T>;

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
        let mut party_modulo_shares: PartyShares<Vec<ModuloShares<T>>> = PartyShares::default();
        for (index, (dividend, divisor)) in self.dividend_divisor.iter().enumerate() {
            let dividend_shares = shamir.generate_shares(dividend, PolyDegree::T)?;
            let zipped = dividend_shares.into_points().into_iter();
            for dividend_share_point in zipped {
                let (x, dividend_share) = dividend_share_point.into_coordinates();
                let party_id = mapper.party(&x).ok_or_else(|| anyhow!("party id for {x:?} not found"))?;
                let prep_elements =
                    prep_modulo_shares.get(party_id).ok_or_else(|| anyhow!("shares for {party_id} not found"))?;
                let comparands = ModuloShares {
                    dividend: dividend_share,
                    divisor: *divisor,
                    prep_elements: prep_elements[index].clone(),
                };
                party_modulo_shares.entry(party_id.clone()).or_default().push(comparands);
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
        let (state, initial_messages) = ModuloState::new(comparands, Arc::new(secret_sharer), self.kappa, self.k)?;
        Ok(InitializedProtocol::new(state, initial_messages))
    }
}

/// The internal configuration of an COMPARE protocol.
pub struct ModuloConfig<T: SafePrime> {
    parties: Vec<PartyId>,
    party_modulo_shares: PartyShares<Vec<ModuloShares<T>>>,
}

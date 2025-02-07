//! Implementation of the MOD2M protocol to be run under `simulator::SymmetricProtocolSimulator`

use super::{
    super::offline::{validation::PrepModulo2mSharesBuilder, PrepModulo2mShares},
    state::{states::Mod2mTruncVariant, Modulo2mShares, Modulo2mState},
};
use crate::simulator::symmetric::{InitializedProtocol, Protocol};
use anyhow::{anyhow, Error};
use math_lib::{
    fields::PrimeField,
    modular::{ModularNumber, ModularPow, SafePrime},
    polynomial::{point::Point, point_sequence::PointSequence},
};
use shamir_sharing::{
    party::{PartyId, PartyMapper},
    protocol::{PolyDegree, Shamir},
    secret_sharer::{PartyShares, SafePrimeSecretSharer, ShamirSecretSharer},
};
use std::sync::Arc;

/// The MOD2M protocol.
///
/// This is only meant to be used under a simulator, be it for testing or benchmarking purposes.
pub struct Modulo2mProtocol<T: SafePrime> {
    dividend_exponent: Vec<(ModularNumber<T>, ModularNumber<T>)>,
    polynomial_degree: u64,
    kappa: usize,
    k: usize,
}

impl<T> Modulo2mProtocol<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Constructs a new MOD2M protocol.
    pub fn new(
        dividend_exponent: Vec<(ModularNumber<T>, ModularNumber<T>)>,
        polynomial_degree: u64,
        kappa: usize,
        k: usize,
    ) -> Self {
        Self { dividend_exponent, polynomial_degree, kappa, k }
    }

    /// Validates the output of MOD2M protocol.
    pub fn validate_output(&self, party_shares: PartyShares<Vec<ModularNumber<T>>>) -> Result<(), Error> {
        let mapper = PartyMapper::<PrimeField<T>>::new(party_shares.keys().cloned().collect())?;
        let mut point_sequences = vec![PointSequence::<PrimeField<T>>::default(); self.dividend_exponent.len()];
        for (party_id, party_shares) in party_shares {
            if party_shares.len() != self.dividend_exponent.len() {
                return Err(anyhow!(
                    "unexpected element share count: expected {}, got {}",
                    self.dividend_exponent.len(),
                    party_shares.len()
                ));
            }
            let x =
                *mapper.abscissa(&party_id).ok_or_else(|| anyhow!("failed to find abscissa for party {party_id:?}"))?;

            for (element_index, share) in party_shares.into_iter().enumerate() {
                point_sequences[element_index].push(Point::new(x, share));
            }
        }

        let two = ModularNumber::two();
        let zipped = point_sequences.into_iter().zip(self.dividend_exponent.iter());
        for (point_sequence, (dividend, divexpm)) in zipped {
            let modulo_output = point_sequence.lagrange_interpolate()?;
            let two_m = two.exp_mod(&divexpm.into_value());
            let expected_value = (dividend % &two_m).expect("failed to compute clear modulo");
            assert_eq!(
                modulo_output,
                expected_value,
                "failed for {} % 2^{}",
                dividend.into_value(),
                divexpm.into_value()
            );
        }

        Ok(())
    }

    fn create_prep_modulo_shares(
        &self,
        parties: &[PartyId],
        count: usize,
    ) -> Result<PartyShares<Vec<PrepModulo2mShares<T>>>, Error> {
        let sharer = ShamirSecretSharer::new(parties[0].clone(), self.polynomial_degree, parties.to_vec())?;
        let builder = PrepModulo2mSharesBuilder::new(&sharer, self.k, self.kappa)?;
        builder.build(count)
    }

    fn initialize_generic(
        &self,
        party_id: PartyId,
        config: &ModuloConfig<T>,
        protocol_variant: Mod2mTruncVariant,
    ) -> Result<InitializedProtocol<Modulo2mState<T>>, Error> {
        let comparands = config
            .party_modulo_shares
            .get(&party_id)
            .cloned()
            .ok_or_else(|| anyhow!("shares for party {party_id:?}"))?;
        let secret_sharer = ShamirSecretSharer::new(party_id, self.polynomial_degree, config.parties.clone())?;
        let (state, initial_messages) =
            Modulo2mState::new(comparands, Arc::new(secret_sharer), self.kappa, self.k, protocol_variant)?;
        Ok(InitializedProtocol::new(state, initial_messages))
    }
}

impl<T> Protocol for Modulo2mProtocol<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    type State = Modulo2mState<T>;
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
            .create_prep_modulo_shares(&parties, self.dividend_exponent.len())
            .map_err(|e| anyhow!("PREP-MOD2M share creation failed: {e}"))?;
        let mut party_modulo_shares: PartyShares<Vec<Modulo2mShares<T>>> = PartyShares::default();
        for (index, (dividend, divexpm)) in self.dividend_exponent.iter().enumerate() {
            let dividend_shares = shamir.generate_shares(dividend, PolyDegree::T)?;
            let zipped = dividend_shares.into_points().into_iter();
            for dividend_share_point in zipped {
                let (x, dividend_share) = dividend_share_point.into_coordinates();
                let party_id = mapper.party(&x).ok_or_else(|| anyhow!("party id for {x:?} not found"))?;
                let prep_elements =
                    prep_modulo_shares.get(party_id).ok_or_else(|| anyhow!("shares for {party_id} not found"))?;
                let comparands = Modulo2mShares {
                    dividend: dividend_share,
                    divisors_exp_m: *divexpm,
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
        self.initialize_generic(party_id, config, Mod2mTruncVariant::Mod2m)
    }
}

/// The TRUNC protocol.
///
/// This is only meant to be used under a simulator, be it for testing or benchmarking purposes.
pub struct TruncProtocol<T: SafePrime> {
    mod2m: Modulo2mProtocol<T>,
}

impl<T> TruncProtocol<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Constructs a new TRUNC protocol.
    pub fn new(
        dividend_exponent: Vec<(ModularNumber<T>, ModularNumber<T>)>,
        polynomial_degree: u64,
        kappa: usize,
        k: usize,
    ) -> Self {
        Self { mod2m: Modulo2mProtocol { dividend_exponent, polynomial_degree, kappa, k } }
    }

    /// Validates the output of TRUNC protocol.
    pub fn validate_output(&self, party_shares: PartyShares<Vec<ModularNumber<T>>>) -> Result<(), Error> {
        let mapper = PartyMapper::<PrimeField<T>>::new(party_shares.keys().cloned().collect())?;
        let mut point_sequences = vec![PointSequence::<PrimeField<T>>::default(); self.mod2m.dividend_exponent.len()];
        for (party_id, party_shares) in party_shares {
            if party_shares.len() != self.mod2m.dividend_exponent.len() {
                return Err(anyhow!(
                    "unexpected element share count: expected {}, got {}",
                    self.mod2m.dividend_exponent.len(),
                    party_shares.len()
                ));
            }
            let x =
                *mapper.abscissa(&party_id).ok_or_else(|| anyhow!("failed to find abscissa for party {party_id:?}"))?;

            for (element_index, share) in party_shares.into_iter().enumerate() {
                point_sequences[element_index].push(Point::new(x, share));
            }
        }

        let zipped = point_sequences.into_iter().zip(self.mod2m.dividend_exponent.iter());
        for (point_sequence, (dividend, divexpm)) in zipped {
            let trunc_output = point_sequence.lagrange_interpolate()?;
            let expected_value = (dividend >> &divexpm).expect("failed to compute clear modulo");
            assert_eq!(
                trunc_output,
                expected_value,
                "failed for {} >> {}",
                dividend.into_value(),
                divexpm.into_value()
            );
        }

        Ok(())
    }
}

impl<T> Protocol for TruncProtocol<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    type State = Modulo2mState<T>;
    type PrepareOutput = ModuloConfig<T>;

    fn prepare(&self, parties: &[PartyId]) -> Result<Self::PrepareOutput, Error> {
        self.mod2m.prepare(parties)
    }

    fn initialize(
        &self,
        party_id: PartyId,
        config: &Self::PrepareOutput,
    ) -> Result<InitializedProtocol<Self::State>, Error> {
        self.mod2m.initialize_generic(party_id, config, Mod2mTruncVariant::Trunc)
    }
}

/// The internal configuration of an COMPARE protocol.
pub struct ModuloConfig<T: SafePrime> {
    parties: Vec<PartyId>,
    party_modulo_shares: PartyShares<Vec<Modulo2mShares<T>>>,
}

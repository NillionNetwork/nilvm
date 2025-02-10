//! Implementation of the TRUNCPR protocol to be run under [`crate::simulator::SymmetricProtocolSimulator`]

use crate::simulator::symmetric::{InitializedProtocol, Protocol};
use anyhow::{anyhow, Error};
use math_lib::{
    fields::PrimeField,
    modular::{ModularNumber, SafePrime},
    polynomial::{point::Point, point_sequence::PointSequence},
};
use num_bigint::BigInt;
use shamir_sharing::{
    party::{PartyId, PartyMapper},
    protocol::{PolyDegree, Shamir},
    secret_sharer::{PartyShares, SafePrimeSecretSharer, ShamirSecretSharer},
};
use std::sync::Arc;

use super::super::offline::{validation::PrepTruncPrSharesBuilder, PrepTruncPrShares};

use super::state::{TruncPrShares, TruncPrState};

/// The TRUNCPR protocol.
///
/// This is only meant to be used under a simulator, be it for testing or benchmarking purposes.
pub struct TruncPrProtocol<T: SafePrime> {
    dividend_exponent: Vec<(ModularNumber<T>, ModularNumber<T>)>,
    polynomial_degree: u64,
    kappa: usize,
    k: usize,
}

impl<T> TruncPrProtocol<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Constructs a new TRUNCPR protocol.
    pub fn new(
        dividend_exponent: Vec<(ModularNumber<T>, ModularNumber<T>)>,
        polynomial_degree: u64,
        kappa: usize,
        k: usize,
    ) -> Self {
        Self { dividend_exponent, polynomial_degree, kappa, k }
    }

    /// Validates the output of TRUNC protocol.
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
                point_sequences
                    .get_mut(element_index)
                    .ok_or(anyhow!("Point sequence not found for {element_index}"))?
                    .push(Point::new(x, share));
            }
        }

        let zipped = point_sequences.into_iter().zip(self.dividend_exponent.iter());
        for (point_sequence, (dividend, divexpm)) in zipped {
            let trunc_output = point_sequence.lagrange_interpolate()?;
            let expected_value = (dividend >> divexpm).map_err(|_| anyhow!("failed to compute clear truncation"))?;
            assert!(
                (expected_value - &trunc_output) <= ModularNumber::ONE,
                "failed for {} >> {}, expected: {}, actual: {}",
                BigInt::from(dividend),
                BigInt::from(divexpm),
                BigInt::from(&expected_value),
                BigInt::from(&trunc_output),
            );
        }

        Ok(())
    }

    fn create_prep_truncpr_shares(
        &self,
        parties: &[PartyId],
        count: usize,
    ) -> Result<PartyShares<Vec<PrepTruncPrShares<T>>>, Error> {
        let sharer = ShamirSecretSharer::new(
            parties.first().ok_or(anyhow!("unable to find party"))?.clone(),
            self.polynomial_degree,
            parties.to_vec(),
        )?;
        let builder = PrepTruncPrSharesBuilder::new(&sharer, self.k, self.kappa)?;
        builder.build(count)
    }
}

impl<T> Protocol for TruncPrProtocol<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    type State = TruncPrState<T>;
    type PrepareOutput = TruncPrConfig<T>;

    fn prepare(&self, parties: &[PartyId]) -> Result<Self::PrepareOutput, Error> {
        let parties = parties.to_vec();
        let mapper = PartyMapper::<PrimeField<T>>::new(parties.clone())?;
        // Note: the party id doesn't matter in this context
        let shamir = Shamir::<PrimeField<T>>::new(PartyId::from(0), self.polynomial_degree, parties.clone())?;

        // We're using 2 sets of parties here: _ours_ and the ones that PREP-TRUNCPR ran with.
        // Because of this, we need to map our parties into an abscissa and into a party id in the
        // PREP-TRUNCPR party set.
        let prep_truncpr_shares = self
            .create_prep_truncpr_shares(&parties, self.dividend_exponent.len())
            .map_err(|e| anyhow!("PREP-TRUNCPR share creation failed: {e}"))?;
        let mut party_shares: PartyShares<Vec<TruncPrShares<T>>> = PartyShares::default();
        for (index, (dividend, divexpm)) in self.dividend_exponent.iter().enumerate() {
            let dividend_shares = shamir.generate_shares(dividend, PolyDegree::T)?;
            let zipped = dividend_shares.into_points().into_iter();
            for dividend_share_point in zipped {
                let (x, dividend_share) = dividend_share_point.into_coordinates();
                let party_id = mapper.party(&x).ok_or_else(|| anyhow!("party id for {x:?} not found"))?;
                let prep_elements =
                    prep_truncpr_shares.get(party_id).ok_or_else(|| anyhow!("shares for {party_id} not found"))?;
                let comparands = TruncPrShares {
                    dividend: dividend_share,
                    divisors_exp_m: *divexpm,
                    prep_elements: prep_elements
                        .get(index)
                        .ok_or(anyhow!("unable to find preprocessing elements with index {index}"))?
                        .clone(),
                };
                party_shares.entry(party_id.clone()).or_default().push(comparands);
            }
        }
        Ok(TruncPrConfig { parties, party_shares })
    }

    fn initialize(
        &self,
        party_id: PartyId,
        config: &Self::PrepareOutput,
    ) -> Result<InitializedProtocol<Self::State>, Error> {
        let comparands =
            config.party_shares.get(&party_id).cloned().ok_or_else(|| anyhow!("shares for party {party_id:?}"))?;
        let secret_sharer = ShamirSecretSharer::new(party_id, self.polynomial_degree, config.parties.clone())?;
        let (state, initial_messages) = TruncPrState::new(comparands, Arc::new(secret_sharer), self.kappa, self.k)?;
        Ok(InitializedProtocol::new(state, initial_messages))
    }
}

/// The internal configuration of an TRUNCPR protocol.
pub struct TruncPrConfig<T: SafePrime> {
    parties: Vec<PartyId>,
    party_shares: PartyShares<Vec<TruncPrShares<T>>>,
}

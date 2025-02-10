//! Implementation the QUATERNARY-LESS-THAN protocol to be run under `simulator::SymmetricProtocolSimulator`

use super::state::{QuatComparands, QuatLessState};
use crate::{
    random::random_quaternary::QuatShare,
    simulator::symmetric::{InitializedProtocol, Protocol},
};
use anyhow::{anyhow, Error};
use math_lib::{
    fields::PrimeField,
    modular::{AsBits, ModularNumber, SafePrime},
    polynomial::{point::Point, point_sequence::PointSequence},
};
use shamir_sharing::{
    party::{PartyId, PartyMapper},
    protocol::{PolyDegree, Shamir},
    secret_sharer::{PartyShares, SafePrimeSecretSharer, ShamirSecretSharer},
};
use std::sync::Arc;

/// The QUATERNARY-LESS-THAN protocol.
///
/// This is only meant to be used under a simulator, be it for testing or benchmarking purposes.
pub struct QuatLessProtocol<T: SafePrime> {
    comparands: Vec<(ModularNumber<T>, ModularNumber<T>)>,
    polynomial_degree: u64,
}

impl<T> QuatLessProtocol<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Constructs a new QUATERNARY-LESS-THAN protocol.
    pub fn new(comparands: Vec<(ModularNumber<T>, ModularNumber<T>)>, polynomial_degree: u64) -> Self {
        Self { comparands, polynomial_degree }
    }

    /// Validates the output of QUATERNARY-LESS-THAN protocol.
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
}

impl<T> Protocol for QuatLessProtocol<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    type State = QuatLessState<T>;
    type PrepareOutput = QuatLessConfig<T>;

    fn prepare(&self, parties: &[PartyId]) -> Result<Self::PrepareOutput, Error> {
        let parties = parties.to_vec();
        let mapper = PartyMapper::<PrimeField<T>>::new(parties.clone())?;
        // Note: the party id doesn't matter in this context
        let shamir = Shamir::<PrimeField<T>>::new(PartyId::from(0), self.polynomial_degree, parties.clone())?;

        // We're using 2 sets of parties here: _ours_ and the ones that PREP-QUATERNARY-LESS-THAN ran with.
        // Because of this, we need to map our parties into an abscissa and into a party id in the
        // PREP-QUATERNARY-LESS-THAN party set.
        let mut party_comparand_shares: PartyShares<Vec<QuatComparands<T>>> = PartyShares::default();
        for (public, secret) in self.comparands.iter() {
            let secret = secret.into_value();
            let mut quat_shares: PartyShares<Vec<QuatShare<T>>> = PartyShares::default();
            let bits = std::cmp::max(secret.bits(), public.into_value().bits());
            let bits = std::cmp::max(bits, 4);
            for i in 0..(bits + 1) / 2 {
                let low = ModularNumber::from_u32(secret.bit(2 * i) as u32);
                let high = ModularNumber::from_u32(secret.bit(2 * i + 1) as u32);
                let cross = low * &high;
                let low_shares = shamir.generate_shares(&low, PolyDegree::T)?;
                let high_shares = shamir.generate_shares(&high, PolyDegree::T)?;
                let cross_shares = shamir.generate_shares(&cross, PolyDegree::T)?;
                let zipped = low_shares
                    .into_points()
                    .into_iter()
                    .zip(high_shares.into_points().into_iter())
                    .zip(cross_shares.into_points().into_iter());
                for ((low_point, high_point), cross_point) in zipped {
                    let (_, low_share) = low_point.into_coordinates();
                    let (_, high_share) = high_point.into_coordinates();
                    let (x, cross_share) = cross_point.into_coordinates();
                    let party_id = mapper.party(&x).ok_or_else(|| anyhow!("party id for {x:?} not found"))?;
                    let quat = QuatShare::new(low_share, high_share, cross_share);
                    quat_shares.entry(party_id.clone()).or_default().push(quat);
                }
            }
            for (party_id, quats) in quat_shares.into_iter() {
                let comparands = QuatComparands { secret: quats.into(), public: *public };
                party_comparand_shares.entry(party_id.clone()).or_default().push(comparands);
            }
        }
        Ok(QuatLessConfig { parties, party_comparand_shares })
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
        let (state, initial_messages) = QuatLessState::new(comparands, Arc::new(secret_sharer))?;
        Ok(InitializedProtocol::new(state, initial_messages))
    }
}

/// The internal configuration of an QUATERNARY-LESS-THAN protocol.
pub struct QuatLessConfig<T: SafePrime> {
    parties: Vec<PartyId>,
    party_comparand_shares: PartyShares<Vec<QuatComparands<T>>>,
}

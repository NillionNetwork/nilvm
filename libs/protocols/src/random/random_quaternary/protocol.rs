//! Implementation the RAN-QUATERNARY protocol to be run under `simulator::SymmetricProtocolSimulator`

#![allow(clippy::arithmetic_side_effects, clippy::panic, clippy::indexing_slicing)]

use crate::{
    random::{
        random_bitwise::RanBitwiseMode,
        random_quaternary::{state::RanQuaternaryState, QuaternaryShares},
    },
    simulator::symmetric::{InitializedProtocol, Protocol},
};
use anyhow::{anyhow, Error};
use math_lib::{
    fields::PrimeField,
    modular::{Integer, SafePrime},
    polynomial::{point::Point, point_sequence::PointSequence},
};
use shamir_sharing::{
    party::{PartyId, PartyMapper},
    secret_sharer::{PartyShares, SafePrimeSecretSharer, ShamirSecretSharer},
};
use std::{marker::PhantomData, sync::Arc};

/// The RAN-QUATERNARY protocol.
///
/// This is only meant to be used under a simulator, be it for testing or benchmarking purposes.
pub struct RanQuaternaryProtocol<T> {
    element_count: usize,
    polynomial_degree: u64,
    _unused: PhantomData<T>,
}

impl<T> RanQuaternaryProtocol<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Constructs a new RAN-QUATERNARY protocol.
    pub fn new(element_count: usize, polynomial_degree: u64) -> Self {
        Self { element_count, polynomial_degree, _unused: Default::default() }
    }

    /// Validates the output to make sure it is correct.
    pub fn validate_output(&self, party_shares: PartyShares<Vec<QuaternaryShares<T>>>) -> Result<(), Error> {
        let prime_bits = T::Normal::BITS;
        let mapper = PartyMapper::<PrimeField<T>>::new(party_shares.keys().cloned().collect())?;

        // We have N vecs one per element count, which has M vecs one per bit, which is a point
        // sequence with a point per party.
        let mut low_sequences =
            vec![vec![PointSequence::<PrimeField<T>>::default(); (prime_bits + 1) / 2]; self.element_count];
        let mut high_sequences =
            vec![vec![PointSequence::<PrimeField<T>>::default(); (prime_bits + 1) / 2]; self.element_count];
        let mut cross_sequences =
            vec![vec![PointSequence::<PrimeField<T>>::default(); (prime_bits + 1) / 2]; self.element_count];
        for (party_id, party_shares) in party_shares {
            if party_shares.len() != self.element_count {
                return Err(anyhow!("unexpected element share count"));
            }

            let x =
                *mapper.abscissa(&party_id).ok_or_else(|| anyhow!("failed to find abscissa for party {party_id:?}"))?;
            for (element_index, quaternary_shares) in party_shares.into_iter().enumerate() {
                let shares = Vec::from(quaternary_shares);
                if shares.len() != (prime_bits + 1) / 2 {
                    return Err(anyhow!("unexpected bit share count"));
                }
                for (index, share) in shares.into_iter().enumerate() {
                    let (low, high, cross) = share.as_parts();
                    low_sequences[element_index][index].push(Point::new(x, *low));
                    high_sequences[element_index][index].push(Point::new(x, *high));
                    cross_sequences[element_index][index].push(Point::new(x, *cross));
                }
            }
        }

        for ((lows, highs), crosses) in low_sequences.iter().zip(high_sequences.iter()).zip(cross_sequences.iter()) {
            for ((low, high), cross) in lows.iter().zip(highs.iter()).zip(crosses.iter()) {
                let low = low.lagrange_interpolate().expect("interpolation failed");
                let high = high.lagrange_interpolate().expect("interpolation failed");
                let product = low * &high;
                let expected = cross.lagrange_interpolate().expect("interpolation failed");
                assert_eq!(product, expected);
            }
        }

        Ok(())
    }
}

impl<T> Protocol for RanQuaternaryProtocol<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    type State = RanQuaternaryState<T>;
    type PrepareOutput = RanQuaternaryConfig;

    fn prepare(&self, parties: &[PartyId]) -> Result<Self::PrepareOutput, Error> {
        let parties = parties.to_vec();
        Ok(RanQuaternaryConfig { parties })
    }

    fn initialize(
        &self,
        party_id: PartyId,
        config: &Self::PrepareOutput,
    ) -> Result<InitializedProtocol<RanQuaternaryState<T>>, Error> {
        let secret_sharer = ShamirSecretSharer::new(party_id, self.polynomial_degree, config.parties.clone())?;
        let (state, initial_messages) =
            RanQuaternaryState::new(RanBitwiseMode::Full, self.element_count, Arc::new(secret_sharer))?;
        Ok(InitializedProtocol::new(state, initial_messages))
    }
}

/// The internal configuration of an RAN-QUATERNARY protocol.
pub struct RanQuaternaryConfig {
    parties: Vec<PartyId>,
}

//! Validator for the PREP-COMPARE protocol.

// This is only meant to be used for testing so panic'ing is fine.
#![allow(clippy::indexing_slicing, clippy::arithmetic_side_effects, clippy::panic, clippy::unwrap_used)]

use super::PrepCompareShares;
use crate::{
    multiplication::multiplication_unbounded::prefix::PrefixMultTuple,
    random::random_quaternary::{QuatShare, QuaternaryShares},
};
use anyhow::{anyhow, Error};
use math_lib::{
    fields::{Inv, PrimeField},
    modular::{AsBits, CryptoRngCore, ModularNumber, SafePrime},
    polynomial::{point::Point, point_sequence::PointSequence},
};
use shamir_sharing::{
    party::PartyMapper,
    protocol::PolyDegree,
    secret_sharer::{PartyShares, SafePrimeSecretSharer, SecretSharer, ShamirSecretSharer},
};

/// A validator for the output of the PREP-COMPARE protocol.
#[derive(Default)]
pub struct PrepCompareValidator;

impl PrepCompareValidator {
    /// Validate that the provided shares are correct.
    pub fn validate<T: SafePrime>(
        &self,
        batch_size: usize,
        party_shares: PartyShares<Vec<PrepCompareShares<T>>>,
    ) -> Result<(), Error> {
        let mapper = PartyMapper::<PrimeField<T>>::new(party_shares.keys().cloned().collect())?;
        let mut point_sequences = vec![PointSequences::<T>::default(); batch_size];
        for (party_id, party_shares) in party_shares {
            if party_shares.len() != batch_size {
                return Err(anyhow!(
                    "unexpected element share count: expected {}, got {}",
                    batch_size,
                    party_shares.len()
                ));
            }
            let x =
                *mapper.abscissa(&party_id).ok_or_else(|| anyhow!("failed to find abscissa for party {party_id:?}"))?;

            for (element_index, element_shares) in party_shares.into_iter().enumerate() {
                let point_sequence = &mut point_sequences[element_index];
                point_sequence.bitwise.push(Point::new(x, element_shares.bitwise));
                let mut quaternary = Vec::new();
                for quat in element_shares.quaternary.shares() {
                    let (l, h, c) = quat.as_parts();
                    quaternary.push(*l);
                    quaternary.push(*h);
                    quaternary.push(*c);
                }
                unzip_shares(x, quaternary, &mut point_sequence.quaternary)?;
                point_sequence.compare_least_bit.push(Point::new(x, element_shares.comparison_least_bit));
                point_sequence.compare_most_bit.push(Point::new(x, element_shares.comparison_most_bit));
                let mut tuples = Vec::new();
                for t in element_shares.prefix_mult_tuples {
                    tuples.push(t.mask);
                    tuples.push(t.domino);
                }
                unzip_shares(x, tuples, &mut point_sequence.prefix_mult_tuples)?;
                unzip_shares(x, element_shares.zero_shares, &mut point_sequence.zero_shares)?;
            }
        }

        for point_sequences in point_sequences {
            let bitwise = point_sequences.bitwise.lagrange_interpolate()?;
            let quaternary = interpolate_many(point_sequences.quaternary)?;
            let compare_least_bit = point_sequences.compare_least_bit.lagrange_interpolate()?;
            let compare_most_bit = point_sequences.compare_most_bit.lagrange_interpolate()?;

            let quaternary_least = quaternary[0].into_value().bit(0);
            let bitwise_least = bitwise.into_value().bit(0);
            let bitwise_most = bitwise.into_value().bit(T::MODULO.bits() - 1);
            assert_eq!(compare_least_bit, ModularNumber::from_u32((quaternary_least ^ bitwise_least) as u32));
            assert_eq!(
                compare_most_bit,
                ModularNumber::from_u32((quaternary_least ^ bitwise_least ^ bitwise_most) as u32)
            );

            let prefix_mult_tuples = interpolate_many(point_sequences.prefix_mult_tuples)?;
            self.validate_prefixes(&prefix_mult_tuples)?;
            let zeros = interpolate_many(point_sequences.zero_shares)?;
            for z in zeros {
                assert_eq!(z, ModularNumber::ZERO);
            }
        }

        Ok(())
    }

    fn validate_prefixes<T: SafePrime>(&self, prefix: &[ModularNumber<T>]) -> Result<(), Error> {
        let len = (T::MODULO.bits() - 1) / 2;
        let mut product = ModularNumber::ONE;
        for index in (0..len).rev() {
            let mask = prefix[index * 2];
            let domino = prefix[index * 2 + 1];
            product = product * &domino;
            assert_eq!(mask * &product, ModularNumber::ONE);
        }
        Ok(())
    }
}

fn unzip_shares<T: SafePrime>(
    x: T::Normal,
    shares: Vec<ModularNumber<T>>,
    point_sequences: &mut [PointSequence<PrimeField<T>>],
) -> Result<(), Error> {
    if shares.len() != point_sequences.len() {
        return Err(anyhow!("unexpected share count: expected {}, got {}", point_sequences.len(), shares.len()));
    }
    for (share, point_sequence) in shares.into_iter().zip(point_sequences.iter_mut()) {
        point_sequence.push(Point::new(x, share));
    }
    Ok(())
}

fn interpolate_many<T: SafePrime>(
    point_sequences: Vec<PointSequence<PrimeField<T>>>,
) -> Result<Vec<ModularNumber<T>>, Error> {
    let mut secrets = Vec::new();
    for point_sequence in point_sequences {
        secrets.push(point_sequence.lagrange_interpolate()?);
    }
    Ok(secrets)
}

#[derive(Clone)]
struct PointSequences<T: SafePrime> {
    bitwise: PointSequence<PrimeField<T>>,
    quaternary: Vec<PointSequence<PrimeField<T>>>,
    compare_least_bit: PointSequence<PrimeField<T>>,
    compare_most_bit: PointSequence<PrimeField<T>>,
    prefix_mult_tuples: Vec<PointSequence<PrimeField<T>>>,
    zero_shares: Vec<PointSequence<PrimeField<T>>>,
}

impl<T: SafePrime> Default for PointSequences<T> {
    fn default() -> Self {
        let p = PointSequence::default();
        let bits = T::MODULO.bits();
        Self {
            bitwise: PointSequence::default(),
            quaternary: vec![p.clone(); (bits + 1) / 2 * 3],
            compare_least_bit: PointSequence::default(),
            compare_most_bit: PointSequence::default(),
            prefix_mult_tuples: vec![p.clone(); (bits - 1) / 2 * 2],
            zero_shares: vec![p; (bits + 1) / 2],
        }
    }
}

struct Invertible<T: SafePrime> {
    value: ModularNumber<T>,
    inverse: ModularNumber<T>,
}

struct Prefix<T: SafePrime> {
    mask: ModularNumber<T>,
    domino: ModularNumber<T>,
}

/// Builder that creates PREP-COMPARE shares without running PREP-COMPARE.
///
/// **This is meant to be used for testing purposes only**.
pub struct PrepCompareSharesBuilder<'a, R, T: SafePrime> {
    secret_sharer: &'a ShamirSecretSharer<T>,
    rng: R,
}

impl<'a, R, T> PrepCompareSharesBuilder<'a, R, T>
where
    R: CryptoRngCore,
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Construct a new shares builder.
    pub fn new(secret_sharer: &'a ShamirSecretSharer<T>, rng: R) -> Result<Self, Error> {
        Ok(Self { secret_sharer, rng })
    }

    /// Build `count` PREP-COMPARE shares.
    pub fn build(mut self, count: usize) -> Result<PartyShares<Vec<PrepCompareShares<T>>>, Error> {
        let mut party_shares: PartyShares<Vec<PrepCompareShares<T>>> = PartyShares::default();
        for _ in 0..count {
            let shares = self.build_one()?;
            for (party_id, shares) in shares {
                party_shares.entry(party_id).or_default().push(shares);
            }
        }
        Ok(party_shares)
    }

    fn build_one(&mut self) -> Result<PartyShares<PrepCompareShares<T>>, Error> {
        let prime_bits = T::MODULO.bits();
        let r = ModularNumber::<T>::gen_random_with_rng(&mut self.rng);
        let s = ModularNumber::<T>::gen_random_with_rng(&mut self.rng);
        let w0 = r.into_value().bit(0) ^ s.into_value().bit(0);
        let w1 = w0 ^ s.into_value().bit(prime_bits - 1);
        self.share_values(s, r, w0, w1)
    }

    #[allow(clippy::too_many_arguments)]
    fn share_values(
        &mut self,
        bitwise: ModularNumber<T>,
        quaternary: ModularNumber<T>,
        comparison_least_bit: bool,
        comparison_most_bit: bool,
    ) -> Result<PartyShares<PrepCompareShares<T>>, Error> {
        let bitwise: PartyShares<ModularNumber<T>> = self.secret_sharer.generate_shares(&bitwise, PolyDegree::T)?;
        let mut quaternary = self.quaternary_shares(quaternary).unwrap();
        let mut comparison_least_bit: PartyShares<ModularNumber<T>> =
            self.secret_sharer.generate_shares(&ModularNumber::from_u32(comparison_least_bit as u32), PolyDegree::T)?;
        let mut comparison_most_bit: PartyShares<ModularNumber<T>> =
            self.secret_sharer.generate_shares(&ModularNumber::from_u32(comparison_most_bit as u32), PolyDegree::T)?;
        let mut tuples = self.prefix_tuples().unwrap();
        let zeros = vec![ModularNumber::ZERO; (T::MODULO.bits() + 1) / 2];
        let mut zero_shares: PartyShares<Vec<ModularNumber<T>>> =
            self.secret_sharer.generate_shares(&zeros, PolyDegree::T)?;
        let mut output_shares = PartyShares::default();
        for (party_id, bitwise) in bitwise {
            let quaternary: QuaternaryShares<T> =
                quaternary.remove(&party_id).ok_or_else(|| anyhow!("{party_id} not found"))?.into();
            let comparison_least_bit =
                comparison_least_bit.remove(&party_id).ok_or_else(|| anyhow!("{party_id} not found"))?;
            let comparison_most_bit =
                comparison_most_bit.remove(&party_id).ok_or_else(|| anyhow!("{party_id} not found"))?;
            let prefix_mult_tuples = tuples.remove(&party_id).ok_or_else(|| anyhow!("{party_id} not found"))?;
            let zero_shares = zero_shares.remove(&party_id).ok_or_else(|| anyhow!("{party_id} not found"))?;
            output_shares.insert(
                party_id,
                PrepCompareShares {
                    bitwise,
                    quaternary,
                    comparison_least_bit,
                    comparison_most_bit,
                    prefix_mult_tuples,
                    zero_shares,
                },
            );
        }
        Ok(output_shares)
    }

    fn quaternary_shares(&mut self, secret: ModularNumber<T>) -> Result<PartyShares<Vec<QuatShare<T>>>, Error> {
        let bits = T::MODULO.bits();
        let secret = secret.into_value();
        let mut quat_shares: PartyShares<Vec<QuatShare<T>>> = PartyShares::default();
        for i in 0..(bits + 1) / 2 {
            let low = ModularNumber::from_u32(secret.bit(2 * i) as u32);
            let high = ModularNumber::from_u32(secret.bit(2 * i + 1) as u32);
            let cross = low * &high;
            let low_shares = self.secret_sharer.generate_shares(&low, PolyDegree::T)?;
            let high_shares = self.secret_sharer.generate_shares(&high, PolyDegree::T)?;
            let cross_shares = self.secret_sharer.generate_shares(&cross, PolyDegree::T)?;
            for (party_id, low) in low_shares.into_iter() {
                let high = high_shares.get(&party_id).unwrap();
                let cross = cross_shares.get(&party_id).unwrap();
                let quat = QuatShare::new(low, *high, *cross);
                quat_shares.entry(party_id.clone()).or_default().push(quat);
            }
        }
        Ok(quat_shares)
    }

    fn inv_rand(&mut self, count: usize) -> Result<Vec<Invertible<T>>, Error> {
        let mut invertibles = Vec::new();
        for _ in 0..count {
            // Generate random numbers until we don't get a one.
            let mut value = ModularNumber::gen_random_with_rng(&mut self.rng);
            while value == ModularNumber::ONE {
                value = ModularNumber::gen_random_with_rng(&mut self.rng);
            }
            let inverse = value.inv()?;
            invertibles.push(Invertible { value, inverse });
        }
        Ok(invertibles)
    }

    fn prefix(&mut self, count: usize) -> Result<Vec<Prefix<T>>, Error> {
        let invertibles = self.inv_rand(count)?;
        let mut dominos = vec![invertibles[0].inverse];
        for chunk in invertibles.windows(2) {
            let domino = chunk[1].inverse * &chunk[0].value;
            dominos.push(domino);
        }
        let prefixes =
            invertibles.into_iter().zip(dominos).map(|(inv, domino)| Prefix { mask: inv.value, domino }).collect();
        Ok(prefixes)
    }

    fn postfix(&mut self, count: usize) -> Result<Vec<Prefix<T>>, Error> {
        let mut prefixes = self.prefix(count)?;
        prefixes.reverse();
        Ok(prefixes)
    }

    fn prefix_tuples(&mut self) -> Result<PartyShares<Vec<PrefixMultTuple<T>>>, Error> {
        let bits = T::MODULO.bits();
        let prefixes = self.postfix((bits - 1) / 2)?;
        let mut shares: PartyShares<Vec<PrefixMultTuple<T>>> = PartyShares::default();
        for tuple in prefixes {
            let mask_shares = self.secret_sharer.generate_shares(&tuple.mask, PolyDegree::T)?;
            let domino_shares = self.secret_sharer.generate_shares(&tuple.domino, PolyDegree::T)?;
            for (party_id, mask) in mask_shares.into_iter() {
                let domino = *domino_shares.get(&party_id).unwrap();
                let tuple = PrefixMultTuple { mask, domino };
                shares.entry(party_id.clone()).or_default().push(tuple);
            }
        }
        Ok(shares)
    }
}

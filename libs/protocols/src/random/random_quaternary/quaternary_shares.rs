//! Quaternary Shares.

use crate::random::random_bit::output::BitShareError;
use math_lib::modular::{Modular, ModularNumber};

/// Represents shares where the secret behind it can only contain the values 0, 1, 2, or 3.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuatShare<T: Modular> {
    /// Low bit.
    low: ModularNumber<T>,

    /// High bit.
    high: ModularNumber<T>,

    /// Product of the two bits.
    cross: ModularNumber<T>,
}

impl<T: Modular> QuatShare<T> {
    /// Creates a new QuatShare.
    pub fn new(low: ModularNumber<T>, high: ModularNumber<T>, cross: ModularNumber<T>) -> QuatShare<T> {
        QuatShare { low, high, cross }
    }

    /// Creates a new QuatShare from single bit.
    pub fn single(low: ModularNumber<T>) -> QuatShare<T> {
        let high = ModularNumber::ZERO;
        let cross = ModularNumber::ZERO;
        QuatShare { low, high, cross }
    }

    /// Returns the values as parts.
    pub fn as_parts(&self) -> (&ModularNumber<T>, &ModularNumber<T>, &ModularNumber<T>) {
        (&self.low, &self.high, &self.cross)
    }

    /// Returns the low value.
    pub fn low(&self) -> &ModularNumber<T> {
        &self.low
    }

    /// Returns the high value.
    pub fn high(&self) -> &ModularNumber<T> {
        &self.high
    }

    /// Returns the cross value.
    pub fn cross(&self) -> &ModularNumber<T> {
        &self.cross
    }
}

/// A number where each of its bits is represented by a share.
#[derive(Clone, Debug)]
pub struct QuaternaryShares<T: Modular>(Vec<QuatShare<T>>);

impl<T: Modular> QuaternaryShares<T> {
    /// Gets the underlying shares.
    pub fn shares(&self) -> &[QuatShare<T>] {
        &self.0
    }

    /// Returns the least significant bit.
    pub fn least(&self) -> Result<&ModularNumber<T>, BitShareError> {
        Ok(self.shares().first().ok_or(BitShareError::NoElements)?.low())
    }

    /// Calculates r = \sum [r_i] * 2^i.
    pub fn merge_bits(&self) -> ModularNumber<T> {
        let mut output = ModularNumber::ZERO;
        let two = ModularNumber::two();
        let mut two_i = ModularNumber::ONE;
        for bit in self.shares().iter() {
            let term = two_i * bit.low();
            output = output + &term;
            two_i = two_i * &two;
            let term = two_i * bit.high();
            output = output + &term;
            two_i = two_i * &two;
        }
        output
    }
}

impl<T: Modular> From<Vec<QuatShare<T>>> for QuaternaryShares<T> {
    fn from(value: Vec<QuatShare<T>>) -> Self {
        Self(value)
    }
}

impl<T: Modular> From<QuaternaryShares<T>> for Vec<QuatShare<T>> {
    fn from(value: QuaternaryShares<T>) -> Self {
        value.0
    }
}

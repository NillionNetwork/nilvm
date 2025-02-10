//! Bitwise shared number.

use crate::random::random_bit::output::BitShare;
use anyhow::{anyhow, Error};
use math_lib::modular::{Modular, ModularNumber};

/// A number where each of its bits is represented by a share.
#[derive(Clone, Debug)]
pub struct BitwiseNumberShares<T: Modular>(Vec<BitShare<T>>);

impl<T: Modular> BitwiseNumberShares<T> {
    /// Gets the underlying shares.
    pub fn shares(&self) -> &[BitShare<T>] {
        &self.0
    }

    /// Returns the least significant bit bit.
    pub fn least(&self) -> Result<BitShare<T>, Error> {
        self.shares().first().cloned().ok_or_else(|| anyhow!("least significant bit not found"))
    }

    /// Returns the most significatn bit.
    pub fn most(&self) -> Result<BitShare<T>, Error> {
        self.shares().last().cloned().ok_or_else(|| anyhow!("most significant bit not found"))
    }

    /// Calculates r = \sum [r_i] * 2^i.
    pub fn merge_bits(&self) -> ModularNumber<T> {
        merge_bits(self.shares())
    }

    /// Returns the length of the shares.
    pub fn len(&self) -> usize {
        self.shares().len()
    }

    /// Returns the if the bitwise shares are empty.
    pub fn is_empty(&self) -> bool {
        self.shares().is_empty()
    }
}

impl<T: Modular> From<Vec<ModularNumber<T>>> for BitwiseNumberShares<T> {
    fn from(value: Vec<ModularNumber<T>>) -> Self {
        Self(value.into_iter().map(|b| b.into()).collect())
    }
}

impl<T: Modular> From<Vec<BitShare<T>>> for BitwiseNumberShares<T> {
    fn from(value: Vec<BitShare<T>>) -> Self {
        Self(value)
    }
}

impl<T: Modular> From<BitwiseNumberShares<T>> for Vec<BitShare<T>> {
    fn from(value: BitwiseNumberShares<T>) -> Self {
        value.0
    }
}

/// Calculates r = \sum [r_i] * 2^i.
pub fn merge_bits<T: Modular>(bits: &[BitShare<T>]) -> ModularNumber<T> {
    let mut output = ModularNumber::ZERO;
    let two = ModularNumber::two();
    let mut two_i = ModularNumber::ONE;
    for bit in bits.iter() {
        let term = two_i * bit.value();
        output = output + &term;
        two_i = two_i * &two;
    }
    output
}

#[cfg(test)]
mod test {
    use super::*;
    use math_lib::modular::U64SafePrime;

    type Prime = U64SafePrime;

    #[test]
    fn bit_merging() {
        let shares = BitwiseNumberShares::from(vec![
            BitShare::from(ModularNumber::<Prime>::ONE),
            BitShare::from(ModularNumber::<Prime>::ZERO),
            BitShare::from(ModularNumber::<Prime>::ONE),
            BitShare::from(ModularNumber::<Prime>::ONE),
        ]);
        let value = shares.merge_bits();
        assert_eq!(value, ModularNumber::from_u32(13));
    }
}

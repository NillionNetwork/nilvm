//! Powers of generator

use super::{Modular, Prime};
use crate::modular::{
    repr::{AsBits, Integer, NonZero},
    ModularNumber,
};
use crypto_bigint::CheckedSub;
use std::ops::{BitAnd, Rem};

/// Power triple
#[derive(Default, Debug)]
struct PowerTriple<T: Modular> {
    uno: ModularNumber<T>,
    dos: ModularNumber<T>,
    tre: ModularNumber<T>,
}

/// Cached Powers of ModularNumber
#[derive(Default, Debug)]
pub struct Power<T: Modular> {
    powers: Vec<PowerTriple<T>>,
}

impl<T: Prime> Power<T> {
    /// Create new Power
    pub fn new(value: ModularNumber<T>) -> Power<T> {
        let mut powers = Vec::new();
        let mut uno;
        let mut dos = value;
        let mut tre;
        let bits = T::MODULO.bits();
        for i in 1..bits {
            uno = dos;
            dos = dos * &uno;
            if i.bitand(1) == 1 {
                tre = uno * &dos;
                let triple = PowerTriple { uno, dos, tre };
                powers.push(triple);
            }
        }
        if bits.bitand(1) == 1 {
            let one = ModularNumber::ONE;
            let triple = PowerTriple { uno: dos, dos: one, tre: one };
            powers.push(triple);
        }
        Power { powers }
    }

    /// Power of generator
    pub fn exp(&self, exponent: &T::Normal) -> ModularNumber<T> {
        let one = T::Normal::ONE;
        let group = NonZero::new(T::MODULO.checked_sub(&one).unwrap()).unwrap();
        let exponent = exponent.rem(group);
        let mut res = ModularNumber::ONE;
        for (i, p) in self.powers.iter().enumerate() {
            let j = i.wrapping_mul(2);
            let k = j.wrapping_add(1);
            let bit_j = exponent.bit(j);
            let bit_k = exponent.bit(k);
            match (bit_j, bit_k) {
                (true, false) => {
                    res = res * &p.uno;
                }
                (false, true) => {
                    res = res * &p.dos;
                }
                (true, true) => {
                    res = res * &p.tre;
                }
                _ => {}
            }
        }
        res
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::test_prime;
    use rstest::rstest;

    test_prime!(P19, 19);
    test_prime!(P13, 13);
    test_prime!(P227, 227);

    #[rstest]
    #[case(P19, 5, 117, 1)]
    #[case(P13, 50, 1234, 10)]
    #[case(P227, 86, 79, 123)]
    fn power_exp<T: Prime>(#[case] _prime: T, #[case] base: u64, #[case] exponent: u64, #[case] expected: u64) {
        let base = ModularNumber::<T>::from_u64(base);
        let power = Power::new(base);
        let exponent = T::Normal::from(exponent);
        let result = power.exp(&exponent);
        assert_eq!(result, ModularNumber::from_u64(expected));
    }
}

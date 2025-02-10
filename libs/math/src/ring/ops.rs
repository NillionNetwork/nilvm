//! Operations for ring elements.

use super::RingTuple;
use crate::modular::SophiePrime;
use std::ops::{Add, Mul, Neg, Sub};

impl<T: SophiePrime> Add<&RingTuple<T>> for RingTuple<T> {
    type Output = RingTuple<T>;

    #[allow(clippy::arithmetic_side_effects)]
    fn add(self, rhs: &RingTuple<T>) -> Self::Output {
        let (prime_element, binary_ext_element) = self.into_parts();
        let prime_element = prime_element + rhs.prime_element();
        let binary_ext_element = binary_ext_element + rhs.binary_ext_element();
        RingTuple::new(prime_element, binary_ext_element)
    }
}

impl<T: SophiePrime> Sub<&RingTuple<T>> for RingTuple<T> {
    type Output = RingTuple<T>;

    #[allow(clippy::arithmetic_side_effects)]
    fn sub(self, rhs: &RingTuple<T>) -> Self::Output {
        let (prime_element, binary_ext_element) = self.into_parts();
        let prime_element = prime_element - rhs.prime_element();
        let binary_ext_element = binary_ext_element - rhs.binary_ext_element();
        RingTuple::new(prime_element, binary_ext_element)
    }
}

impl<T: SophiePrime> Neg for &RingTuple<T> {
    type Output = RingTuple<T>;

    fn neg(self) -> Self::Output {
        let (prime, binary) = self.as_parts();
        RingTuple::new(-prime, -binary)
    }
}

impl<T: SophiePrime> Mul<&RingTuple<T>> for RingTuple<T> {
    type Output = RingTuple<T>;

    #[allow(clippy::arithmetic_side_effects)]
    fn mul(self, rhs: &RingTuple<T>) -> Self::Output {
        let (prime_element, binary_ext_element) = self.into_parts();
        let prime_element = prime_element * rhs.prime_element();
        let binary_ext_element = binary_ext_element * rhs.binary_ext_element();
        RingTuple::new(prime_element, binary_ext_element)
    }
}

#[cfg(test)]
mod test {
    use rstest::rstest;

    use super::*;
    use crate::{
        galois::GF256,
        modular::{ModularNumber, U64SophiePrime},
        ring::crt,
    };

    type Prime = U64SophiePrime;

    #[test]
    fn addition() {
        let lhs = RingTuple::new(ModularNumber::<Prime>::from_u32(42), GF256::new(12));
        let rhs = RingTuple::new(ModularNumber::<Prime>::from_u32(100), GF256::new(155));
        let result = lhs + &rhs;
        assert_eq!(result.prime_element(), &ModularNumber::from_u32(142));
        assert_eq!(result.binary_ext_element(), &GF256::new(151));
    }

    #[test]
    fn subtraction() {
        let lhs = RingTuple::new(ModularNumber::<Prime>::from_u32(100), GF256::new(155));
        let rhs = RingTuple::new(ModularNumber::<Prime>::from_u32(42), GF256::new(8));
        let result = lhs - &rhs;
        assert_eq!(result.prime_element(), &ModularNumber::from_u32(58));
        assert_eq!(result.binary_ext_element(), &GF256::new(147));
    }

    #[rstest]
    #[case(100, 10, 9223372036318953373u64)]
    #[case(200, 17, 9223372036318953273u64)]
    fn negative(#[case] prime: u64, #[case] binary: u8, #[case] neg_prime: u64) {
        let lhs = RingTuple::new(ModularNumber::<Prime>::from_u64(prime), GF256::new(binary));
        let result = -&lhs;
        assert_eq!(result.prime_element(), &ModularNumber::from_u64(neg_prime));
        assert_eq!(result.binary_ext_element(), &GF256::new(binary));
        let left = crt(lhs);
        let right = crt(result);
        assert_eq!(left, -right);
    }

    #[rstest]
    #[case(54, 6, 42, 8, 2268, 48)]
    #[case(54, 1, 42, 8, 2268, 8)]
    fn multiplication(
        #[case] p1: u32,
        #[case] b1: u8,
        #[case] p2: u32,
        #[case] b2: u8,
        #[case] pr: u32,
        #[case] br: u8,
    ) {
        let lhs = RingTuple::new(ModularNumber::<Prime>::from_u32(p1), GF256::new(b1));
        let rhs = RingTuple::new(ModularNumber::<Prime>::from_u32(p2), GF256::new(b2));
        let result = lhs.clone() * &rhs;
        assert_eq!(result.prime_element(), &ModularNumber::from_u32(pr));
        assert_eq!(result.binary_ext_element(), &GF256::new(br));
    }
}

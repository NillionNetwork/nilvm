//! ModularNumber Operations

use super::{Modular, Prime, RemEuclid};
use crate::{errors::DivByZero, fields::Inv, modular::ModularNumber};
use crypto_bigint::NonZero;
use num_bigint::BigInt;
use num_traits::Zero;
use std::ops::{Add, Div, Mul, Neg, Rem, Shr, Sub};

impl<T: Modular> Sub<&ModularNumber<T>> for ModularNumber<T> {
    type Output = ModularNumber<T>;

    fn sub(self, other: &ModularNumber<T>) -> ModularNumber<T> {
        (&self).sub(other)
    }
}

impl<T: Modular> Sub for &ModularNumber<T> {
    type Output = ModularNumber<T>;

    fn sub(self, other: &ModularNumber<T>) -> ModularNumber<T> {
        let value = T::sub_mod(&self.value, &other.value);
        // Note: this is initialized this way as it's already guaranteed to be mod p and
        // `ModularNumber::new` will otherwise attempt to perform another modulo by default.
        ModularNumber { value }
    }
}

impl<T: Modular> Add<&ModularNumber<T>> for ModularNumber<T> {
    type Output = ModularNumber<T>;

    fn add(self, other: &ModularNumber<T>) -> ModularNumber<T> {
        (&self).add(other)
    }
}

impl<T: Modular> Add for &ModularNumber<T> {
    type Output = ModularNumber<T>;

    fn add(self, other: &ModularNumber<T>) -> ModularNumber<T> {
        let value = T::add_mod(&self.value, &other.value);
        ModularNumber { value }
    }
}

impl<T: Modular> Mul<&ModularNumber<T>> for ModularNumber<T> {
    type Output = ModularNumber<T>;

    fn mul(self, other: &ModularNumber<T>) -> ModularNumber<T> {
        (&self).mul(other)
    }
}

impl<T: Modular> Mul for &ModularNumber<T> {
    type Output = ModularNumber<T>;

    fn mul(self, other: &ModularNumber<T>) -> ModularNumber<T> {
        let value = T::mul_mod(&self.value, &other.value);
        ModularNumber { value }
    }
}

impl<T: Prime> Div<&ModularNumber<T>> for ModularNumber<T> {
    type Output = Result<ModularNumber<T>, DivByZero>;

    fn div(self, other: &ModularNumber<T>) -> Result<ModularNumber<T>, DivByZero> {
        (&self).div(other)
    }
}

impl<T: Prime> Div for &ModularNumber<T> {
    type Output = Result<ModularNumber<T>, DivByZero>;

    #[allow(clippy::suspicious_arithmetic_impl)]
    fn div(self, other: &ModularNumber<T>) -> Result<ModularNumber<T>, DivByZero> {
        Ok(self * &other.inverse())
    }
}

impl<T: Modular> Neg for ModularNumber<T> {
    type Output = ModularNumber<T>;

    fn neg(self) -> Self::Output {
        (&self).neg()
    }
}

impl<T: Modular> Neg for &ModularNumber<T> {
    type Output = ModularNumber<T>;

    fn neg(self) -> Self::Output {
        let value = T::neg_mod(&self.value);
        ModularNumber { value }
    }
}

impl<T: Prime> Inv for ModularNumber<T> {
    type Output = Result<ModularNumber<T>, DivByZero>;

    fn inv(self) -> Self::Output {
        if self.is_zero() {
            return Err(DivByZero);
        }
        Ok(self.inverse())
    }
}

/// Donald Knuth promotes floored division, for which the quotient is defined by q = floor(a / n)
/// where floor function rounds down to the nearest integer. Thus according to this equation, the
/// remainder has the same sign as the divisor n: r = a - n * floor(a / n).
pub trait FloorMod {
    /// Floor modulo output.
    type Output;

    /// Floor modulo.
    fn fmod(self, rhs: Self) -> Self::Output;
}

impl<T: Modular> FloorMod for &ModularNumber<T> {
    type Output = Result<ModularNumber<T>, DivByZero>;

    /// Floor mod returns the floor modulo for *SIGNED* ModularNumbers.
    /// Rust returns truncated modulo so we need to convert it.
    fn fmod(self, divisor: Self) -> Self::Output {
        let zero = BigInt::zero();
        // Convert ModularNumbers to signed BigInts.
        let dividend = BigInt::from(self);
        let divisor = BigInt::from(divisor);
        if divisor == zero {
            return Err(DivByZero);
        }
        let mut rem = dividend % &divisor;
        if (rem > zero && divisor < zero) || (rem < zero && divisor > zero) {
            // Remainder and divisor have different signs, so truncated mod and floor mod differ.
            // Add the divisor so we get the floor value instead of the truncated modulo.
            rem += divisor;
        }
        // Safety: Remainder is smaller than divisor, hence can convert to ModularNumber.
        #[allow(clippy::unwrap_used)]
        Ok(ModularNumber::try_from(&rem).unwrap())
    }
}

impl<T: Modular> Rem<&ModularNumber<T>> for ModularNumber<T> {
    type Output = Result<ModularNumber<T>, DivByZero>;

    fn rem(self, rhs: &Self) -> Self::Output {
        (&self).rem(rhs)
    }
}

impl<T: Modular> Rem for &ModularNumber<T> {
    type Output = Result<ModularNumber<T>, DivByZero>;

    fn rem(self, rhs: Self) -> Self::Output {
        let left = self.into_value();
        let right = match Option::from(NonZero::new(rhs.into_value())) {
            Some(value) => value,
            None => return Err(DivByZero),
        };
        let modulo = left.rem_euclid(&right);
        Ok(ModularNumber::new(modulo))
    }
}

impl<T: Prime> Shr for ModularNumber<T> {
    type Output = Result<ModularNumber<T>, DivByZero>;

    fn shr(self, rhs: Self) -> Self::Output {
        (&self).shr(&rhs)
    }
}

impl<T: Prime> Shr for &ModularNumber<T> {
    type Output = Result<ModularNumber<T>, DivByZero>;

    fn shr(self, rhs: Self) -> Self::Output {
        let dividend = self;
        let two_to_m = ModularNumber::two().exp_mod(&rhs.into_value());
        let even_dividend = dividend - &(dividend % &two_to_m)?;
        Ok(even_dividend * &two_to_m.inverse())
    }
}

/// Modular pow
pub trait ModularPow<E> {
    /// output type of Modular pow
    type Output;

    /// modular pow operation
    fn exp_mod(self, exp: &E) -> Self::Output;
}

impl<T: Modular> ModularPow<T::Normal> for ModularNumber<T> {
    type Output = Self;

    fn exp_mod(self, exp: &T::Normal) -> Self::Output {
        let value = T::exp_mod(&self.value, exp);
        ModularNumber { value }
    }
}

/// Modular inverse
pub trait ModularInverse {
    /// Modular inverse output
    type Output;

    /// Modular inverse operation
    fn inverse(self) -> Self::Output;

    /// Check if rhs is modular inverse of self
    fn is_inverse(&self, rhs: &Self) -> bool;
}

impl<T: Prime> ModularInverse for ModularNumber<T> {
    type Output = ModularNumber<T>;

    fn inverse(self) -> ModularNumber<T> {
        let value = T::inv_mod(&self.value);
        ModularNumber { value }
    }

    fn is_inverse(&self, rhs: &Self) -> bool {
        self.mul(rhs) == Self::ONE
    }
}

#[cfg(test)]
mod test {
    use super::FloorMod;
    use crate::{
        modular::{
            ops::{ModularInverse, ModularPow},
            ModularNumber,
        },
        test_prime,
    };
    use crypto_bigint::U64;
    use num_bigint::BigInt;
    use rstest::rstest;

    test_prime!(P11, 11u64);
    test_prime!(P13, 13u64);
    test_prime!(P19, 19u64);

    #[rstest]
    #[case(1, 1, 1)]
    #[case(1, 2, 2)]
    #[case(2, 3, 6)]
    #[case(3, 4, 1)]
    #[case(4, 4, 5)]
    #[case(10, 1, 10)]
    fn test_mult_mod_11(#[case] left: u32, #[case] right: u32, #[case] expected: u32) {
        let left = ModularNumber::<P11>::from_u32(left);
        let right = ModularNumber::<P11>::from_u32(right);
        let expected = ModularNumber::<P11>::from_u32(expected);
        assert_eq!(left * &right, expected);
    }

    #[test]
    fn test_100_mod13_div_2_mod_13() {
        let lhs = ModularNumber::<P13>::from_u32(100);
        let rhs = ModularNumber::<P13>::from_u32(2);
        assert_eq!((lhs / &rhs).unwrap(), ModularNumber::from_u32(11));
    }

    #[test]
    fn test_5_pow_117_mod_19() {
        let base = ModularNumber::<P19>::from_u32(5);
        let exponent = U64::from_u32(117);
        assert_eq!(base.exp_mod(&exponent), ModularNumber::ONE);
    }

    #[test]
    fn test_5_pow_1234_mod_13() {
        let base = ModularNumber::<P13>::from_u32(50);
        let exponent = U64::from_u32(1234);
        assert_eq!(base.exp_mod(&exponent), ModularNumber::from_u32(10));
    }

    #[test]
    fn test_inv_3_mod_11() {
        let n = ModularNumber::<P11>::from_u32(3);
        assert_eq!(n.inverse(), ModularNumber::from_u32(4));
    }

    #[test]
    fn test_inv_7_mod_11() {
        let n = ModularNumber::<P11>::from_u32(7);
        assert_eq!(n.inverse(), ModularNumber::from_u32(8));
    }

    #[rstest]
    #[case(1, 1, 0)]
    #[case(2, 4, 2)]
    #[case(8, 5, 3)]
    fn test_rem_operation(#[case] left: u32, #[case] right: u32, #[case] expected: u32) {
        let left = ModularNumber::<P11>::from_u32(left);
        let right = ModularNumber::<P11>::from_u32(right);
        let expected = ModularNumber::<P11>::from_u32(expected);
        assert_eq!((left % &right).unwrap(), expected);
    }

    #[rstest]
    #[case(1, 1, 0)]
    #[case(5, 3, 2)]
    #[case(2, 4, 2)]
    #[case(-1, 5, 4)]
    #[case(-5, 3, 1)]
    #[case(3, -2, -1)]
    #[case(-3, -2, -1)]
    #[case(-4, 2, 0)]
    #[case(4, -2, 0)]
    fn test_floor_modulo_operation(#[case] left: i32, #[case] right: i32, #[case] expected: i32) {
        let left = ModularNumber::<P11>::try_from(&BigInt::from(left)).unwrap();
        let right = ModularNumber::<P11>::try_from(&BigInt::from(right)).unwrap();
        let expected = ModularNumber::<P11>::try_from(&BigInt::from(expected)).unwrap();
        assert_eq!((left.fmod(&right)).unwrap(), expected);
    }

    #[rstest]
    #[case(1, 1, 0)]
    #[case(4, 1, 2)]
    #[case(9, 1, 4)]
    #[case(10, 2, 2)]
    fn test_right_shift_operation(#[case] left: u32, #[case] right: u32, #[case] expected: u32) {
        let left = ModularNumber::<P11>::from_u32(left);
        let right = ModularNumber::<P11>::from_u32(right);
        let expected = ModularNumber::<P11>::from_u32(expected);
        assert_eq!((left >> right).unwrap(), expected);
    }
}

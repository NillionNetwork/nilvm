//! ModularNumber Square Root.

use super::Prime;
use crate::{
    errors::DivByZero,
    modular::{ops::ModularPow, ModularNumber, RemEuclid},
};
use crypto_bigint::{subtle::CtOption, CheckedAdd, CheckedSub, Integer, NonZero, Zero};
use std::{cmp::min, ops::BitAnd};
use thiserror::Error;

/// Square root of a prime field element.
pub trait Sqrt {
    /// Result of modular number square root.
    type Output;

    /// Square Root.
    fn sqrt(self) -> Self::Output;
}

type TonelliResult<T, R> = Result<(R, ModularNumber<T>, R), SqrtError>;

fn tonelli<T: Prime>() -> TonelliResult<T, T::Normal> {
    let zero = T::Normal::ZERO;
    let one = T::Normal::ONE;
    let mut q = as_result(T::MODULO.checked_sub(&one))?;
    let mut s = one;
    let two = NonZero::new(T::Normal::from(2)).unwrap();
    while q.bitand(&one) == zero {
        q = q / two;
        s = as_result(s.checked_add(&one))?;
    }

    let mut z = ModularNumber::ONE;
    let exponent = as_result(T::MODULO.checked_sub(&one))? / two;
    let mut i = T::Normal::from(2);
    while i < T::MODULO {
        let z_i = ModularNumber::<T>::new(i);
        let pow_z_i = z_i.exp_mod(&exponent);
        if pow_z_i != ModularNumber::ONE {
            z = ModularNumber::new(i);
            break;
        }
        i = as_result(i.checked_add(&one))?;
    }
    let c = z.exp_mod(&q);

    Ok((s, c, q))
}

impl<T: Prime> Sqrt for ModularNumber<T> {
    type Output = Result<Self, SqrtError>;

    fn sqrt(self) -> Self::Output {
        let one = T::Normal::ONE;
        let zero = T::Normal::ZERO;
        let four = NonZero::new(T::Normal::from(4)).unwrap();
        if T::MODULO.rem_euclid(&four) == T::Normal::from(3) {
            let exponent = as_result(T::MODULO.checked_add(&one))? / four;
            let r = self.exp_mod(&exponent);
            let r = min(-r, r);
            let r2 = r * &r;
            if r2 == self {
                return Ok(r);
            } else {
                return Err(SqrtError::NonQuadraticResidue);
            }
        }

        // p % 4 = 1. s,c,q can be cached as they only depend on p.
        let (mut s, mut c, q) = tonelli::<T>()?;

        let mut t = self.exp_mod(&q);
        let two = NonZero::new(T::Normal::from(2)).unwrap();
        let mut r = self.exp_mod(&(as_result(q.checked_add(&one))? / two));
        loop {
            if t == ModularNumber::ZERO {
                return Err(SqrtError::NonQuadraticResidue);
            } else if t == ModularNumber::ONE {
                r = min(-r, r);
                return Ok(r);
            }
            let mut i = one;
            let mut tt = t;
            while i < s {
                if tt == ModularNumber::ONE {
                    break;
                }
                tt = tt * &tt;
                i = as_result(i.checked_add(&one))?;
            }
            if i == s {
                return Err(SqrtError::NonQuadraticResidue);
            }
            let power = as_result(as_result(s.checked_sub(&i))?.checked_sub(&one))?;
            let mut b = c;
            let mut j = zero;
            while j < power {
                b = b * &b;
                j = as_result(j.checked_add(&one))?;
            }
            s = i;
            r = r * &b;
            c = b * &b;
            t = t * &c;
        }
    }
}

// TODO: this kind of defeats the purpose of constant time arithmetic. We should go through this
// and make it better.
fn as_result<T>(value: CtOption<T>) -> Result<T, SqrtError> {
    if value.is_some().unwrap_u8() == 1 { Ok(value.unwrap()) } else { Err(SqrtError::Arithmetic) }
}

/// Square Root Error.
#[derive(Error, Debug, Eq, PartialEq)]
pub enum SqrtError {
    /// Operation error.
    #[error("square root operation error: division by zero")]
    Operation(#[from] DivByZero),

    /// No square root exists error.
    #[error("square root is not a quadratic residue")]
    NonQuadraticResidue,

    /// An arithmetic under/overflow.
    #[error("square root arithmetic under/overflow")]
    Arithmetic,
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        modular::{U128SafePrime, U256SafePrime, U64SafePrime},
        test_prime,
    };
    use rstest::rstest;

    test_prime!(P11, 11u64);
    test_prime!(P13, 13u64);
    test_prime!(P65537, 65537u64);
    test_prime!(P18446744073709550147, 18446744073709550147u64);
    test_prime!(P18446744073709551557, 18446744073709551557u64);

    #[rstest]
    #[case(P11, 6)]
    #[case(P13, 6)]
    #[case(P65537, 6)]
    #[case(P18446744073709550147, 6)]
    #[case(P18446744073709551557, 3)]
    fn sqrt_non_residue<T: Prime>(#[case] _prime: T, #[case] base: u64) {
        let base = ModularNumber::<T>::from_u64(base);
        let result = base.sqrt().err().unwrap();
        let expected = SqrtError::NonQuadraticResidue;
        assert_eq!(result, expected);
    }

    #[rstest]
    #[case(P11, 9, 3)]
    #[case(P13, 4, 2)]
    #[case(P65537, 16, 4)]
    #[case(P18446744073709550147, 9, 3)]
    #[case(P18446744073709550147, 16, 4)]
    #[case(P18446744073709550147, 3, 7167371418810987020)]
    #[case(P18446744073709551557, 9, 3)]
    #[case(P18446744073709551557, 16, 4)]
    #[case(P18446744073709551557, 6, 3789919121787743779)]
    fn sqrt_residue<T: Prime>(#[case] _prime: T, #[case] base: u64, #[case] expected: u64) {
        let base = ModularNumber::<T>::from_u64(base);
        let result = base.clone().sqrt().unwrap();
        let result_squared = result.clone() * &result;
        assert_eq!(result_squared, base);
        let expected = ModularNumber::from_u64(expected);
        assert_eq!(result, expected);
    }

    #[rstest]
    #[case(P11)]
    #[case(P13)]
    #[case(P65537)]
    #[case(P18446744073709550147)]
    #[case(U64SafePrime)]
    #[case(U128SafePrime)]
    #[case(U256SafePrime)]
    fn sqrt_does_not_panic<T: Prime>(#[case] _prime: T) {
        let number = ModularNumber::<T>::from_u64(4);
        let root = number.sqrt().unwrap();
        assert_eq!(root, ModularNumber::two());
    }
}

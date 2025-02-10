//! Binary Extension Field Operations

use crate::{errors::DivByZero, fields::Inv, galois::GF256};
use std::ops::{Add, Div, Mul, Neg, Sub};

#[allow(clippy::suspicious_arithmetic_impl)]
impl Add<&GF256> for GF256 {
    type Output = GF256;

    fn add(self, other: &Self) -> GF256 {
        (&self).add(other)
    }
}

#[allow(clippy::suspicious_arithmetic_impl)]
impl Add for &GF256 {
    type Output = GF256;

    fn add(self, other: Self) -> GF256 {
        GF256::new(self.value() ^ other.value())
    }
}

#[allow(clippy::suspicious_arithmetic_impl)]
impl Sub<&GF256> for GF256 {
    type Output = GF256;

    fn sub(self, other: &Self) -> Self::Output {
        (&self).sub(other)
    }
}

#[allow(clippy::suspicious_arithmetic_impl)]
impl Sub for &GF256 {
    type Output = GF256;

    fn sub(self, other: Self) -> Self::Output {
        GF256::new(self.value() ^ other.value())
    }
}

impl Neg for GF256 {
    type Output = GF256;

    fn neg(self) -> Self::Output {
        (&self).neg()
    }
}

impl Neg for &GF256 {
    type Output = GF256;

    fn neg(self) -> Self::Output {
        GF256::new(self.value())
    }
}

impl Mul<&GF256> for GF256 {
    type Output = GF256;

    fn mul(self, other: &Self) -> GF256 {
        (&self).mul(other)
    }
}

// Note: on Mul, Div and Inv we allow integer arithmetic + index slicing given we know that any
// lookups succeed. There's a test in `test.rs` that ensures no combination of inputs can panic.
impl Mul for &GF256 {
    type Output = GF256;

    #[allow(clippy::suspicious_arithmetic_impl, clippy::arithmetic_side_effects, clippy::indexing_slicing)]
    fn mul(self, other: Self) -> GF256 {
        // ALOGTABLE[(LOGTABLE[self.value()]) + (LOGTABLE[other.value()])]
        let log_self = LOGTABLE[self.value() as usize];
        let log_other = LOGTABLE[other.value() as usize];
        let log = log_self + log_other;
        let alog = ALOGTABLE[log];
        GF256::new(alog)
    }
}

impl Div<&GF256> for GF256 {
    type Output = Result<GF256, DivByZero>;

    fn div(self, other: &Self) -> Result<GF256, DivByZero> {
        (&self).div(other)
    }
}

impl Div for &GF256 {
    type Output = Result<GF256, DivByZero>;

    #[allow(clippy::arithmetic_side_effects, clippy::indexing_slicing)]
    fn div(self, other: Self) -> Result<GF256, DivByZero> {
        if other.value() == 0 {
            return Err(DivByZero);
        }
        // ALOGTABLE[LOGTABLE[self.value()] + 255 - LOGTABLE[other.value()]]
        let log_self = LOGTABLE[self.value() as usize];
        let log_other = LOGTABLE[other.value() as usize];
        let log = log_self + 255 - log_other;
        let alog = ALOGTABLE[log];
        Ok(GF256::new(alog))
    }
}

impl Inv for GF256 {
    type Output = Result<GF256, DivByZero>;

    #[allow(clippy::arithmetic_side_effects, clippy::indexing_slicing)]
    fn inv(self) -> Result<GF256, DivByZero> {
        if self.value() == 0u8 {
            return Err(DivByZero);
        }
        // ALOGTABLE[255 - (LOGTABLE[self.idx()] % 255)]
        let log_self = LOGTABLE[self.value() as usize];
        let log = 255 - log_self % 255;
        let alog = ALOGTABLE[log];
        Ok(GF256::new(alog))
    }
}

/// Generates multiplication tables.
#[allow(clippy::indexing_slicing)]
#[allow(clippy::arithmetic_side_effects)]
pub const fn gen_tables(genpoly: usize) -> ([usize; 256], [u8; 1025]) {
    let mut logtable: [usize; 256] = [0; 256];
    let mut alogtable: [u8; 1025] = [0; 1025];

    logtable[0] = 512;
    alogtable[0] = 1;

    let mut i = 1;
    while i < 255 {
        let mut next = (alogtable[i - 1] as usize) * 2;
        if next >= 256 {
            next ^= genpoly;
        }

        alogtable[i] = next as u8;
        logtable[alogtable[i] as usize] = i;

        i += 1;
    }

    alogtable[255] = alogtable[0];
    logtable[alogtable[255] as usize] = 255;
    let mut i = 256;
    while i < 510 {
        alogtable[i] = alogtable[i % 255];

        i += 1;
    }

    alogtable[510] = 1;

    (logtable, alogtable)
}

const TABLES: ([usize; 256], [u8; 1025]) = gen_tables(0x11D);
const LOGTABLE: &[usize; 256] = &TABLES.0;
const ALOGTABLE: &[u8; 1025] = &TABLES.1;

#[cfg(test)]
mod test {
    use crate::{fields::field::Inv, galois::GF256};
    use std::ops::{Div, Mul};

    #[test]
    fn test_add_gf256() {
        let a = GF256::new(4);
        let b = GF256::new(12);
        let c = a + &b;
        let result = GF256::new(8);
        assert_eq!(c, result);
    }

    #[test]
    fn test_sub_gf256() {
        let a = GF256::new(123);
        let b = GF256::new(79);
        let c = a - &b;
        let result = GF256::new(52);
        assert_eq!(c, result);
    }

    #[test]
    fn test_neg_gf256() {
        let a = GF256::new(4);
        let b = -a;
        assert_eq!(a, b);
    }

    #[test]
    fn test_mul_gf256() {
        let a = GF256::new(4);
        let b = GF256::new(69);
        let c = a * &b;
        let result = GF256::new(9);
        assert_eq!(c, result);
    }

    #[test]
    fn test_div_gf256() {
        let a = GF256::new(29);
        let b = GF256::new(69);
        let c = (a / &b).unwrap();
        let result = GF256::new(181);
        assert_eq!(c, result);
    }

    #[test]
    fn test_inv_gf256() {
        let a = GF256::new(39);
        let b = a.inv().unwrap();
        let c = a * &b;
        let result = GF256::new(1);
        assert_eq!(c, result);
    }

    // This test ensures `Div` and `Inv` don't panic for any combination of inputs.
    #[test]
    fn test_ops_dont_panic() {
        for left in 0..=255 {
            let left = GF256::new(left);
            // x / 0 should return an error.
            assert!(left.div(&GF256::new(0)).is_err());
            // Everything else shouldn't.
            for right in 1..=255 {
                let right = GF256::new(right);
                assert!(left.div(&right).is_ok());
                assert!(left.mul(&right) >= GF256::new(0));
            }
        }
        // 0 has no inverse.
        assert!(GF256::new(0).inv().is_err());
    }
}

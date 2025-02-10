//! Conversions between types.

use crate::modular::Overflow;
use num_bigint::BigInt;
use num_traits::{One, Zero};

/// Converts a BigInt back into a bool.
pub fn boolean_from_bigint(value: BigInt) -> Result<bool, Overflow> {
    if value == BigInt::one() {
        Ok(true)
    } else if value == BigInt::zero() {
        Ok(false)
    } else {
        Err(Overflow)
    }
}

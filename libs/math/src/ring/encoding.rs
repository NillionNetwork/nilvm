//! Encoding for `RingTuple`.

use super::RingTuple;
use crate::{
    galois::GF256,
    modular::{DecodeError, EncodedModularNumber, ModularNumber, SophiePrime},
};

/// An encoded version of a `RingTuple` that hides the modulo in use.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EncodedRingTuple {
    prime_element: EncodedModularNumber,
    binary_ext_element: u8,
}

impl EncodedRingTuple {
    /// Attempt to decode a ring tuple.
    pub fn try_decode<T: SophiePrime>(&self) -> Result<RingTuple<T>, DecodeError> {
        RingTuple::try_from(self)
    }
}

impl<T: SophiePrime> From<&RingTuple<T>> for EncodedRingTuple {
    fn from(value: &RingTuple<T>) -> Self {
        let (prime_element, binary_ext_element) = value.as_parts();
        Self { prime_element: prime_element.into(), binary_ext_element: binary_ext_element.into() }
    }
}

impl<T: SophiePrime> TryFrom<&EncodedRingTuple> for RingTuple<T> {
    type Error = DecodeError;

    fn try_from(value: &EncodedRingTuple) -> Result<Self, Self::Error> {
        let prime_element = ModularNumber::try_from(&value.prime_element)?;
        let binary_ext_element = GF256::new(value.binary_ext_element);
        Ok(RingTuple::new(prime_element, binary_ext_element))
    }
}

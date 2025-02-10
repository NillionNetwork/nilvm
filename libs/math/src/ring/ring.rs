//! Definition of a ring tuple.

use crate::{
    galois::GF256,
    modular::{DecodeError, Modular, ModularNumber, SophiePrime},
};

use super::EncodedRingTuple;

/// Represents a tuple in the ring Z_{2q}.
#[derive(Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub struct RingTuple<T: Modular> {
    prime_element: ModularNumber<T>,
    binary_ext_element: GF256,
}

impl<T: SophiePrime> RingTuple<T> {
    /// Construct a new ring tuple.
    pub fn new(prime_element: ModularNumber<T>, binary_ext_element: GF256) -> Self {
        Self { prime_element, binary_ext_element }
    }

    /// Get the prime element.
    pub fn prime_element(&self) -> &ModularNumber<T> {
        &self.prime_element
    }

    /// Get the binary extension field element.
    pub fn binary_ext_element(&self) -> &GF256 {
        &self.binary_ext_element
    }

    /// Decomposes the ring tuple into its elements by reference.
    pub fn as_parts(&self) -> (&ModularNumber<T>, &GF256) {
        (&self.prime_element, &self.binary_ext_element)
    }

    /// Decomposes the ring tuple into its elements.
    pub fn into_parts(self) -> (ModularNumber<T>, GF256) {
        (self.prime_element, self.binary_ext_element)
    }

    /// Encode this `RingTuple`.
    pub fn encode(&self) -> EncodedRingTuple {
        EncodedRingTuple::from(self)
    }

    /// Try to decode an encoded share.
    pub fn try_from_encoded(encoded: &EncodedRingTuple) -> Result<Self, DecodeError> {
        encoded.try_into()
    }
}

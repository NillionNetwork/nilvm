//! Binary Extension Field

use rand::Rng;

/// Galois Field 2^8
#[derive(Copy, Clone, Debug, PartialEq, PartialOrd, Ord, Hash, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GF256 {
    value: u8,
}

impl GF256 {
    /// The zero value.
    pub const ZERO: GF256 = GF256 { value: 0 };

    /// The one value.
    pub const ONE: GF256 = GF256 { value: 1 };

    /// Create new GF256
    pub fn new<T: Into<u8>>(value: T) -> GF256 {
        let value = value.into();
        GF256 { value }
    }

    /// Value getter
    #[inline(always)]
    pub fn value(self) -> u8 {
        self.value
    }

    /// Generates a random GF256 number between a range
    pub fn gen_random() -> GF256 {
        let mut rng = rand::thread_rng();
        Self::gen_random_with_rng(&mut rng)
    }

    /// Generates a random GF256 number using the provided RNG.
    pub fn gen_random_with_rng<R: Rng>(rng: &mut R) -> Self {
        let value: u8 = rng.gen();
        GF256::new(value)
    }
}

// These are here to allow making `impl Field for BinaryExtField` work.

impl From<&GF256> for u8 {
    fn from(value: &GF256) -> Self {
        value.value
    }
}

impl From<&u8> for GF256 {
    fn from(value: &u8) -> Self {
        GF256::new(*value)
    }
}

//! Encoding for modular numbers.

use super::{Modular, ModularNumber, UintType};
use num_bigint::{BigInt, BigUint};
use std::marker::PhantomData;

/// An enum representing a modulo to be used in a `ModularNumber`.
///
/// This allows representing any of the supported modulos in non-generic contexts, like sending
/// them through the wire, storing them on a repository, etc.
#[allow(clippy::enum_variant_names)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum EncodedModulo {
    /// The U64 safe prime modulo.
    #[default]
    U64SafePrime,

    /// The U64 Sophie Germain prime modulo.
    U64SophiePrime,

    /// The U64 semi-prime modulo.
    U64SemiPrime,

    /// The U128 safe prime modulo.
    U128SafePrime,

    /// The U128 Sophie Germain prime modulo.
    U128SophiePrime,

    /// The U128 semi-prime modulo.
    U128SemiPrime,

    /// The U256 safe prime modulo.
    U256SafePrime,

    /// The U256 Sophie Germain prime modulo.
    U256SophiePrime,

    /// The U256 semi-prime modulo.
    U256SemiPrime,
}

impl EncodedModulo {
    /// Number of bits in this modular number.
    pub fn bits(&self) -> usize {
        use EncodedModulo::*;

        match self {
            U64SafePrime | U64SophiePrime | U64SemiPrime => 64,
            U128SafePrime | U128SophiePrime | U128SemiPrime => 128,
            U256SafePrime | U256SophiePrime | U256SemiPrime => 256,
        }
    }
    /// Attempt to create a safe prime modulo from the number of bits.
    pub fn try_safe_prime_from_bits(bits: u32) -> Result<EncodedModulo, SafePrimeBitsNotSupported> {
        match bits {
            64 => Ok(EncodedModulo::U64SafePrime),
            128 => Ok(EncodedModulo::U128SafePrime),
            256 => Ok(EncodedModulo::U256SafePrime),
            _ => Err(SafePrimeBitsNotSupported),
        }
    }
}

/// The safe prime bits size is not supported.
#[derive(Debug, thiserror::Error)]
#[error("Supported prime sizes are 64, 128, and 256")]
pub struct SafePrimeBitsNotSupported;

/// An encoded modular number.
///
/// This type allows encoding a [ModularNumber] into a non-generic type that still allows
/// decoding back into a [ModularNumber] if needed.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EncodedModularNumber {
    pub(crate) value: Vec<u8>,
    pub(crate) modulo: EncodedModulo,
}

impl EncodedModularNumber {
    /// Construct a new encoded modular number from its parts.
    ///
    /// Only use if you know what you're doing.
    pub fn new_unchecked(value: Vec<u8>, modulo: EncodedModulo) -> Self {
        Self { value, modulo }
    }

    /// Attempt to decode this modular number.
    pub fn try_decode<T: Modular>(&self) -> Result<ModularNumber<T>, DecodeError> {
        ModularNumber::try_from_encoded(self)
    }

    /// Consume the value and take its underlying bytes
    pub fn into_bytes(self) -> Vec<u8> {
        self.value
    }

    /// Retrieve the underlying bytes.
    pub fn as_bytes(&self) -> &[u8] {
        &self.value
    }

    /// Return true if the value is zero
    pub fn is_zero(&self) -> bool {
        self.value.iter().all(|v| *v == 0)
    }
}

impl TryFrom<&EncodedModularNumber> for BigUint {
    type Error = DecodeError;

    fn try_from(value: &EncodedModularNumber) -> Result<Self, Self::Error> {
        let converter = Box::<dyn AsInteger>::from(&value.modulo);
        converter.as_unsigned_integer(value)
    }
}

impl TryFrom<&EncodedModularNumber> for BigInt {
    type Error = DecodeError;

    fn try_from(value: &EncodedModularNumber) -> Result<Self, Self::Error> {
        let converter = Box::<dyn AsInteger>::from(&value.modulo);
        converter.as_integer(value)
    }
}

/// A decoding error.
#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    /// A mismatch between the encoded and expected types.
    #[error("modulo mismatch")]
    ModuloMismatch,

    /// The value has an unexpected length.
    #[error("invalid value length")]
    ValueLength,

    /// Not implemented.
    #[error("not implemented: {0}")]
    Unimplemented(String),
}

/// A codec for modular numbers.
pub trait Codec: UintType + Sized {
    /// The encoded modulo.
    const ENCODED_MODULO: EncodedModulo;

    /// Encode a modular number.
    fn encode(number: &ModularNumber<Self>) -> EncodedModularNumber;

    /// Try to decode a modular number.
    fn decode(encoded: &EncodedModularNumber) -> Result<ModularNumber<Self>, DecodeError>;
}

// Proxy types to allow decoding an encoded modular number directly into a big int without
// explicitly going through a generic context.
#[derive(Default)]
struct BigUintConverter<T: Modular>(PhantomData<T>);

trait AsInteger {
    fn as_unsigned_integer(&self, number: &EncodedModularNumber) -> Result<BigUint, DecodeError>;
    fn as_integer(&self, number: &EncodedModularNumber) -> Result<BigInt, DecodeError>;
}

impl<T: Modular> AsInteger for BigUintConverter<T> {
    fn as_unsigned_integer(&self, number: &EncodedModularNumber) -> Result<BigUint, DecodeError> {
        let number = number.try_decode::<T>()?;
        Ok(BigUint::from(&number))
    }

    fn as_integer(&self, number: &EncodedModularNumber) -> Result<BigInt, DecodeError> {
        let number = number.try_decode::<T>()?;
        Ok(BigInt::from(&number))
    }
}

crate::impl_boxed_from_encoded_modulo!(BigUintConverter, AsInteger);

/// The modulo is not a safe prime.
#[derive(Debug, thiserror::Error)]
#[error("not a safe prime")]
pub struct NotSafePrime;

/// Allows creating trait objects from an encoded safe prime.
///
/// This is a helpful macro when you want to create a type that will do something in particular
/// based on the modulo type being used. Doing a conversion between an encoded modulo, which is the
/// way modulos are represented in non-generic contexts, would otherwise require a bunch of
/// boilerplate all over the place. This macro allows constructing an instance of a particular
/// type behind a trait and returning it as a trait object.
#[macro_export]
macro_rules! impl_boxed_from_encoded_safe_prime {
    ($type:ident, $trait:ident) => {
        impl TryFrom<&$crate::modular::EncodedModulo> for Box<dyn $trait> {
            type Error = $crate::modular::NotSafePrime;

            #[allow(clippy::box_default)]
            fn try_from(encoded: &$crate::modular::EncodedModulo) -> Result<Box<dyn $trait>, Self::Error> {
                use $crate::modular::encoding::{EncodedModulo::*, NotSafePrime};
                match encoded {
                    U64SafePrime => Ok(Box::new($type::<$crate::modular::U64SafePrime>::default())),
                    U128SafePrime => Ok(Box::new($type::<$crate::modular::U128SafePrime>::default())),
                    U256SafePrime => Ok(Box::new($type::<$crate::modular::U256SafePrime>::default())),
                    _ => Err(NotSafePrime),
                }
            }
        }
    };
}

/// Allows creating trait objects from an encoded modulo.
///
/// This is a version of `impl_boxed_from_encoded_safe_prime` that allows any modulo.
#[macro_export]
macro_rules! impl_boxed_from_encoded_modulo {
    ($type:ident, $trait:ident) => {
        impl From<&$crate::modular::EncodedModulo> for Box<dyn $trait> {
            fn from(encoded: &$crate::modular::EncodedModulo) -> Box<dyn $trait> {
                use $crate::modular::encoding::EncodedModulo::*;
                match encoded {
                    U64SafePrime => Box::<$type<$crate::modular::U64SafePrime>>::default(),
                    U64SophiePrime => Box::<$type<$crate::modular::U64SophiePrime>>::default(),
                    U64SemiPrime => Box::<$type<$crate::modular::U64SemiPrime>>::default(),
                    U128SafePrime => Box::<$type<$crate::modular::U128SafePrime>>::default(),
                    U128SophiePrime => Box::<$type<$crate::modular::U128SophiePrime>>::default(),
                    U128SemiPrime => Box::<$type<$crate::modular::U128SemiPrime>>::default(),
                    U256SafePrime => Box::<$type<$crate::modular::U256SafePrime>>::default(),
                    U256SophiePrime => Box::<$type<$crate::modular::U256SophiePrime>>::default(),
                    U256SemiPrime => Box::<$type<$crate::modular::U256SemiPrime>>::default(),
                }
            }
        }
    };
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::modular::{repr::AsBits, SafePrime, U256SafePrime};
    use rstest::rstest;
    use std::str::FromStr;

    #[derive(Default)]
    struct PrimeWrapper<T: SafePrime>(PhantomData<T>);

    trait Wrap {
        fn bits(&self) -> usize;
    }

    impl<T: SafePrime> Wrap for PrimeWrapper<T> {
        fn bits(&self) -> usize {
            T::MODULO.bits()
        }
    }

    impl_boxed_from_encoded_safe_prime!(PrimeWrapper, Wrap);

    #[rstest]
    #[case(EncodedModulo::U64SafePrime, 64)]
    #[case(EncodedModulo::U128SafePrime, 128)]
    #[case(EncodedModulo::U256SafePrime, 256)]
    fn from_encoded(#[case] modulo: EncodedModulo, #[case] bits: usize) {
        let wrapper = Box::<dyn Wrap>::try_from(&modulo).unwrap();
        assert_eq!(wrapper.bits(), bits);
    }

    #[test]
    fn from_encoded_failure() {
        let result = Box::<dyn Wrap>::try_from(&EncodedModulo::U64SemiPrime);
        assert!(result.is_err());
    }

    #[test]
    fn to_biguint() {
        let string_repr = "115792089237316195423570985008687907853269984665640564039457584007911397392386";
        let expected = BigUint::from_str(string_repr).unwrap();
        let encoded = ModularNumber::<U256SafePrime>::try_from(&expected).unwrap().encode();
        let value = BigUint::try_from(&encoded).expect("conversion failed");
        assert_eq!(value, expected);
    }

    #[test]
    fn to_bigint() {
        let string_repr = "-57896044618658097711785492504343953926634992332820282019728792003955698696192";
        let expected = BigInt::from_str(string_repr).unwrap();
        let encoded = ModularNumber::<U256SafePrime>::try_from(&expected).unwrap().encode();
        let value = BigInt::try_from(&encoded).expect("conversion failed");
        assert_eq!(value, expected);
    }
}

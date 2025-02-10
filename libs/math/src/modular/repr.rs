//! The underlying representation for a modular number.

use super::Codec;
use crate::modular::RemEuclid;
use crypto_bigint::Word;
pub use crypto_bigint::{
    rand_core::CryptoRngCore, Bounded, CheckedAdd, CheckedSub, Encoding, Integer, NonZero, RandomMod, Zero, U128, U256,
    U64,
};
use num_bigint::BigUint;
use std::{
    fmt::{Debug, Display},
    hash::Hash,
    mem::size_of,
    ops::BitAnd,
};
pub use subtle::ConditionallySelectable;
use thiserror::Error;

/// A type that defines how modular arithmetic is performed.
///
/// This trait merges several other traits and allows:
/// * Defining what the modulo used in arithmetic is.
/// * Defining what underlying crypto-bigint type to use. Depending on the size of the modulo, we
///   may use `U64`, `U128`, `U256`, etc.
/// * Defining how modular arithmetic operations are implemented for the underlying type and other
///   properties of it.
pub trait Modular:
    UintType + UintModulo + Clone + Copy + Debug + PartialEq + Eq + PartialOrd + Ord + Codec + Send + Sync + 'static
{
}

impl<
    T: UintType + UintModulo + Clone + Copy + Debug + PartialEq + Eq + PartialOrd + Ord + Codec + Send + Sync + 'static,
> Modular for T
{
}

/// A type that can be used as the representation for modular numbers.
///
/// This trait is a way to use the various `crypto_bigint::Uint<LIMBS>` types (e.g. `U128` and
/// `U256`) without knowing what concrete type we're using.
pub trait Uint:
    Debug
    + Display
    + PartialEq
    + Eq
    + PartialOrd
    + Ord
    + Hash
    + Clone
    + Copy
    + Integer
    + for<'a> BitAnd<&'a Self, Output = Self>
    + Bounded
    + for<'a> RemEuclid<&'a NonZero<Self>, Output = Self>
    + Zero
    + RandomMod
    + From<u64>
    + AsBits
    + ToBigUint
    + TryIntoU64
    + TryFromU8Slice
    + ToU8Vec
{
}

impl<T> Uint for T where
    T: Debug
        + Display
        + PartialEq
        + Eq
        + PartialOrd
        + Ord
        + Hash
        + Clone
        + Copy
        + Integer
        + for<'a> BitAnd<&'a Self, Output = Self>
        + Bounded
        + for<'a> RemEuclid<&'a NonZero<Self>, Output = Self>
        + Zero
        + RandomMod
        + From<u64>
        + AsBits
        + ToBigUint
        + TryIntoU64
        + TryFromU8Slice
        + ToU8Vec
{
}

/// Allows dealing with the bits in a number.
pub trait AsBits {
    /// Get the number of bits in this number.
    fn bits(&self) -> usize;

    /// Get the ith bit in this number.
    fn bit(&self, index: usize) -> bool;
}

// TODO: these operations use vartime.
impl<const LIMBS: usize> AsBits for crypto_bigint::Uint<LIMBS> {
    fn bits(&self) -> usize {
        self.bits_vartime()
    }

    fn bit(&self, index: usize) -> bool {
        self.bit_vartime(index)
    }
}

/// Allows converting a number into a `BigUint`.
pub trait ToBigUint {
    /// Convert this value into a `BigUint`.
    fn to_biguint(&self) -> BigUint;
}

impl<const LIMBS: usize> ToBigUint for crypto_bigint::Uint<LIMBS>
where
    Self: ToU8Vec,
{
    fn to_biguint(&self) -> BigUint {
        BigUint::from_bytes_le(&self.to_u8_vec())
    }
}

/// An error during an attempted conversion to u64
#[derive(Error, Debug, Clone, PartialEq)]
#[error("overflow")]
pub struct Overflow;

/// Trait to convert something into a u64.
///
/// This is a workaround for defining `TryInto` for a foreign type.
pub trait TryIntoU64 {
    /// Attempts to convert self into a u64.
    fn try_into_u64(self) -> Result<u64, Overflow>;
}

impl<const LIMBS: usize> TryIntoU64 for crypto_bigint::Uint<LIMBS> {
    fn try_into_u64(self) -> Result<u64, Overflow> {
        if self > Self::from_u64(u64::MAX) {
            Err(Overflow)
        } else {
            // Limb can be 32 or 64 bits depending on the architecture. e.g. on wasm this uses 32 bits
            #[allow(clippy::unnecessary_cast)]
            let value = self.as_limbs()[0].0 as u64;
            Ok(value)
        }
    }
}

/// A type that can be constructed from a u8 slice.
pub trait TryFromU8Slice: Sized {
    /// Attempt to construct a value from a u8 slice.
    fn try_from_u8_slice(values: &[u8]) -> Result<Self, Overflow>;
}

#[allow(clippy::indexing_slicing, clippy::arithmetic_side_effects)]
impl<const LIMBS: usize> TryFromU8Slice for crypto_bigint::Uint<LIMBS> {
    fn try_from_u8_slice(values: &[u8]) -> Result<Self, Overflow> {
        // This is like `Uint::from_le_slice` except it won't panic if the number is too small and
        // will instead append zeroes at the end.
        let mut input = [Word::from(0u8); LIMBS];
        for (index, chunk) in values.chunks(size_of::<Word>()).enumerate() {
            if index >= input.len() {
                return Err(Overflow);
            }
            let mut word = Word::from(0u8);
            for &value in chunk.iter().rev() {
                word = (word << 8) | value as Word;
            }
            input[index] = word;
        }
        Ok(Self::from_words(input))
    }
}

/// A type that can be converted into a `Vec<u8>`.
///
/// Note that this trait is not symmetric with `TryFromU8Slice` because we lose track of the size
/// of the original input. That is, if the input to `TryFromU8Slice::try_from_u8_slice` did not
/// have the same length as the underlying `Uint` type, then this trait will return a larger
/// array.
pub trait ToU8Vec {
    /// Convert this into a `Vec<u8>`.
    fn to_u8_vec(&self) -> Vec<u8>;
}

impl<const LIMBS: usize> ToU8Vec for crypto_bigint::Uint<LIMBS>
where
    Self: Encoding,
{
    fn to_u8_vec(&self) -> Vec<u8> {
        Vec::from(self.to_le_bytes().as_ref())
    }
}

/// Modular operations.
pub trait ModOps<T> {
    /// The type used as an exponent.
    type Exponent;

    /// Modular addition.
    fn add_mod(lhs: &T, rhs: &T) -> T;

    /// Modular subtraction.
    fn sub_mod(lhs: &T, rhs: &T) -> T;

    /// Modular multiplication.
    fn mul_mod(lhs: &T, rhs: &T) -> T;

    /// Modular exponentiation.
    fn exp_mod(lhs: &T, rhs: &Self::Exponent) -> T;

    /// Modular inverse.
    fn inv_mod(value: &T) -> T;

    /// Negation.
    fn neg_mod(value: &T) -> T;
}

/// Allows defining the representations used for a type.
pub trait UintType: ModOps<Self::Arithmetic, Exponent = Self::Normal> + 'static {
    /// The normal representation of this type.
    ///
    /// This is the default representation and should be used when you want access to the "real"
    /// number.
    type Normal: Uint;

    /// The arithmetic representation of this type.
    ///
    /// This maps to the Montgomery form for a number and therefore should not be used when you
    /// want access to the "real" number, but instead **only** for arithmetic operations.
    type Arithmetic: PartialEq + Eq + Debug + Clone + Copy + Send + Sync;

    /// The zero value in arithmetic form.
    const ARITHMETIC_ZERO: Self::Arithmetic;

    /// The value one in arithmetic form.
    const ARITHMETIC_ONE: Self::Arithmetic;

    /// Converts from a normal form value into an arithmetic one.
    fn to_arithmetic(value: &Self::Normal) -> Self::Arithmetic;

    /// Converts from an arithmetic form value into a normal one.
    fn to_normal(value: &Self::Arithmetic) -> Self::Normal;
}

impl<const LIMBS: usize> RemEuclid<&NonZero<Self>> for crypto_bigint::Uint<LIMBS> {
    type Output = Self;

    fn rem_euclid(self, rhs: &NonZero<Self>) -> Self::Output {
        self.rem(rhs)
    }
}

/// A number that has an associated modulo.
pub trait UintModulo: UintType {
    /// The modulo to be used.
    const MODULO: Self::Normal;
}

/// A number that contains a generator for its associated field.
pub trait Generator<T> {
    /// The generator for this number's field.
    const GENERATOR: T;
}

/// A marker trait for prime numbers.
///
/// This is obviously just a marker so it should be used with caution only when defining types that
/// represent prime numbers.
pub trait Prime: Modular {}

/// A safe prime number. That is, a prime `p` such that `p = 2q + 1` where `q` is another prime
/// number.
pub trait SafePrime: Modular + Generator<Self::Arithmetic> + Prime {
    /// The Sophie Germaine prime for this safe prime, AKA "q".
    type SophiePrime: Modular<Normal = Self::Normal> + SophiePrime<SemiPrime = Self::SemiPrime>;

    /// A "semi-prime" for this safe prime. That is, 2q.
    type SemiPrime: Modular<Normal = Self::Normal>;
}

/// A Sophie Germain prime. That is, a prime `q` such that `p = 2q + 1` where `p` is another prime
/// number.
pub trait SophiePrime: Modular + Prime {
    /// The safe prime for this Sophie Germain prime. That is, p.
    type SafePrime: Modular<Normal = Self::Normal> + SafePrime<SemiPrime = Self::SemiPrime>;

    /// The "semi-prime" for this Sophie Germain prime. That is, 2q.
    type SemiPrime: Modular<Normal = Self::Normal>;
}

/// Implements modular operations for prime/odd modulos.
///
/// Macro adapted from https://docs.rs/crypto-bigint/0.5.2/src/crypto_bigint/uint/modular/constant_mod/macros.rs.html
///
/// This needed to be adapted as the original macro defines a new type and we want to stick
/// everything under the same one.
macro_rules! impl_prime_mod_ops {
    ($name:ident, $uint_type:ty) => {
        #[allow(clippy::arithmetic_side_effects)]
        impl<const DLIMBS: usize>
            crypto_bigint::modular::constant_mod::ResidueParams<{ crypto_bigint::nlimbs!(<$uint_type>::BITS) }>
            for $name
        where
            crypto_bigint::Uint<{ crypto_bigint::nlimbs!(<$uint_type>::BITS) }>:
                crypto_bigint::Concat<Output = crypto_bigint::Uint<DLIMBS>>,
            Self: $crate::modular::UintModulo,
        {
            const LIMBS: usize = { crypto_bigint::nlimbs!(<$uint_type>::BITS) };
            const MODULUS: crypto_bigint::Uint<{ crypto_bigint::nlimbs!(<$uint_type>::BITS) }> =
                <$name as $crate::modular::UintModulo>::MODULO;
            const R: crypto_bigint::Uint<{ crypto_bigint::nlimbs!(<$uint_type>::BITS) }> =
                crypto_bigint::Uint::MAX.const_rem(&Self::MODULUS).0.wrapping_add(&crypto_bigint::Uint::ONE);
            const R2: crypto_bigint::Uint<{ crypto_bigint::nlimbs!(<$uint_type>::BITS) }> =
                crypto_bigint::Uint::const_rem_wide(Self::R.square_wide(), &Self::MODULUS).0;
            const MOD_NEG_INV: crypto_bigint::Limb = crypto_bigint::Limb(
                crypto_bigint::Word::MIN
                    .wrapping_sub(Self::MODULUS.inv_mod2k(crypto_bigint::Word::BITS as usize).as_limbs()[0].0),
            );
            const R3: crypto_bigint::Uint<{ crypto_bigint::nlimbs!(<$uint_type>::BITS) }> =
                crypto_bigint::modular::montgomery_reduction(
                    &Self::R2.square_wide(),
                    &Self::MODULUS,
                    Self::MOD_NEG_INV,
                );
        }

        impl $crate::modular::ModOps<crypto_bigint::modular::constant_mod::Residue<Self, { <$uint_type>::LIMBS }>>
            for $name
        {
            type Exponent = <Self as $crate::modular::UintType>::Normal;

            fn add_mod(
                lhs: &crypto_bigint::modular::constant_mod::Residue<Self, { <$uint_type>::LIMBS }>,
                rhs: &crypto_bigint::modular::constant_mod::Residue<Self, { <$uint_type>::LIMBS }>,
            ) -> crypto_bigint::modular::constant_mod::Residue<Self, { <$uint_type>::LIMBS }> {
                lhs.add(rhs)
            }

            fn sub_mod(
                lhs: &crypto_bigint::modular::constant_mod::Residue<Self, { <$uint_type>::LIMBS }>,
                rhs: &crypto_bigint::modular::constant_mod::Residue<Self, { <$uint_type>::LIMBS }>,
            ) -> crypto_bigint::modular::constant_mod::Residue<Self, { <$uint_type>::LIMBS }> {
                lhs.sub(rhs)
            }

            fn mul_mod(
                lhs: &crypto_bigint::modular::constant_mod::Residue<Self, { <$uint_type>::LIMBS }>,
                rhs: &crypto_bigint::modular::constant_mod::Residue<Self, { <$uint_type>::LIMBS }>,
            ) -> crypto_bigint::modular::constant_mod::Residue<Self, { <$uint_type>::LIMBS }> {
                lhs.mul(rhs)
            }

            fn exp_mod(
                lhs: &crypto_bigint::modular::constant_mod::Residue<Self, { <$uint_type>::LIMBS }>,
                rhs: &Self::Exponent,
            ) -> crypto_bigint::modular::constant_mod::Residue<Self, { <$uint_type>::LIMBS }> {
                lhs.pow(rhs)
            }

            fn inv_mod(
                value: &crypto_bigint::modular::constant_mod::Residue<Self, { <$uint_type>::LIMBS }>,
            ) -> crypto_bigint::modular::constant_mod::Residue<Self, { <$uint_type>::LIMBS }> {
                value.invert().0
            }

            fn neg_mod(
                value: &crypto_bigint::modular::constant_mod::Residue<Self, { <$uint_type>::LIMBS }>,
            ) -> crypto_bigint::modular::constant_mod::Residue<Self, { <$uint_type>::LIMBS }> {
                value.neg()
            }
        }
    };
}

/// Implements modular operations for even modulos.
macro_rules! impl_even_mod_ops {
    ($name:ident, $uint_type:ty) => {
        impl $crate::modular::ModOps<$uint_type> for $name {
            type Exponent = <Self as $crate::modular::UintType>::Normal;

            fn add_mod(lhs: &$uint_type, rhs: &$uint_type) -> $uint_type {
                lhs.add_mod(rhs, &<Self as $crate::modular::UintModulo>::MODULO)
            }

            fn sub_mod(lhs: &$uint_type, rhs: &$uint_type) -> $uint_type {
                lhs.sub_mod(rhs, &<Self as $crate::modular::UintModulo>::MODULO)
            }

            fn mul_mod(_lhs: &$uint_type, _rhs: &$uint_type) -> $uint_type {
                panic!("multiplication unimplemented for even modulos")
            }

            fn exp_mod(_lhs: &$uint_type, _rhs: &$uint_type) -> $uint_type {
                panic!("exp_mod unimplemented for even modulos");
            }

            fn inv_mod(_value: &$uint_type) -> $uint_type {
                panic!("exp_mod unimplemented for even modulos");
            }

            fn neg_mod(value: &$uint_type) -> $uint_type {
                value.neg_mod(&<Self as $crate::modular::UintModulo>::MODULO)
            }
        }
    };
}

pub(crate) use impl_even_mod_ops;
pub(crate) use impl_prime_mod_ops;

#[cfg(test)]
mod test {
    use super::*;
    use crate::modular::{U128SafePrime, U256SafePrime, U64SafePrime};
    use rstest::rstest;

    #[rstest]
    #[case::u64(U64SafePrime)]
    #[case::u128(U128SafePrime)]
    #[case::u256(U256SafePrime)]
    fn try_into_u64_success<T: UintType>(#[case] _prime: T) {
        let value = T::Normal::from(u64::MAX);
        let converted = value.try_into_u64().expect("conversion failed");
        assert_eq!(u64::MAX, converted);
    }

    #[rstest]
    #[case::u128(U128SafePrime)]
    #[case::u256(U256SafePrime)]
    fn try_into_u64_faiure<T: UintType>(#[case] _prime: T) {
        let value = T::Normal::from(u64::MAX);
        let value = value.checked_add(&T::Normal::from(1)).unwrap();
        let result = value.try_into_u64();
        assert!(result.is_err());
    }

    #[rstest]
    #[case::u64_partial(U64SafePrime, 4)]
    #[case::u64_full(U64SafePrime, 8)]
    #[case::u128_partial(U128SafePrime, 4)]
    #[case::u128_full(U128SafePrime, 16)]
    #[case::u256_partial(U256SafePrime, 20)]
    #[case::u256_full(U256SafePrime, 32)]
    fn try_from_slice_success<T: UintType>(#[case] _prime: T, #[case] length: usize) {
        let value: Vec<_> = (1..=length).map(|value| (value % 256) as u8).collect();

        let number = T::Normal::try_from_u8_slice(&value).expect("construction failed");
        let serialized: Vec<_> = number.to_u8_vec().into_iter().take(length).collect();
        assert_eq!(serialized, value);
    }

    #[test]
    fn one_encoding() {
        let one = U256::from(1u32);
        let bytes = one.to_u8_vec();
        assert_eq!(
            bytes,
            vec![1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
        );
    }
}

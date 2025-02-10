//! The supported modulos.
//!
//! [ModularNumber] can only be instantiated with the modulos defined in this module.

use super::ModularNumber;
use crypto_bigint::{Encoding, U128, U256, U64};

// Implements the `Codec` trait for a particular type.
//
// This will assume that `Codec::<$modulo>` exists, which should be the case for all "official"
// modulos defined below.
macro_rules! impl_codec {
    ($modulo:ident) => {
        impl $crate::modular::Codec for $modulo {
            const ENCODED_MODULO: $crate::modular::EncodedModulo =
                paste::paste! { $crate::modular::EncodedModulo::$modulo };

            fn encode(number: &ModularNumber<Self>) -> $crate::modular::EncodedModularNumber {
                // let modulo = $modulo_variant;
                let modulo = paste::paste! { $crate::modular::EncodedModulo::$modulo };
                let value = number.into_value().to_le_bytes().to_vec();
                $crate::modular::EncodedModularNumber { value, modulo }
            }

            fn decode(
                encoded: &$crate::modular::EncodedModularNumber,
            ) -> Result<ModularNumber<Self>, $crate::modular::DecodeError> {
                match encoded.modulo {
                    paste::paste! { $crate::modular::EncodedModulo::$modulo } => {
                        let value: [u8; { <$modulo as $crate::modular::UintType>::Normal::BYTES }] = encoded
                            .value
                            .as_slice()
                            .try_into()
                            .map_err(|_| $crate::modular::DecodeError::ValueLength)?;
                        let value = <$modulo as $crate::modular::UintType>::Normal::from_le_bytes(value);
                        let value = ModularNumber::new(value);
                        Ok(value)
                    }
                    _ => Err($crate::modular::DecodeError::ModuloMismatch),
                }
            }
        }
    };
}

/// Defines a safe prime and its associated primes.
macro_rules! safe_prime {
    ($safe_prime_name:ident, $sophie_prime_name:ident, $semi_prime_name:ident, $type:ty, $prime:expr, $string_repr:literal, $generator:expr) => {
        #[doc = concat!("The semi-prime for [", stringify!($safe_prime_name), "]")]
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
        pub struct $semi_prime_name;

        impl $crate::modular::repr::UintType for $semi_prime_name {
            type Normal = $type;
            type Arithmetic = $type;

            const ARITHMETIC_ZERO: Self::Arithmetic = Self::Arithmetic::ZERO;
            const ARITHMETIC_ONE: Self::Arithmetic = Self::Arithmetic::ONE;

            fn to_arithmetic(value: &Self::Normal) -> Self::Arithmetic {
                *value
            }

            fn to_normal(value: &Self::Arithmetic) -> Self::Normal {
                *value
            }
        }

        impl $crate::modular::repr::UintModulo for $semi_prime_name {
            const MODULO: $type = $prime.sub_mod(&<$type>::ONE, &$prime);
        }

        $crate::modular::impl_even_mod_ops!($semi_prime_name, $type);

        #[doc = concat!("The safe prime ", $string_repr)]
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
        pub struct $safe_prime_name;

        impl $crate::modular::repr::UintType for $safe_prime_name {
            type Normal = $type;
            type Arithmetic = crypto_bigint::modular::constant_mod::Residue<$safe_prime_name, { <$type>::LIMBS }>;

            const ARITHMETIC_ZERO: Self::Arithmetic = Self::Arithmetic::ZERO;
            const ARITHMETIC_ONE: Self::Arithmetic = Self::Arithmetic::ONE;

            fn to_arithmetic(value: &Self::Normal) -> Self::Arithmetic {
                Self::Arithmetic::new(value)
            }

            fn to_normal(value: &Self::Arithmetic) -> Self::Normal {
                value.retrieve()
            }
        }

        impl $crate::modular::Prime for $safe_prime_name {}

        impl $crate::modular::repr::UintModulo for $safe_prime_name {
            const MODULO: $type = $prime;
        }

        impl
            $crate::modular::repr::Generator<
                crypto_bigint::modular::constant_mod::Residue<$safe_prime_name, { <$type>::LIMBS }>,
            > for $safe_prime_name
        {
            const GENERATOR: crypto_bigint::modular::constant_mod::Residue<$safe_prime_name, { <$type>::LIMBS }> =
                <Self as $crate::modular::UintType>::Arithmetic::new(&$generator);
        }

        $crate::modular::impl_prime_mod_ops!($safe_prime_name, $type);

        #[doc = concat!("The Sophie Germain prime for [", stringify!($safe_prime_name), "]")]
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
        pub struct $sophie_prime_name;

        impl $crate::modular::repr::UintType for $sophie_prime_name {
            type Normal = $type;
            type Arithmetic = crypto_bigint::modular::constant_mod::Residue<$sophie_prime_name, { <$type>::LIMBS }>;

            const ARITHMETIC_ZERO: Self::Arithmetic = Self::Arithmetic::ZERO;
            const ARITHMETIC_ONE: Self::Arithmetic = Self::Arithmetic::ONE;

            fn to_arithmetic(value: &Self::Normal) -> Self::Arithmetic {
                Self::Arithmetic::new(value)
            }

            fn to_normal(value: &Self::Arithmetic) -> Self::Normal {
                value.retrieve()
            }
        }

        impl $crate::modular::Prime for $sophie_prime_name {}

        impl $crate::modular::repr::UintModulo for $sophie_prime_name {
            const MODULO: $type = <$safe_prime_name>::MODULO.sub_mod(&<$type>::ONE, &$prime).shr_vartime(1);
        }

        $crate::modular::impl_prime_mod_ops!($sophie_prime_name, $type);

        // Map the safe prime to this Sophie Germain prime.
        impl $crate::modular::SafePrime for $safe_prime_name {
            type SophiePrime = $sophie_prime_name;
            type SemiPrime = $semi_prime_name;
        }

        // Map the Sophie Germain prime to this prime.
        impl $crate::modular::SophiePrime for $sophie_prime_name {
            type SafePrime = $safe_prime_name;
            type SemiPrime = $semi_prime_name;
        }
    };
}

macro_rules! crypto_bigint_safe_prime {
    ($(($type:ty, $prime:expr, $string_repr:literal, $generator:expr)),+) => {
        $(
            paste::paste! {
                safe_prime!(
                    [<$type SafePrime>],
                    [<$type SophiePrime>],
                    [<$type SemiPrime>],
                    $type,
                    $prime,
                    $string_repr,
                    $generator
                );
                $crate::modular::modulos::impl_codec!([<$type SafePrime>]);
                $crate::modular::modulos::impl_codec!([<$type SophiePrime>]);
                $crate::modular::modulos::impl_codec!([<$type SemiPrime>]);
            }
        )+
    };
}

pub(crate) use impl_codec;
#[cfg(test)]
pub(crate) use safe_prime;

// Define the prime types that we can operate on.
//
// This defines types `<crypto-bigint type>::(Safe|Sohie|Semi)Prime`. e.g. `U64SafePrime`,
// `U128SemiPrime`, etc.
crypto_bigint_safe_prime!(
    (U64, U64::from_u64(18446744072637906947), "18446744072637906947", U64::from_u32(20)),
    (
        U128,
        U128::from_u128(340282366920938463463374607429104828419),
        "340282366920938463463374607429104828419",
        U128::from_u32(20)
    ),
    (
        U256,
        U256::from_be_hex("ffffffffffffffffffffffffffffffffffffffffffffffffffffffff98c00003"),
        "115792089237316195423570985008687907853269984665640564039457584007911397392387",
        U256::from_u32(20)
    )
);

// NOTE: if you're adding a new prime type, you need to call the macro to define secret sharing for
// it in the `shamir` crate.

#[cfg(test)]
mod test {
    use super::*;
    use crate::modular::{DecodeError, EncodedModularNumber, EncodedModulo, Modular};
    use rstest::rstest;

    #[rstest]
    #[case::u64_safe(U64SafePrime)]
    #[case::u64_sophie(U64SophiePrime)]
    #[case::u64_semi(U64SemiPrime)]
    #[case::u128_safe(U128SafePrime)]
    #[case::u128_sophie(U128SophiePrime)]
    #[case::u128_semi(U128SemiPrime)]
    #[case::u256_safe(U256SafePrime)]
    #[case::u256_sophie(U256SophiePrime)]
    #[case::u256_semi(U256SemiPrime)]
    fn conversions<T>(#[case] _modulo: T)
    where
        T: Modular,
    {
        let one = ModularNumber::<T>::ONE;
        let encoded = one.encode();
        let decoded = ModularNumber::try_from_encoded(&encoded).expect("decoding failed");
        assert_eq!(decoded, one);
    }

    #[test]
    fn convert_invalid_modulo() {
        let value = ModularNumber::<U64SafePrime>::ONE;
        let encoded = value.encode();
        let result = ModularNumber::<U64SemiPrime>::try_from_encoded(&encoded);
        assert!(matches!(result, Err(DecodeError::ModuloMismatch)));
    }

    #[test]
    fn convert_invalid_value_length() {
        let encoded = EncodedModularNumber { value: vec![42; 100], modulo: EncodedModulo::U64SafePrime };
        let result = ModularNumber::<U64SafePrime>::try_from_encoded(&encoded);
        assert!(matches!(result, Err(DecodeError::ValueLength)));
    }
}

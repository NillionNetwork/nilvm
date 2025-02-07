//! Test macros.

/// Defines a fake encoding for a prime number to satisfy `Modular`.
#[macro_export]
macro_rules! fake_modulo_codec {
    ($name:ident) => {
        impl $crate::modular::Codec for $name {
            const ENCODED_MODULO: $crate::modular::EncodedModulo = unimplemented!();

            fn encode(_: &$crate::modular::ModularNumber<Self>) -> $crate::modular::EncodedModularNumber {
                panic!("test prime numbers can't be encoded");
            }

            fn decode(
                _: &$crate::modular::EncodedModularNumber,
            ) -> Result<$crate::modular::ModularNumber<Self>, $crate::modular::DecodeError> {
                panic!("test prime numbers can't be decoded");
            }
        }
    };
}

/// Defines a prime used for testing.
#[macro_export]
macro_rules! test_prime {
    ($name:ident, $value:literal) => {
        /// A prime number.
        #[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
        struct $name;

        impl $crate::modular::UintType for $name {
            type Normal = crypto_bigint::U64;
            type Arithmetic = crypto_bigint::modular::constant_mod::Residue<$name, { crypto_bigint::U64::LIMBS }>;
            const ARITHMETIC_ZERO: Self::Arithmetic = Self::Arithmetic::ZERO;
            const ARITHMETIC_ONE: Self::Arithmetic = Self::Arithmetic::ONE;

            fn to_arithmetic(value: &Self::Normal) -> Self::Arithmetic {
                Self::Arithmetic::new(value)
            }

            fn to_normal(value: &Self::Arithmetic) -> Self::Normal {
                value.retrieve()
            }
        }

        impl $crate::modular::UintModulo for $name {
            const MODULO: crypto_bigint::U64 = crypto_bigint::U64::from_u64($value);
        }

        impl $crate::modular::Prime for $name {}

        $crate::modular::impl_prime_mod_ops!($name, crypto_bigint::U64);
        $crate::fake_modulo_codec!($name);
    };
}

/// Defines a safe prime used for testing.
#[macro_export]
macro_rules! test_safe_prime {
    ($safe_prime_name:ident, $sophie_prime_name:ident, $semi_prime_name:ident, $type:ty, $prime:expr, $generator:expr) => {
        $crate::modular::modulos::safe_prime!(
            $safe_prime_name,
            $sophie_prime_name,
            $semi_prime_name,
            $type,
            $prime,
            "",
            $generator
        );
        $crate::fake_modulo_codec!($safe_prime_name);
        $crate::fake_modulo_codec!($sophie_prime_name);
        $crate::fake_modulo_codec!($semi_prime_name);
    };
}

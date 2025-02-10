//! Modular Big Integers

use super::{DecodeError, EncodedModularNumber, Generator, Modular, Overflow, ToU8Vec, TryFromU8Slice, UintType};
use crate::modular::{RemEuclid, ToBigUint};
use crypto_bigint::{rand_core::CryptoRngCore, NonZero, RandomMod};
use num_bigint::{BigInt, BigUint, Sign};
use std::{
    fmt::{Debug, Display, Formatter},
    hash::Hash,
    str::FromStr,
};

/// A number that performs modular arithmetic in every operation.
///
/// `ModularNumber<T>` allows modular arithmetic on the modulo provided by its generic type `T`.
///
/// Under the hood, this type keeps the value it represents in Montgomery form. This allows
/// arithmetic operations to run faster, at the cost of an extra Montgomery reduction when you want
/// access to the value in normal form.
///
/// # Examples
///
/// Use specific prime number types to create modular numbers and operate on them.
///
/// ```
/// use math_lib::modular::{ModularNumber, U64SafePrime};
///
/// let two = ModularNumber::<U64SafePrime>::from_u32(2);
/// let one = ModularNumber::ONE;
/// let three = two + &one;
/// let six = three * &two;
///
/// assert_eq!(six, ModularNumber::from_u32(6));
/// ```
///
/// # num_bigint conversions
///
/// [ModularNumber] can be converted to/from [num_bigint::BigUint] types. This is a costly
/// operation but can be used when you want a non-generic but useable type:
///
/// ```
/// use math_lib::modular::{ModularNumber, U64SafePrime};
/// use num_bigint::BigUint;
///
/// # fn test() -> anyhow::Result<()> {
/// let forty_two = BigUint::from(42u32);
/// let forty_two_modular = ModularNumber::<U64SafePrime>::try_from(&forty_two)?;
/// assert_eq!(BigUint::from(&forty_two_modular), forty_two);
/// # Ok(())
/// # }
/// ```
#[derive(Eq, PartialEq, Clone, Copy)]
pub struct ModularNumber<T: UintType> {
    pub(crate) value: T::Arithmetic,
}

impl<T: Modular> ModularNumber<T> {
    /// The modulo being used.
    pub const MODULO: T::Normal = T::MODULO;

    /// The zero value.
    pub const ZERO: Self = ModularNumber { value: T::ARITHMETIC_ZERO };

    /// The value one.
    pub const ONE: Self = ModularNumber { value: T::ARITHMETIC_ONE };

    /// Two.
    pub fn two() -> Self {
        ModularNumber::ONE + &ModularNumber::ONE
    }

    /// Constructs a new modular number.
    ///
    /// This takes the value in normal form. For conversions from [u32] or [u64] use
    /// [ModularNumber::from_u32] and [ModularNumber::from_u64] respectively.
    pub fn new(value: T::Normal) -> Self {
        let value = if value >= Self::MODULO { value.rem_euclid(&NonZero::new(Self::MODULO).unwrap()) } else { value };
        Self { value: T::to_arithmetic(&value) }
    }

    /// Constructs a modular number from a u32.
    pub fn from_u32(value: u32) -> Self {
        Self::new(T::Normal::from(value as u64))
    }

    /// Constructs a modular number from a u64.
    pub fn from_u64(value: u64) -> Self {
        Self::new(T::Normal::from(value))
    }

    /// Generates a random modular number.
    pub fn gen_random() -> Self {
        let mut rng = rand::thread_rng();
        Self::gen_random_with_rng(&mut rng)
    }

    /// Generates a random number using the provided random number generator.
    pub fn gen_random_with_rng<R: CryptoRngCore>(rng: &mut R) -> Self {
        let prime = NonZero::new(Self::MODULO).unwrap();
        let value = T::Normal::random_mod(rng, &prime);
        ModularNumber::new(value)
    }

    /// Check if this modular number is zero.
    pub fn is_zero(&self) -> bool {
        self.value == T::ARITHMETIC_ZERO
    }

    /// Check if this modular number is one.
    pub fn is_one(&self) -> bool {
        self.value == T::ARITHMETIC_ONE
    }

    /// Consume the modular number and return the inner value.
    ///
    /// This converts the value from Montgomery form into normal form so it has a non-negligible
    /// cost.
    pub fn into_value(&self) -> T::Normal {
        T::to_normal(&self.value)
    }

    /// Encode this modular number.
    ///
    /// This can be used to turn a `ModularNumber` into a non-generic type.
    pub fn encode(&self) -> EncodedModularNumber {
        T::encode(self)
    }

    /// Attempt to decode a modular number.
    pub fn try_from_encoded(encoded: &EncodedModularNumber) -> Result<Self, DecodeError> {
        T::decode(encoded)
    }

    /// Constructs a modular number from a u8 slice.
    pub fn try_from_u8_slice(value: &[u8]) -> Result<Self, Overflow> {
        let value = T::Normal::try_from_u8_slice(value)?;
        Ok(ModularNumber::new(value))
    }

    /// Absolute value of the modular number.
    pub fn abs(&self) -> Self {
        let mut r = *self;
        if !r.is_positive() {
            r = -r;
        }
        r
    }

    /// Sign value of the modular number.
    pub fn is_positive(&self) -> bool {
        let two = NonZero::new(ModularNumber::<T>::from_u32(2).into_value()).unwrap();
        let threshold: ModularNumber<T> = ModularNumber::new(T::MODULO / two);
        if self > &threshold {
            return false;
        }
        true
    }
}

impl<T: Modular> Default for ModularNumber<T> {
    fn default() -> Self {
        Self::ZERO
    }
}

impl<T: Modular> From<&ModularNumber<T>> for BigUint {
    fn from(value: &ModularNumber<T>) -> Self {
        value.into_value().to_biguint()
    }
}

impl<T: Modular> TryFrom<&BigUint> for ModularNumber<T> {
    type Error = Overflow;

    fn try_from(value: &BigUint) -> Result<Self, Self::Error> {
        let value = value.to_bytes_le();
        let value = T::Normal::try_from_u8_slice(&value)?;
        if value >= T::MODULO {
            return Err(Overflow);
        }
        Ok(ModularNumber::new(value))
    }
}

impl<T: Modular> From<&ModularNumber<T>> for BigInt {
    fn from(value: &ModularNumber<T>) -> Self {
        if value.is_positive() {
            BigInt::from_biguint(Sign::Plus, BigUint::from(value))
        } else {
            BigInt::from_biguint(Sign::Minus, BigUint::from(&-value))
        }
    }
}

impl<T: Modular> TryFrom<&BigInt> for ModularNumber<T> {
    type Error = Overflow;

    fn try_from(value: &BigInt) -> Result<Self, Self::Error> {
        let sign = value.sign();
        let value = ModularNumber::<T>::try_from(value.magnitude())?;
        if !value.is_positive() {
            return Err(Overflow);
        }
        let value = match sign {
            Sign::Minus => -value,
            _ => value,
        };
        Ok(value)
    }
}

// Note: this is inefficient as we can't compare numbers in Montgomery form.
//
// We shouldn't be using comparisons anywhere outside tests and even an occasional conversion back
// to normal form for comparison should affect performance negligibly.
impl<T: Modular> PartialOrd for ModularNumber<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: Modular> Ord for ModularNumber<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.into_value().cmp(&other.into_value())
    }
}

impl<T: Modular> Hash for ModularNumber<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.into_value().hash(state)
    }
}

impl<T: Modular + Generator<T::Arithmetic>> ModularNumber<T> {
    /// The generator for the field used in this modular number.
    pub const GENERATOR: ModularNumber<T> = ModularNumber { value: T::GENERATOR };
}

// String conversions.

impl<T: Modular> Debug for ModularNumber<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let value = BigUint::from(self);
        let modulo = T::MODULO.to_biguint();
        write!(f, "{value} mod {modulo}")
    }
}

impl<T: Modular> Display for ModularNumber<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let value = BigUint::from(self);
        write!(f, "{value}")
    }
}

/// An error when parsing a modular number.
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    /// The input value contained invalid digits.
    #[error("invalid digits")]
    InvalidDigits,

    /// The input value contained a number that required more bytes than the underlying type
    /// allows.
    #[error("value is too large")]
    Overflow,
}

impl<T: Modular> FromStr for ModularNumber<T> {
    type Err = ParseError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let parsed = BigUint::from_str(input).map_err(|_| ParseError::InvalidDigits)?;
        let bytes = parsed.to_bytes_le();
        let value = T::Normal::try_from_u8_slice(&bytes).map_err(|_| ParseError::Overflow)?;
        Ok(ModularNumber::new(value))
    }
}

// General conversions.

impl<T: Modular> From<&ModularNumber<T>> for EncodedModularNumber {
    fn from(value: &ModularNumber<T>) -> Self {
        value.encode()
    }
}

impl<T: Modular> TryFrom<&EncodedModularNumber> for ModularNumber<T> {
    type Error = DecodeError;

    fn try_from(value: &EncodedModularNumber) -> Result<Self, Self::Error> {
        ModularNumber::try_from_encoded(value)
    }
}

impl<T: Modular> From<&ModularNumber<T>> for Vec<u8> {
    fn from(value: &ModularNumber<T>) -> Self {
        value.into_value().to_u8_vec()
    }
}

impl<T: Modular> TryFrom<&[u8]> for ModularNumber<T> {
    type Error = Overflow;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        let value = T::Normal::try_from_u8_slice(value)?;
        Ok(ModularNumber::new(value))
    }
}

#[cfg(test)]
mod test {
    use std::ops::Add;

    use super::*;
    use crate::{
        modular::{U128SafePrime, U256SafePrime, U64SafePrime},
        test_prime,
    };
    use crypto_bigint::U256;
    use rstest::rstest;

    test_prime!(P11, 11u64);

    #[rstest]
    #[case(0, 0)]
    #[case(10, 10)]
    #[case(11, 0)]
    #[case(12, 1)]
    #[case(15, 4)]
    fn test_construction_mod_11(#[case] value: u32, #[case] expected: u32) {
        let value = ModularNumber::<P11>::from_u32(value);
        let expected = ModularNumber::<P11>::from_u32(expected);
        assert_eq!(value, expected);
    }

    #[rstest]
    #[case::u64(U64SafePrime)]
    #[case::u128(U128SafePrime)]
    #[case::u256(U256SafePrime)]
    fn string_conversions<T: Modular>(#[case] _prime: T) {
        let value = "16045690985374408367";
        let parsed = ModularNumber::<T>::from_str(value).expect("parsing failed");
        assert_eq!(parsed.to_string(), value);
    }

    #[rstest]
    #[case::overflow(U64SafePrime, "999999999999999999999999999999999999")]
    #[case::invalid_value(U64SafePrime, "potato")]
    #[case::partially_invalid_value(U64SafePrime, "42potato")]
    fn invalid_string_values<T: Modular>(#[case] _prime: T, #[case] value: &str) {
        let result = ModularNumber::<T>::from_str(value);
        assert!(result.is_err());
    }

    #[test]
    fn debug() {
        let value = ModularNumber::<U64SafePrime>::from_u32(42);
        let formatted = format!("{value:?}");
        assert_eq!(formatted, "42 mod 18446744072637906947");
    }

    #[test]
    fn large_number_to_string() {
        let value = ModularNumber::<U256SafePrime>::new(U256::from_be_hex(
            "ffffffffffffffffffffffffffffffffffffffffffffffffffffffff98c00002",
        ));
        assert_eq!(value.to_string(), "115792089237316195423570985008687907853269984665640564039457584007911397392386");
    }

    #[rstest]
    #[case::small("42")]
    #[case::larger("115792089237316195423570985008687907853269984665640564039457584007911397392386")]
    fn biguint_conversion(#[case] input: &str) {
        let original = BigUint::from_str(input).unwrap();
        let modular = ModularNumber::<U256SafePrime>::try_from(&original).expect("conversion failed");
        let output = BigUint::from(&modular);
        assert_eq!(output, original);
    }

    #[test]
    fn to_biguint_overflow() {
        // 2 ** 64
        let value = BigUint::from(18446744073709551616_u128);
        let result = ModularNumber::<U64SafePrime>::try_from(&value);
        assert!(result.is_err());
    }

    #[rstest]
    #[case::zero("0")]
    #[case::positive("42")]
    #[case::negative("-42")]
    #[case::maximum("57896044618658097711785492504343953926634992332820282019728792003955698696193")]
    #[case::minimum("-57896044618658097711785492504343953926634992332820282019728792003955698696193")]
    fn bigint_conversion(#[case] input: &str) {
        let original = BigInt::from_str(input).unwrap();
        let modular = ModularNumber::<U256SafePrime>::try_from(&original).expect("conversion failed");
        let output = BigInt::from(&modular);
        assert_eq!(output, original);
    }

    #[rstest]
    #[case::maximum("57896044618658097711785492504343953926634992332820282019728792003955698696194")]
    #[case::minimum("-57896044618658097711785492504343953926634992332820282019728792003955698696194")]
    fn to_bigint_overflow(#[case] input: &str) {
        let original = BigInt::from_str(input).unwrap();
        let result = ModularNumber::<U256SafePrime>::try_from(&original);
        assert!(result.is_err());
    }

    #[test]
    fn add_two_negative_numbers() {
        let first = BigInt::from(-2);
        let second = BigInt::from(-3);
        let first_modular = ModularNumber::<U64SafePrime>::try_from(&first).unwrap();
        let second_modular = ModularNumber::<U64SafePrime>::try_from(&second).unwrap();
        let modular_addition = first_modular.add(&second_modular);
        assert_eq!(BigInt::from(-5), BigInt::from(&modular_addition));
    }

    #[test]
    fn test_abs() {
        let minus_two = BigInt::from(-2);
        let two = BigInt::from(2);
        let minus_two_modular = ModularNumber::<U64SafePrime>::try_from(&minus_two).unwrap();
        let two_modular = ModularNumber::<U64SafePrime>::try_from(&two).unwrap();
        assert_eq!(two, BigInt::from(&minus_two_modular.abs()));
        assert_eq!(two, BigInt::from(&two_modular.abs()));
    }

    #[test]
    fn test_sign() {
        let minus_two = BigInt::from(-2);
        let two = BigInt::from(2);
        let minus_two_modular = ModularNumber::<U64SafePrime>::try_from(&minus_two).unwrap();
        let two_modular = ModularNumber::<U64SafePrime>::try_from(&two).unwrap();
        assert!(!minus_two_modular.is_positive());
        assert!(two_modular.is_positive());
    }
}

//! Definitions for fields.

use crate::{
    errors::DivByZero,
    galois::GF256,
    modular::{EncodedModularNumber, Modular, ModularNumber, Prime},
    polynomial::Polynomial,
    serde::Serde,
};
use crypto_bigint::rand_core::CryptoRngCore;
use std::{
    cmp::PartialEq,
    convert::Infallible,
    fmt::Debug,
    hash::Hash,
    marker::PhantomData,
    ops::{Add, Div, Mul, Neg, Sub},
};

/// Multiplicative inverse of a field element.
pub trait Inv {
    /// Inverse of Field Element.
    type Output;

    /// Multiplicative inverse.
    fn inv(self) -> Self::Output;
}

/// A finite field that has its own way to define elements and a way of reconstructing them.
pub trait Field: Clone + Serde {
    /// The type used to represent an element of this field.
    type Element: Clone
        + Copy
        + PartialEq
        + Debug
        + for<'a> Add<&'a Self::Element, Output = Self::Element>
        + for<'a> Mul<&'a Self::Element, Output = Self::Element>
        + for<'a> Sub<&'a Self::Element, Output = Self::Element>
        + for<'a> Div<&'a Self::Element, Output = Result<Self::Element, DivByZero>>
        + Inv<Output = Result<Self::Element, DivByZero>>
        + Neg<Output = Self::Element>
        + Ord
        + Send
        + for<'a> TryFrom<&'a Self::EncodedElement, Error = Self::DecodeError>
        + 'static;

    /// The type used to represent an encoded version of `Self::Element`.
    type EncodedElement: Clone + Debug + Serde + Send + for<'a> From<&'a Self::Element> + 'static;

    /// The underlying type for elements in this field.
    type Inner: Eq + Hash + Clone + Ord + Copy + Debug;

    /// An error when decoding an element.
    type DecodeError: std::error::Error + Send + Sync + 'static;

    /// Return the multiplicative identity.
    const ONE: Self::Element;

    /// Return the additive identity.
    const ZERO: Self::Element;

    /// Construct an element out of an instance of the inner type.
    fn as_element(inner: Self::Inner) -> Self::Element;

    /// Construct an inner out of an instance of the element type.
    fn as_inner(inner: Self::Element) -> Self::Inner;

    /// Get the first N inner elements in this field.
    fn inner_elements(total: u32) -> Result<Vec<Self::Inner>, TooManyElements>;

    /// Make polynomial in same field.
    fn polynomial(coefs: Vec<Self::Element>) -> Polynomial<Self> {
        Polynomial::new(coefs)
    }

    /// Generates a random number in this field.
    fn gen_random_element<R: CryptoRngCore>(rng: &mut R) -> Self::Element;

    /// Encodes elements.
    fn encode<'a, I>(elements: I) -> Vec<Self::EncodedElement>
    where
        I: IntoIterator<Item = &'a Self::Element>,
    {
        elements.into_iter().map(Self::EncodedElement::from).collect()
    }

    /// Attempts to decode elements.
    fn try_decode<'a, I>(numbers: I) -> Result<Vec<Self::Element>, Self::DecodeError>
    where
        I: IntoIterator<Item = &'a Self::EncodedElement>,
    {
        numbers.into_iter().map(Self::Element::try_from).collect()
    }
}

/// A finite field which is a prime field.
#[derive(Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PrimeField<T>(PhantomData<T>);

impl<T: Modular> Debug for PrimeField<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PrimeField<{}>", T::MODULO)
    }
}

impl<T> Default for PrimeField<T> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<T: Prime> Field for PrimeField<T> {
    type Element = ModularNumber<T>;
    type EncodedElement = EncodedModularNumber;
    type Inner = T::Normal;
    type DecodeError = crate::modular::DecodeError;

    const ZERO: Self::Element = ModularNumber::ZERO;
    const ONE: Self::Element = ModularNumber::ONE;

    fn as_element(inner: Self::Inner) -> Self::Element {
        ModularNumber::new(inner)
    }

    fn as_inner(element: Self::Element) -> Self::Inner {
        element.into_value()
    }

    fn inner_elements(total: u32) -> Result<Vec<Self::Inner>, TooManyElements> {
        let values = (0..total).map(|value| Self::Inner::from(value as u64)).collect();
        Ok(values)
    }

    fn gen_random_element<R: CryptoRngCore>(rng: &mut R) -> Self::Element {
        ModularNumber::gen_random_with_rng(rng)
    }
}

/// A binary extension field that operates in modulo 256.
#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BinaryExtField;

impl Field for BinaryExtField {
    type Element = GF256;
    type EncodedElement = u8;
    type Inner = u8;
    type DecodeError = Infallible;

    const ZERO: Self::Element = GF256::ZERO;
    const ONE: Self::Element = GF256::ONE;

    fn as_element(inner: Self::Inner) -> Self::Element {
        GF256::new(inner)
    }

    fn as_inner(element: Self::Element) -> Self::Inner {
        element.value()
    }

    fn inner_elements(total: u32) -> Result<Vec<Self::Inner>, TooManyElements> {
        let total = u8::try_from(total).map_err(|_| TooManyElements)?;
        let values = (0..total).collect();
        Ok(values)
    }

    fn gen_random_element<R: CryptoRngCore>(rng: &mut R) -> Self::Element {
        GF256::gen_random_with_rng(rng)
    }
}

/// Too many elements were requested from a field.
#[derive(Debug, thiserror::Error)]
#[error("too many elements")]
pub struct TooManyElements;

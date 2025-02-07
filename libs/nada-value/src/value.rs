//! This module defines a `NadaValue`:
//! * `NadaValue` lists all types but also contains a value. This value need to implement the `PrimitiveTypes` trait to
//!   specify the underlying types that should be used.
//!
use enum_as_inner::EnumAsInner;
use indexmap::IndexMap;
use math_lib::modular::{Modular, ModularNumber, Overflow, ToBigUint, TryFromU8Slice};
use nada_type::{HashableIndexMap, NadaType, NadaTypeKind, PrimitiveTypes, TypeError, MAX_RECURSION_DEPTH};
use num_bigint::{BigInt, BigUint, Sign};
use std::{
    fmt,
    fmt::{Display, Formatter},
    ops::{Deref, DerefMut},
};
use types_proc_macros::{EnumNewFunctions, EnumToNadaType, EnumToNadaTypeKind};

/// Represents a Nada value.
#[derive(Eq, PartialEq, Debug, EnumAsInner, EnumNewFunctions, EnumToNadaType, EnumToNadaTypeKind)]
#[cfg_attr(feature = "secret-serde", derive(serde::Serialize, serde::Deserialize))]
pub enum NadaValue<T: PrimitiveTypes> {
    // Primitive types.
    /// Integer.
    Integer(T::Integer),

    /// Unsigned integer.
    UnsignedInteger(T::UnsignedInteger),

    /// Boolean.
    Boolean(T::Boolean),

    /// Secret integer.
    SecretInteger(T::SecretInteger),

    /// Secret unsigned integer.
    SecretUnsignedInteger(T::SecretUnsignedInteger),

    /// Secret boolean.
    SecretBoolean(T::SecretBoolean),

    /// Secret blob.
    SecretBlob(T::SecretBlob),

    /// Shamir share integer.
    ShamirShareInteger(T::ShamirShareInteger),

    /// Shamir share unsigned integer.
    ShamirShareUnsignedInteger(T::ShamirShareUnsignedInteger),

    /// Shamir share boolean.
    ShamirShareBoolean(T::ShamirShareBoolean),

    /// Array: collection of homogeneous values.
    #[to_type_functions(to_type = array_to_type, into_type = array_into_type)]
    #[skip_new_function]
    Array {
        /// Inner type for this array. Used to enforce that all elements of this array have the same type.
        inner_type: NadaType,

        /// Array values.
        values: Vec<Self>,
    },

    /// Tuple: two heterogeneous values.
    #[to_type_functions(to_type = tuple_to_type, into_type = tuple_into_type)]
    #[skip_new_function]
    Tuple {
        /// Left value.
        left: Box<Self>,

        /// Right value.
        right: Box<Self>,
    },

    /// ECDSA private key for the threshold ecdsa signature feature.
    EcdsaPrivateKey(T::EcdsaPrivateKey),

    /// NTuple: any number of heterogeneous values.
    #[to_type_functions(to_type = n_tuple_to_type, into_type = n_tuple_into_type)]
    #[skip_new_function]
    NTuple {
        /// Tuple values.
        values: Vec<Self>,
    },

    /// Public ECDSA message digest.
    EcdsaDigestMessage(T::EcdsaDigestMessage),

    /// Object: key-value hash map.
    #[to_type_functions(to_type = object_to_type, into_type = object_into_type)]
    #[skip_new_function]
    Object {
        /// Key-value types.
        values: IndexMap<String, Self>,
    },

    /// Private ECDSA signature.
    EcdsaSignature(T::EcdsaSignature),
}

impl<T: PrimitiveTypes> Clone for NadaValue<T> {
    fn clone(&self) -> Self {
        match self {
            Self::Integer(value) => Self::Integer(value.clone()),
            Self::UnsignedInteger(value) => Self::UnsignedInteger(value.clone()),
            Self::Boolean(value) => Self::Boolean(value.clone()),
            Self::EcdsaDigestMessage(value) => Self::EcdsaDigestMessage(value.clone()),
            Self::SecretInteger(value) => Self::SecretInteger(value.clone()),
            Self::SecretUnsignedInteger(value) => Self::SecretUnsignedInteger(value.clone()),
            Self::SecretBoolean(value) => Self::SecretBoolean(value.clone()),
            Self::SecretBlob(value) => Self::SecretBlob(value.clone()),
            Self::ShamirShareInteger(value) => Self::ShamirShareInteger(value.clone()),
            Self::ShamirShareUnsignedInteger(value) => Self::ShamirShareUnsignedInteger(value.clone()),
            Self::ShamirShareBoolean(value) => Self::ShamirShareBoolean(value.clone()),
            Self::Array { inner_type, values } => {
                Self::Array { inner_type: inner_type.clone(), values: values.clone() }
            }
            Self::Tuple { left, right } => Self::Tuple { left: left.clone(), right: right.clone() },
            Self::EcdsaPrivateKey(value) => Self::EcdsaPrivateKey(value.clone()),
            Self::EcdsaSignature(value) => Self::EcdsaSignature(value.clone()),
            Self::NTuple { values } => Self::NTuple { values: values.clone() },
            Self::Object { values } => Self::Object { values: values.clone() },
        }
    }
}

impl<T: PrimitiveTypes> NadaValue<T> {
    /// Returns a new array.
    /// Values have to be homogeneous (same NadaValue variant).
    /// new_array_non_empty can be used instead if you know there will always be at least one element in the array.
    pub fn new_array(inner_type: NadaType, values: Vec<Self>) -> Result<Self, TypeError> {
        if values.iter().any(|value: &NadaValue<T>| value.to_type() != inner_type) {
            return Err(TypeError::HomogeneousVecOnly);
        }

        let value = NadaValue::Array { inner_type, values };

        if value.recursion_depth() > MAX_RECURSION_DEPTH {
            return Err(TypeError::MaxRecursionDepthExceeded);
        }

        Ok(value)
    }

    /// Returns a new array from a non-empty array.
    /// The inner type is determined based on the values vector.
    /// Values have to be homogeneous (same NadaValue variant).
    pub fn new_array_non_empty(values: Vec<Self>) -> Result<Self, TypeError> {
        let Some(first) = values.first() else {
            return Err(TypeError::NonEmptyVecOnly);
        };
        let inner_type = first.to_type();

        if !values.iter().all(|value| value.to_type() == inner_type) {
            return Err(TypeError::HomogeneousVecOnly);
        }

        let value = NadaValue::Array { inner_type, values };

        if value.recursion_depth() > MAX_RECURSION_DEPTH {
            return Err(TypeError::MaxRecursionDepthExceeded);
        }

        Ok(value)
    }

    /// Returns a new tuple.
    pub fn new_tuple(left: Self, right: Self) -> Result<Self, TypeError> {
        let value = NadaValue::Tuple { left: Box::new(left), right: Box::new(right) };

        if value.recursion_depth() > MAX_RECURSION_DEPTH {
            return Err(TypeError::MaxRecursionDepthExceeded);
        }

        Ok(value)
    }

    /// Returns a new ntuple.
    pub fn new_n_tuple(values: Vec<Self>) -> Result<Self, TypeError> {
        let value = NadaValue::NTuple { values };

        if value.recursion_depth() > MAX_RECURSION_DEPTH {
            return Err(TypeError::MaxRecursionDepthExceeded);
        }

        Ok(value)
    }

    /// Returns a new object.
    pub fn new_object(values: IndexMap<String, Self>) -> Result<Self, TypeError> {
        let value = NadaValue::Object { values };

        if value.recursion_depth() > MAX_RECURSION_DEPTH {
            return Err(TypeError::MaxRecursionDepthExceeded);
        }

        Ok(value)
    }

    /// Returns an iterator over this NadaValue.
    /// This iterator goes over any compound types.
    pub fn iter(&self) -> NadaValueIter<T> {
        NadaValueIter { stack: vec![self] }
    }

    /// Returns a mutable iterator over this NadaValue.
    /// This iterator goes over any compound types.
    pub fn iter_mut(&mut self) -> NadaValueIterMut<T> {
        NadaValueIterMut { stack: vec![self] }
    }

    /// Returns an "into" iterator over this NadaValue.
    /// This iterator goes over any compound types.
    #[allow(clippy::should_implement_trait)] // I believe that name makes sense here, especially because we already have
    // iter and iter_mut.
    pub fn into_iter(self) -> NadaValueIntoIter<T> {
        NadaValueIntoIter { stack: vec![self] }
    }

    /// Returns the recursion depth.
    fn recursion_depth(&self) -> usize {
        let mut stack = vec![(self, 1)];
        let mut max_depth = 0;

        while let Some((value, depth)) = stack.pop() {
            use NadaValue::*;

            max_depth = max_depth.max(depth);

            match value {
                Integer(_)
                | UnsignedInteger(_)
                | Boolean(_)
                | EcdsaDigestMessage(_)
                | SecretInteger(_)
                | SecretUnsignedInteger(_)
                | SecretBoolean(_)
                | SecretBlob(_)
                | ShamirShareInteger(_)
                | ShamirShareUnsignedInteger(_)
                | ShamirShareBoolean(_)
                | EcdsaPrivateKey(_)
                | EcdsaSignature(_) => {}
                Array { values, .. } | NTuple { values } => {
                    for value in values {
                        stack.push((value, depth + 1));
                    }
                }
                Tuple { left, right } => {
                    stack.push((left, depth + 1));
                    stack.push((right, depth + 1));
                }
                Object { values } => {
                    for value in values.values() {
                        stack.push((value, depth + 1));
                    }
                }
            }
        }

        max_depth
    }

    /// Returns a list with the value and every value that it contains.
    /// For instance, for Array { values: [ Integer(1), Integer(2), Integer(3)] } this returns
    /// [
    ///   Array { values: vec![] },
    ///   Integer(1),
    ///   Integer(2),
    ///   Integer(3),
    /// ]
    pub fn flatten_inner_values(self) -> Vec<Self> {
        let mut values = vec![self];
        let mut flattened_values = vec![];
        while let Some(value) = values.pop() {
            use NadaValue::*;

            match value {
                Integer(_)
                | UnsignedInteger(_)
                | Boolean(_)
                | EcdsaDigestMessage(_)
                | SecretInteger(_)
                | SecretUnsignedInteger(_)
                | SecretBoolean(_)
                | SecretBlob(_)
                | ShamirShareInteger(_)
                | ShamirShareUnsignedInteger(_)
                | ShamirShareBoolean(_)
                | EcdsaPrivateKey(_)
                | EcdsaSignature(_) => flattened_values.push(value),
                Array { values: inner_values, inner_type } => {
                    values.extend(inner_values.clone());
                    flattened_values.push(Array { values: inner_values, inner_type });
                }
                Tuple { left, right } => {
                    values.push(*left);
                    values.push(*right);
                }
                NTuple { values: inner_values } => {
                    values.extend(inner_values.clone());
                    flattened_values.push(NTuple { values: inner_values });
                }
                Object { values: inner_values } => {
                    values.extend(inner_values.values().cloned());
                    flattened_values.push(Object { values: inner_values });
                }
            }
        }
        flattened_values
    }
}

/// Iterator over a NadaValue.
/// This iterator goes over any compound types.
pub struct NadaValueIter<'a, T: PrimitiveTypes> {
    stack: Vec<&'a NadaValue<T>>,
}

impl<'a, T: PrimitiveTypes> Iterator for NadaValueIter<'a, T> {
    type Item = &'a NadaValue<T>;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(value) = self.stack.pop() {
            use NadaValue::*;

            match value {
                Integer(_)
                | UnsignedInteger(_)
                | Boolean(_)
                | EcdsaDigestMessage(_)
                | SecretInteger(_)
                | SecretUnsignedInteger(_)
                | SecretBoolean(_)
                | SecretBlob(_)
                | ShamirShareInteger(_)
                | ShamirShareUnsignedInteger(_)
                | ShamirShareBoolean(_)
                | EcdsaPrivateKey(_)
                | EcdsaSignature(_) => return Some(value),
                Array { values, .. } | NTuple { values } => {
                    for value in values.iter().rev() {
                        self.stack.push(value);
                    }
                }
                Tuple { left, right } => {
                    self.stack.push(right);
                    self.stack.push(left);
                }
                Object { values } => {
                    for value in values.values().rev() {
                        self.stack.push(value);
                    }
                }
            }
        }
        None
    }
}

/// Mutable iterator over a NadaValue.
/// This iterator goes over any compound types.
pub struct NadaValueIterMut<'a, T: PrimitiveTypes> {
    stack: Vec<&'a mut NadaValue<T>>,
}

impl<'a, T: PrimitiveTypes> Iterator for NadaValueIterMut<'a, T> {
    type Item = &'a mut NadaValue<T>;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(value) = self.stack.pop() {
            use NadaValue::*;

            match value {
                Integer(_)
                | UnsignedInteger(_)
                | Boolean(_)
                | EcdsaDigestMessage(_)
                | SecretInteger(_)
                | SecretUnsignedInteger(_)
                | SecretBoolean(_)
                | SecretBlob(_)
                | ShamirShareInteger(_)
                | ShamirShareUnsignedInteger(_)
                | ShamirShareBoolean(_)
                | EcdsaPrivateKey(_)
                | EcdsaSignature(_) => return Some(value),
                Array { values, .. } | NTuple { values } => {
                    for value in values.iter_mut().rev() {
                        self.stack.push(value);
                    }
                }
                Tuple { left, right } => {
                    self.stack.push(right);
                    self.stack.push(left);
                }
                Object { values } => {
                    for value in values.values_mut().rev() {
                        self.stack.push(value);
                    }
                }
            }
        }
        None
    }
}

/// Returns an "into" iterator over this NadaValue.
/// This iterator goes over any compound types.
pub struct NadaValueIntoIter<T: PrimitiveTypes> {
    stack: Vec<NadaValue<T>>,
}

impl<T: PrimitiveTypes> Iterator for NadaValueIntoIter<T> {
    type Item = NadaValue<T>;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(value) = self.stack.pop() {
            use NadaValue::*;

            match value {
                Integer(_)
                | UnsignedInteger(_)
                | Boolean(_)
                | EcdsaDigestMessage(_)
                | SecretInteger(_)
                | SecretUnsignedInteger(_)
                | SecretBoolean(_)
                | SecretBlob(_)
                | ShamirShareInteger(_)
                | ShamirShareUnsignedInteger(_)
                | ShamirShareBoolean(_)
                | EcdsaPrivateKey(_)
                | EcdsaSignature(_) => return Some(value),
                Array { values, .. } => {
                    for value in values.into_iter().rev() {
                        self.stack.push(value);
                    }
                }
                Tuple { left, right } => {
                    self.stack.push(*right);
                    self.stack.push(*left);
                }
                NTuple { values } => {
                    for value in values.into_iter().rev() {
                        self.stack.push(value);
                    }
                }
                Object { values } => {
                    for value in values.into_values().rev() {
                        self.stack.push(value);
                    }
                }
            }
        }
        None
    }
}

fn array_to_type<T: PrimitiveTypes>(inner_type: &NadaType, values: &[NadaValue<T>]) -> NadaType {
    NadaType::Array { inner_type: Box::new(inner_type.clone()), size: values.len() }
}

fn array_into_type<T: PrimitiveTypes>(inner_type: NadaType, values: Vec<NadaValue<T>>) -> NadaType {
    NadaType::Array { inner_type: Box::new(inner_type), size: values.len() }
}

#[allow(clippy::borrowed_box)]
fn tuple_to_type<T: PrimitiveTypes>(left: &Box<NadaValue<T>>, right: &Box<NadaValue<T>>) -> NadaType {
    NadaType::Tuple { left_type: Box::new(left.to_type()), right_type: Box::new(right.to_type()) }
}

#[allow(clippy::boxed_local)]
fn tuple_into_type<T: PrimitiveTypes>(left: Box<NadaValue<T>>, right: Box<NadaValue<T>>) -> NadaType {
    NadaType::Tuple { left_type: Box::new(left.into_type()), right_type: Box::new(right.into_type()) }
}

fn n_tuple_to_type<T: PrimitiveTypes>(values: &[NadaValue<T>]) -> NadaType {
    NadaType::NTuple { types: values.iter().map(|value| value.to_type()).collect() }
}

fn n_tuple_into_type<T: PrimitiveTypes>(values: Vec<NadaValue<T>>) -> NadaType {
    NadaType::NTuple { types: values.into_iter().map(|value| value.into_type()).collect() }
}

fn object_to_type<T: PrimitiveTypes>(values: &IndexMap<String, NadaValue<T>>) -> NadaType {
    NadaType::Object {
        types: HashableIndexMap(values.iter().map(|(name, value)| (name.clone(), value.to_type())).collect()),
    }
}

fn object_into_type<T: PrimitiveTypes>(values: IndexMap<String, NadaValue<T>>) -> NadaType {
    NadaType::Object {
        types: HashableIndexMap(values.into_iter().map(|(name, value)| (name, value.into_type())).collect()),
    }
}

/// A signed integer that can be serialized as a string.
/// Using this instead of BigInt directly offers a few advantages:
/// * BigInt serializes into an array of integers, but we want to serialize to a string instead.
/// * We don't have to use serde_as and "DisplayFromStr" as an attribute every time we use a BigInt in a struct.
///     This also forces you to create a newtype over BigInt just for that, and have a "value" indirection in data input
///     files.
/// * It offers an abstraction layer for future changes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NadaInt(BigInt);

impl Display for NadaInt {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl Deref for NadaInt {
    type Target = BigInt;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for NadaInt {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<NadaInt> for BigInt {
    fn from(value: NadaInt) -> Self {
        value.0
    }
}

impl From<BigInt> for NadaInt {
    fn from(value: BigInt) -> Self {
        Self(value)
    }
}

impl From<i64> for NadaInt {
    fn from(value: i64) -> Self {
        Self(BigInt::from(value))
    }
}

impl From<i32> for NadaInt {
    fn from(value: i32) -> Self {
        Self(BigInt::from(value))
    }
}

impl From<u64> for NadaInt {
    fn from(value: u64) -> Self {
        Self(BigInt::from(value))
    }
}

impl From<u32> for NadaInt {
    fn from(value: u32) -> Self {
        Self(BigInt::from(value))
    }
}

impl NadaInt {
    /// Consumes this NadaInt and returns the BigInt within it.
    pub fn into_inner(self) -> BigInt {
        self.0
    }
}

/// An unsigned integer that can be serialized as a string.
/// Using this instead of BigUint directly offers a few advantages:
/// * BigUint serializes into an array of integers, but we want to serialize to a string instead.
/// * We don't have to use serde_as and "DisplayFromStr" as an attribute every time we use a BigUint in a struct.
///     This also forces you to create a newtype over BigUint just for that, and have a "value" indirection in data
///     input files.
/// * It offers an abstraction layer for future changes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NadaUint(BigUint);

impl Display for NadaUint {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl Deref for NadaUint {
    type Target = BigUint;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for NadaUint {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<NadaUint> for BigUint {
    fn from(value: NadaUint) -> Self {
        value.0
    }
}

impl From<BigUint> for NadaUint {
    fn from(value: BigUint) -> Self {
        Self(value)
    }
}

impl From<bool> for NadaUint {
    fn from(value: bool) -> Self {
        Self(BigUint::from(value as u8))
    }
}

impl From<u64> for NadaUint {
    fn from(value: u64) -> Self {
        Self(BigUint::from(value))
    }
}

impl From<u32> for NadaUint {
    fn from(value: u32) -> Self {
        Self(BigUint::from(value))
    }
}

impl NadaUint {
    /// Consumes this NadaUint and returns the BigUint within it.
    pub fn into_inner(self) -> BigUint {
        self.0
    }
}

impl<T: Modular> From<&ModularNumber<T>> for NadaUint {
    fn from(value: &ModularNumber<T>) -> Self {
        value.into_value().to_biguint().into()
    }
}

impl<T: Modular> From<&ModularNumber<T>> for NadaInt {
    fn from(value: &ModularNumber<T>) -> Self {
        BigInt::from(value).into()
    }
}

impl<T: Modular> TryFrom<&NadaUint> for ModularNumber<T> {
    type Error = Overflow;

    fn try_from(value: &NadaUint) -> Result<Self, Self::Error> {
        let value = value.to_bytes_le();
        let value = T::Normal::try_from_u8_slice(&value)?;
        if value >= T::MODULO {
            return Err(Overflow);
        }
        Ok(ModularNumber::new(value))
    }
}

impl<T: Modular> TryFrom<&NadaInt> for ModularNumber<T> {
    type Error = Overflow;

    fn try_from(value: &NadaInt) -> Result<Self, Self::Error> {
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

#[cfg(feature = "secret-serde")]
mod serde_impl {
    use super::{NadaInt, NadaUint};
    use num_bigint::{BigInt, BigUint};
    use serde::{de, de::Visitor, Deserialize, Deserializer, Serialize, Serializer};
    use std::{fmt, fmt::Formatter, str::FromStr};

    impl Serialize for NadaInt {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            serializer.serialize_str(&self.0.to_string())
        }
    }

    impl<'de> Deserialize<'de> for NadaInt {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            deserializer.deserialize_str(NadaIntVisitor)
        }
    }

    struct NadaIntVisitor;

    impl<'de> Visitor<'de> for NadaIntVisitor {
        type Value = NadaInt;

        fn expecting(&self, formatter: &mut Formatter) -> fmt::Result {
            formatter.write_str("a string representing a BigInt")
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            BigInt::from_str(value).map(NadaInt).map_err(de::Error::custom)
        }
    }

    impl Serialize for NadaUint {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            serializer.serialize_str(&self.0.to_string())
        }
    }

    impl<'de> Deserialize<'de> for NadaUint {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            deserializer.deserialize_str(NadaUintVisitor)
        }
    }

    struct NadaUintVisitor;

    impl<'de> Visitor<'de> for NadaUintVisitor {
        type Value = NadaUint;

        fn expecting(&self, formatter: &mut Formatter) -> fmt::Result {
            formatter.write_str("a string representing a BigInt")
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            BigUint::from_str(value).map(NadaUint).map_err(de::Error::custom)
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{clear::Clear, NadaValue};
    use anyhow::Result;
    use indexmap::IndexMap;
    use nada_type::{NadaType, NadaTypeKind, PrimitiveTypes, TypeError, MAX_RECURSION_DEPTH};

    #[derive(Clone, Debug, PartialEq)]
    struct TestStruct;

    impl PrimitiveTypes for TestStruct {
        type Integer = i32;
        type UnsignedInteger = u32;
        type Boolean = bool;
        type SecretInteger = i32;
        type SecretUnsignedInteger = i32;
        type SecretBoolean = bool;
        type SecretBlob = Vec<u8>;
        type ShamirShareInteger = i32;
        type ShamirShareUnsignedInteger = i32;
        type ShamirShareBoolean = bool;
        type EcdsaPrivateKey = [u8; 32];
        type EcdsaDigestMessage = [u8; 32];
        type EcdsaSignature = i32;
    }

    type MyTestType = NadaValue<TestStruct>;

    #[test]
    fn basic_test() {
        let mut my_value = MyTestType::new_integer(42);
        assert!(my_value.to_type().is_primitive());
        assert_eq!(my_value.to_type(), NadaType::Integer);
        assert_eq!(my_value.to_type_kind(), NadaTypeKind::Integer);
        assert_eq!(my_value.as_integer(), Some(&42));
        *my_value.as_integer_mut().unwrap() = 43;
        assert_eq!(my_value.as_integer(), Some(&43));
        assert!(my_value.is_integer());

        let my_value = MyTestType::new_boolean(true);
        assert_ne!(my_value.to_type(), NadaType::Integer);
        assert_ne!(my_value.to_type_kind(), NadaTypeKind::Integer);
        assert_eq!(my_value.as_integer(), None);
        assert!(!my_value.is_integer());

        let my_value = MyTestType::new_tuple(MyTestType::new_integer(42), MyTestType::new_integer(43)).unwrap();
        assert!(!my_value.to_type().is_primitive());
        assert!(my_value.is_tuple());
        if let Some((left, right)) = my_value.as_tuple() {
            assert_eq!(left.as_integer(), Some(&42));
            assert_eq!(right.as_integer(), Some(&43));
        }

        let my_value = MyTestType::new_n_tuple(vec![MyTestType::new_integer(42), MyTestType::new_integer(43)]).unwrap();
        assert!(!my_value.to_type().is_primitive());
        assert!(my_value.is_n_tuple());
        if let Some(elements) = my_value.as_n_tuple() {
            assert_eq!(elements[0].as_integer(), Some(&42));
            assert_eq!(elements[1].as_integer(), Some(&43));
        }

        let my_value = MyTestType::new_object(IndexMap::from([
            ("a".to_string(), MyTestType::new_integer(42)),
            ("b".to_string(), MyTestType::new_integer(43)),
        ]))
        .unwrap();
        assert!(!my_value.to_type().is_primitive());
        assert!(my_value.is_object());
        if let Some(elements) = my_value.as_object() {
            assert_eq!(elements["a"].as_integer(), Some(&42));
            assert_eq!(elements["b"].as_integer(), Some(&43));
        }
    }

    #[test]
    fn test_iter() {
        let value = MyTestType::new_integer(42);
        assert_eq!(value.iter().collect::<Vec<_>>(), vec![&MyTestType::new_integer(42)]);

        let mut value = MyTestType::new_tuple(
            MyTestType::new_array_non_empty(vec![MyTestType::new_integer(42), MyTestType::new_integer(43)]).unwrap(),
            MyTestType::new_integer(44),
        )
        .unwrap();
        assert_eq!(value.iter().map(|value| *value.as_integer().unwrap()).collect::<Vec<_>>(), vec![42, 43, 44]);
        assert_eq!(
            value.iter_mut().map(|value| *value.as_integer_mut().unwrap() * 2).collect::<Vec<_>>(),
            vec![84, 86, 88]
        );
        assert_eq!(value.into_iter().map(|value| *value.as_integer().unwrap()).collect::<Vec<_>>(), vec![42, 43, 44]);

        let mut value = MyTestType::new_n_tuple(vec![
            MyTestType::new_array_non_empty(vec![MyTestType::new_integer(42), MyTestType::new_integer(43)]).unwrap(),
            MyTestType::new_integer(44),
        ])
        .unwrap();
        assert_eq!(value.iter().map(|value| *value.as_integer().unwrap()).collect::<Vec<_>>(), vec![42, 43, 44]);
        assert_eq!(
            value.iter_mut().map(|value| *value.as_integer_mut().unwrap() * 2).collect::<Vec<_>>(),
            vec![84, 86, 88]
        );
        assert_eq!(value.into_iter().map(|value| *value.as_integer().unwrap()).collect::<Vec<_>>(), vec![42, 43, 44]);

        let mut value = MyTestType::new_object(IndexMap::from([
            (
                "a".to_string(),
                MyTestType::new_array_non_empty(vec![MyTestType::new_integer(42), MyTestType::new_integer(43)])
                    .unwrap(),
            ),
            ("b".to_string(), MyTestType::new_integer(44)),
        ]))
        .unwrap();
        assert_eq!(value.iter().map(|value| *value.as_integer().unwrap()).collect::<Vec<_>>(), vec![42, 43, 44]);
        assert_eq!(
            value.iter_mut().map(|value| *value.as_integer_mut().unwrap() * 2).collect::<Vec<_>>(),
            vec![84, 86, 88]
        );
        assert_eq!(value.into_iter().map(|value| *value.as_integer().unwrap()).collect::<Vec<_>>(), vec![42, 43, 44]);
    }

    #[test]
    fn test_depth() {
        let value = MyTestType::new_integer(42);
        assert_eq!(value.recursion_depth(), 1);

        let value = MyTestType::new_tuple(MyTestType::new_integer(42), MyTestType::new_integer(43)).unwrap();
        assert_eq!(value.recursion_depth(), 2);

        let value = MyTestType::new_tuple(
            MyTestType::new_integer(42),
            MyTestType::new_array_non_empty(vec![MyTestType::new_integer(43)]).unwrap(),
        )
        .unwrap();
        assert_eq!(value.recursion_depth(), 3);

        let value = MyTestType::new_n_tuple(vec![MyTestType::new_integer(42), MyTestType::new_integer(43)]).unwrap();
        assert_eq!(value.recursion_depth(), 2);

        let value = MyTestType::new_n_tuple(vec![
            MyTestType::new_integer(42),
            MyTestType::new_array_non_empty(vec![MyTestType::new_integer(43)]).unwrap(),
        ])
        .unwrap();
        assert_eq!(value.recursion_depth(), 3);

        let value = MyTestType::new_object(IndexMap::from([
            ("a".to_string(), MyTestType::new_integer(42)),
            ("b".to_string(), MyTestType::new_array_non_empty(vec![MyTestType::new_integer(43)]).unwrap()),
        ]))
        .unwrap();
        assert_eq!(value.recursion_depth(), 3);

        let value = MyTestType::new_array_non_empty(vec![
            MyTestType::new_array_non_empty(vec![MyTestType::new_integer(42)]).unwrap(),
            MyTestType::new_array_non_empty(vec![MyTestType::new_integer(43)]).unwrap(),
        ])
        .unwrap();
        assert_eq!(value.recursion_depth(), 3);
    }

    #[test]
    fn test_max_recursion_depth() -> Result<()> {
        let mut value = MyTestType::new_array_non_empty(vec![MyTestType::new_integer(42)])?;

        for _ in 0..MAX_RECURSION_DEPTH {
            value = match MyTestType::new_array_non_empty(vec![value]) {
                Ok(value) => value,
                Err(TypeError::MaxRecursionDepthExceeded) => return Ok(()),
                Err(err) => return Err(err.into()),
            };
        }

        Ok(())
    }

    #[test]
    fn test_display() -> Result<()> {
        let value = NadaValue::<Clear>::new_integer(42);

        assert_eq!(value.to_string(), "Integer(42)");
        assert_eq!(value.to_type().to_string(), "Integer");
        assert_eq!(value.to_type_kind().to_string(), "Integer");

        let value = NadaValue::<Clear>::new_array_non_empty(vec![
            NadaValue::new_integer(1),
            NadaValue::new_integer(2),
            NadaValue::new_integer(3),
        ])?;

        assert_eq!(value.to_string(), "Array(Integer(1), Integer(2), Integer(3))");
        assert_eq!(value.to_type().to_string(), "Array [Integer:3]");
        assert_eq!(value.to_type_kind().to_string(), "Array");

        Ok(())
    }
}

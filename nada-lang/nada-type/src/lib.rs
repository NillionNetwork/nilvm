//! This module defines all the types using in Nada:
//! * `NadaType` lists all types. Compound types like array and tuple have additional properties.
//! * `NadaTypeKind` lists all type as unit variants. Compound types are also represented as unit variants.
//!

#![feature(never_type)]

use enum_as_inner::EnumAsInner;
pub use indexmap::IndexMap;
use std::{
    fmt,
    fmt::{Display, Formatter},
    hash::{Hash, Hasher},
    ops::{Deref, DerefMut},
};
use strum_macros::{EnumDiscriminants, EnumIter, IntoStaticStr};
use thiserror::Error;
use types_proc_macros::{EnumIsPrimitive, EnumNewFunctions, EnumPrimitiveToTrait, EnumToNadaTypeKind};

/// Maximum recursion depth.
/// This is set to reduce the risk of hitting a stack overflow.
pub const MAX_RECURSION_DEPTH: usize = 100;

/// A hashable version of IndexMap.
#[derive(Clone, Eq, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HashableIndexMap<K: Hash + Eq, V: Hash>(pub IndexMap<K, V>);

impl<K: Hash + Eq, V: Hash> Hash for HashableIndexMap<K, V> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for (key, value) in self.iter() {
            key.hash(state);
            value.hash(state);
        }
    }
}

impl<K: Hash + Eq, V: Hash> From<IndexMap<K, V>> for HashableIndexMap<K, V> {
    fn from(value: IndexMap<K, V>) -> Self {
        HashableIndexMap(value)
    }
}

impl<K: Hash + Eq, V: Hash> From<HashableIndexMap<K, V>> for IndexMap<K, V> {
    fn from(value: HashableIndexMap<K, V>) -> Self {
        value.0
    }
}

impl<K: Hash + Eq, V: Hash> Deref for HashableIndexMap<K, V> {
    type Target = IndexMap<K, V>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<K: Hash + Eq, V: Hash> DerefMut for HashableIndexMap<K, V> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// The shape of the value during an action execution. The shape of an value depends on the execution
/// stage we are executing. That means that an input can change between different shapes during an
/// execution. For instance, during the compute action the life cycle of a secret is:
/// 1.- A user provide the Secret.
/// 2.- The dealer calculates the shares that are sent to the nodes
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Shape {
    /// Public variable
    PublicVariable,
    /// Secret
    Secret,
    /// Shamir share
    ShamirShare,
}

/// Indicates the type will be used for the user to provide/consume it.
#[derive(Copy, Clone, Eq, PartialEq)]
pub enum NadaPrimitiveType {
    /// The value is an integer
    Integer,
    /// The value is an unsigned integer
    UnsignedInteger,
    /// The value is a boolean
    Boolean,
    /// The value is a blob
    Blob,
    /// The value is a private ecdsa key
    EcdsaPrivateKey,
    /// The value is an ecdsa digest message
    EcdsaDigestMessage,
    /// The value is an ecdsa signature
    EcdsaSignature,
    /// The value is a public ecdsa key
    EcdsaPublicKey,
    /// The value is a store id
    StoreId,
}

/// This struct is used to extract the meta data for a nada type
#[derive(Clone, Eq, PartialEq)]
pub enum NadaTypeMetadata {
    /// Non container type
    PrimitiveType {
        /// Indicates the shape of the value
        shape: Shape,
        /// Indicates the primitive type of the falue
        nada_primitive_type: NadaPrimitiveType,
    },
    /// Array container
    Array {
        /// The capacity of the container
        size: usize,
        /// The type of all elements in the array
        inner: Box<Self>,
    },
    /// Tuple container
    Tuple {
        /// Type for the left element in the tuple
        left: Box<Self>,
        /// Type for the right element in the tuple
        right: Box<Self>,
    },
    /// NTuple container
    NTuple {
        /// Types for all the elements in the the tuple
        types: Vec<Self>,
    },
    /// Object container
    Object {
        /// Types for all the elements in the the object
        types: IndexMap<String, Self>,
    },
}

impl NadaTypeMetadata {
    /// Changes the shape for a new one
    pub fn with_shape(self, new_shape: Shape) -> Self {
        self.with_shape_if(new_shape, |_| true)
    }

    /// Changes the shape if the condition is true
    pub fn with_shape_if(mut self, new_shape: Shape, condition: fn(&Self) -> bool) -> Self {
        let mut inner_types = vec![&mut self];
        while let Some(ty) = inner_types.pop() {
            match ty {
                NadaTypeMetadata::PrimitiveType { .. } if !condition(ty) => {
                    // Do nothing
                }
                NadaTypeMetadata::PrimitiveType { shape, .. } => *shape = new_shape,
                NadaTypeMetadata::Array { inner, .. } => inner_types.push(inner),
                NadaTypeMetadata::Tuple { left, right } => {
                    inner_types.push(left);
                    inner_types.push(right);
                }
                NadaTypeMetadata::NTuple { types } => {
                    for inner_type in types {
                        inner_types.push(inner_type);
                    }
                }
                NadaTypeMetadata::Object { types } => {
                    for inner_type in types.values_mut() {
                        inner_types.push(inner_type);
                    }
                }
            }
        }
        self
    }

    /// Returns if the type's value is private.
    /// We consider private all values to which access is retricted. Thus, this method considers
    /// that a value is private when it is not public or a compound type. For instance:
    /// - Secret, ShamirShare and ShamirParticle are considered privates.
    /// - PublicVariables aren't private, because the access is not restricted.
    /// - Arrays and tuples aren't private, because they are containers and they don't have this
    ///   attribute. Elements contained in compound types also follow these rules.
    pub fn is_private(&self) -> Option<bool> {
        match &self {
            NadaTypeMetadata::PrimitiveType { shape: Shape::PublicVariable, .. } => Some(false),
            NadaTypeMetadata::PrimitiveType { .. } => Some(true),
            NadaTypeMetadata::Array { .. }
            | NadaTypeMetadata::Tuple { .. }
            | NadaTypeMetadata::NTuple { .. }
            | NadaTypeMetadata::Object { .. } => None,
        }
    }

    /// Returns the type's shape if the type has this attribute
    pub fn shape(&self) -> Option<Shape> {
        match &self {
            NadaTypeMetadata::PrimitiveType { shape, .. } => Some(*shape),
            NadaTypeMetadata::Array { .. }
            | NadaTypeMetadata::Tuple { .. }
            | NadaTypeMetadata::NTuple { .. }
            | NadaTypeMetadata::Object { .. } => None,
        }
    }

    /// Returns the type's nada_primitive_type if the type has this attribute
    pub fn nada_primitive_type(&self) -> Option<NadaPrimitiveType> {
        match &self {
            NadaTypeMetadata::PrimitiveType { nada_primitive_type, .. } => Some(*nada_primitive_type),
            NadaTypeMetadata::Array { .. }
            | NadaTypeMetadata::Tuple { .. }
            | NadaTypeMetadata::NTuple { .. }
            | NadaTypeMetadata::Object { .. } => None,
        }
    }

    /// Returns true if the type is numeric
    pub fn is_numeric(&self) -> bool {
        let Some(primitive_type) = self.nada_primitive_type() else {
            return false;
        };
        match primitive_type {
            NadaPrimitiveType::Integer | NadaPrimitiveType::UnsignedInteger => true,
            NadaPrimitiveType::Boolean
            | NadaPrimitiveType::Blob
            | NadaPrimitiveType::EcdsaPrivateKey
            | NadaPrimitiveType::EcdsaDigestMessage
            | NadaPrimitiveType::EcdsaSignature
            | NadaPrimitiveType::EcdsaPublicKey
            | NadaPrimitiveType::StoreId => false,
        }
    }
}

impl From<&NadaType> for NadaTypeMetadata {
    fn from(value: &NadaType) -> Self {
        match value {
            NadaType::Integer => NadaTypeMetadata::PrimitiveType {
                shape: Shape::PublicVariable,
                nada_primitive_type: NadaPrimitiveType::Integer,
            },
            NadaType::UnsignedInteger => NadaTypeMetadata::PrimitiveType {
                shape: Shape::PublicVariable,
                nada_primitive_type: NadaPrimitiveType::UnsignedInteger,
            },
            NadaType::Boolean => NadaTypeMetadata::PrimitiveType {
                shape: Shape::PublicVariable,
                nada_primitive_type: NadaPrimitiveType::Boolean,
            },
            NadaType::EcdsaDigestMessage => NadaTypeMetadata::PrimitiveType {
                shape: Shape::PublicVariable,
                nada_primitive_type: NadaPrimitiveType::EcdsaDigestMessage,
            },
            NadaType::SecretInteger => NadaTypeMetadata::PrimitiveType {
                shape: Shape::Secret,
                nada_primitive_type: NadaPrimitiveType::Integer,
            },
            NadaType::SecretUnsignedInteger => NadaTypeMetadata::PrimitiveType {
                shape: Shape::Secret,
                nada_primitive_type: NadaPrimitiveType::UnsignedInteger,
            },

            NadaType::SecretBoolean => NadaTypeMetadata::PrimitiveType {
                shape: Shape::Secret,
                nada_primitive_type: NadaPrimitiveType::Boolean,
            },
            NadaType::SecretBlob => {
                NadaTypeMetadata::PrimitiveType { shape: Shape::Secret, nada_primitive_type: NadaPrimitiveType::Blob }
            }
            NadaType::ShamirShareInteger => NadaTypeMetadata::PrimitiveType {
                shape: Shape::ShamirShare,
                nada_primitive_type: NadaPrimitiveType::Integer,
            },
            NadaType::ShamirShareUnsignedInteger => NadaTypeMetadata::PrimitiveType {
                shape: Shape::ShamirShare,
                nada_primitive_type: NadaPrimitiveType::UnsignedInteger,
            },
            NadaType::ShamirShareBoolean => NadaTypeMetadata::PrimitiveType {
                shape: Shape::ShamirShare,
                nada_primitive_type: NadaPrimitiveType::Boolean,
            },

            NadaType::Array { size, inner_type } => {
                let inner_type = inner_type.as_ref();
                NadaTypeMetadata::Array { size: *size, inner: Box::new(inner_type.into()) }
            }
            NadaType::Tuple { left_type, right_type } => {
                let left = left_type.as_ref();
                let right = right_type.as_ref();
                NadaTypeMetadata::Tuple { left: Box::new(left.into()), right: Box::new(right.into()) }
            }
            NadaType::EcdsaPrivateKey => NadaTypeMetadata::PrimitiveType {
                shape: Shape::Secret,
                nada_primitive_type: NadaPrimitiveType::EcdsaPrivateKey,
            },
            NadaType::EcdsaSignature => NadaTypeMetadata::PrimitiveType {
                shape: Shape::Secret,
                nada_primitive_type: NadaPrimitiveType::EcdsaSignature,
            },
            NadaType::EcdsaPublicKey => NadaTypeMetadata::PrimitiveType {
                shape: Shape::PublicVariable,
                nada_primitive_type: NadaPrimitiveType::EcdsaPublicKey,
            },
            NadaType::StoreId => NadaTypeMetadata::PrimitiveType {
                shape: Shape::PublicVariable,
                nada_primitive_type: NadaPrimitiveType::StoreId,
            },
            NadaType::NTuple { types } => {
                NadaTypeMetadata::NTuple { types: types.iter().map(|inner_type| inner_type.into()).collect() }
            }
            NadaType::Object { types } => NadaTypeMetadata::Object {
                types: types.iter().map(|(name, inner_type)| (name.clone(), inner_type.into())).collect(),
            },
        }
    }
}

/// Nada types.
#[derive(
    Clone,
    Eq,
    PartialEq,
    Debug,
    Hash,
    EnumDiscriminants,
    EnumAsInner,
    EnumPrimitiveToTrait,
    EnumIsPrimitive,
    EnumNewFunctions,
    EnumToNadaTypeKind
)]
#[strum_discriminants(name(NadaTypeKind), derive(Hash, IntoStaticStr, EnumIter, EnumAsInner, EnumNewFunctions))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum NadaType {
    // Primitive types.
    /// Integer.
    #[primitive]
    Integer,

    /// Unsigned integer.
    #[primitive]
    UnsignedInteger,

    /// Boolean.
    #[primitive]
    Boolean,

    /// Secret integer.
    #[primitive]
    SecretInteger,

    /// Secret unsigned integer.
    #[primitive]
    SecretUnsignedInteger,

    /// Secret boolean.
    #[primitive]
    SecretBoolean,

    /// Secret blob.
    #[primitive]
    SecretBlob,

    /// Shamir share integer.
    #[primitive]
    ShamirShareInteger,

    /// Shamir share unsigned integer.
    #[primitive]
    ShamirShareUnsignedInteger,

    /// Shamir share boolean.
    #[primitive]
    ShamirShareBoolean,

    /// Array: collection of homogeneous values.
    #[skip_new_function]
    Array {
        /// Inner type for this array. Used to enforce that all elements of this array have the same type.
        inner_type: Box<Self>,

        /// Array length.
        size: usize,
    },

    /// Tuple: two heterogeneous values.
    #[skip_new_function]
    Tuple {
        /// Left type.
        left_type: Box<Self>,

        /// Right type.
        right_type: Box<Self>,
    },

    /// ECDSA private key for the threshold ecdsa signature feature.
    #[primitive]
    EcdsaPrivateKey,

    /// NTuple: any number of heterogeneous values.
    #[skip_new_function]
    NTuple {
        /// NTuple types.
        types: Vec<Self>,
    },

    /// Public ECDSA message digest.
    #[primitive]
    EcdsaDigestMessage,

    /// Object: key-value hash map.
    #[skip_new_function]
    Object {
        /// Key-value types.
        types: HashableIndexMap<String, Self>,
    },

    /// Private ECDSA signature.
    #[primitive]
    EcdsaSignature,

    /// ECDSA public key for the threshold ecdsa signature feature.
    #[primitive]
    EcdsaPublicKey,

    /// Store id.
    #[primitive]
    StoreId,
}

impl NadaType {
    /// Returns the public representation for a type
    pub fn as_public(&self) -> Result<Self, TypeError> {
        let metadata: NadaTypeMetadata = self.into();
        (&metadata.with_shape(Shape::PublicVariable)).try_into()
    }

    /// Returns the shamir share representation for a type
    pub fn as_shamir_share(&self) -> Result<Self, TypeError> {
        let metadata: NadaTypeMetadata = self.into();
        (&metadata.with_shape(Shape::ShamirShare)).try_into()
    }

    /// Returns a new array.
    pub fn new_array(inner_type: Self, size: usize) -> Result<Self, TypeError> {
        let value = NadaType::Array { inner_type: Box::new(inner_type), size };

        if value.recursion_depth() > MAX_RECURSION_DEPTH {
            return Err(TypeError::MaxRecursionDepthExceeded);
        }

        Ok(value)
    }

    /// Returns a new tuple.
    pub fn new_tuple(left: Self, right: Self) -> Result<Self, TypeError> {
        let value = NadaType::Tuple { left_type: Box::new(left), right_type: Box::new(right) };

        if value.recursion_depth() > MAX_RECURSION_DEPTH {
            return Err(TypeError::MaxRecursionDepthExceeded);
        }

        Ok(value)
    }

    /// Returns a new ntuple.
    pub fn new_n_tuple(types: Vec<Self>) -> Result<Self, TypeError> {
        let value = NadaType::NTuple { types };

        if value.recursion_depth() > MAX_RECURSION_DEPTH {
            return Err(TypeError::MaxRecursionDepthExceeded);
        }

        Ok(value)
    }

    /// Returns a new object.
    pub fn new_object(types: IndexMap<String, Self>) -> Result<Self, TypeError> {
        let value = NadaType::Object { types: types.into() };

        if value.recursion_depth() > MAX_RECURSION_DEPTH {
            return Err(TypeError::MaxRecursionDepthExceeded);
        }

        Ok(value)
    }

    /// Returns true if a type is a public type
    pub fn is_public(&self) -> bool {
        use NadaType::*;
        let mut inner_types = vec![self];
        // A type will be public if all inner types are public. Otherwise, it is not.
        while let Some(ty) = inner_types.pop() {
            match ty {
                Integer | UnsignedInteger | Boolean | EcdsaDigestMessage => {
                    // Do nothing
                }
                Array { inner_type, .. } => inner_types.push(inner_type),
                Tuple { left_type, right_type } => {
                    inner_types.push(left_type);
                    inner_types.push(right_type);
                }
                NTuple { types } => {
                    inner_types.extend(types);
                }
                Object { types } => {
                    inner_types.extend(types.values());
                }
                _ => return false,
            }
        }
        true
    }

    /// Returns true if a type is a secret type
    pub fn is_secret(&self) -> bool {
        !self.is_public()
    }

    /// Returns true if a type is a secret share type
    pub fn is_secret_share(&self) -> bool {
        if let Ok(count) = self.elements_count() { count.share > 0 } else { false }
    }

    /// Returns the corresponding user type. Returns itself if it is already a user type.
    ///
    /// The purpose of this method is to convert from Shamir or "internal types" into "user types".
    /// The "user secret types" are used to provide secrets in clear form in storage and compute operations.
    /// The runtime converts these types into "internal types", currently Shamir shares.
    ///
    /// In order to return the values to the user, we need to convert from the internal type back to the user type.
    /// This is the purpose of this method.
    ///
    /// For Public types the method returns the same value transparently. The reason is that public types are already 'user types'
    pub fn to_user_type(&self) -> Self {
        use NadaType::*;
        let mut result = self.clone();
        let mut inner_types = vec![&mut result];
        while let Some(ty) = inner_types.pop() {
            match ty {
                // Public types are already 'user types'
                Integer
                | UnsignedInteger
                | Boolean
                | EcdsaDigestMessage
                | EcdsaPublicKey
                | StoreId
                // Secret "user types" do not need to be changed
                | SecretInteger
                | SecretUnsignedInteger
                | SecretBoolean
                | SecretBlob
                | EcdsaPrivateKey
                | EcdsaSignature => {
                    // Do nothing
                },
                // Share types convert to usual secret types
                ShamirShareBoolean => *ty = SecretBoolean,
                ShamirShareInteger => *ty = SecretInteger,
                ShamirShareUnsignedInteger => *ty = SecretUnsignedInteger,
                // For Compound types the inner types are processed
                Array { inner_type, .. } => inner_types.push(inner_type),
                Tuple { left_type, right_type } => {
                    inner_types.push(left_type);
                    inner_types.push(right_type);
                }
                NTuple { types } => {
                    for inner_type in types {
                        inner_types.push(inner_type);
                    }
                }
                Object { types } => {
                    for inner_type in types.values_mut() {
                        inner_types.push(inner_type);
                    }
                }
            }
        }
        result
    }

    /// Returns the corresponding 'internal type'.
    ///
    /// This is the reverse of `to_user_type`
    pub fn to_internal_type(&self) -> Self {
        use NadaType::*;
        let mut result = self.clone();
        let mut inner_types = vec![&mut result];
        while let Some(ty) = inner_types.pop() {
            match ty {
                // Public types are already 'internal types'
                Integer
                | UnsignedInteger
                | Boolean
                | EcdsaDigestMessage
                | EcdsaPublicKey
                | StoreId
                // ShamirShares are already 'internal types'
                | ShamirShareBoolean
                | ShamirShareInteger
                | ShamirShareUnsignedInteger
                // Secret Blob is already 'internal type'
                | SecretBlob
                | EcdsaPrivateKey
                | EcdsaSignature => {
                    // Do nothing
                }
                SecretInteger => *ty = ShamirShareInteger,
                SecretUnsignedInteger => *ty = ShamirShareUnsignedInteger,
                SecretBoolean => *ty = ShamirShareBoolean,
                // For Compound types the inner types are processed
                Array { inner_type, .. } => inner_types.push(inner_type),
                Tuple { left_type, right_type } => {
                    inner_types.push(left_type);
                    inner_types.push(right_type);
                }
                NTuple { types } => {
                    for inner_type in types {
                        inner_types.push(inner_type);
                    }
                }
                Object { types } => {
                    for inner_type in types.values_mut() {
                        inner_types.push(inner_type);
                    }
                }
            }
        }
        result
    }

    /// Returns the corresponding public type. Returns itself if it is already a public type.
    pub fn to_public(&self) -> Result<Self, TypeError> {
        let metadata: NadaTypeMetadata = self.into();
        (&metadata.with_shape(Shape::PublicVariable)).try_into()
    }

    /// Returns the corresponding secret Shamir type. If it is already secret,
    /// it returns itself. This works similar to `to_secret` but it always
    /// returns Shamir secret types.
    pub fn to_secret_shamir(&self) -> Result<Self, TypeError> {
        let metadata: NadaTypeMetadata = self.into();
        (&metadata.with_shape(Shape::ShamirShare)).try_into()
    }

    /// Returns the inner types if it is a compound type or an empty vector if it is a primitive type
    pub fn compound_inner_types(&self) -> Vec<&NadaType> {
        use NadaType::*;
        match self {
            Tuple { left_type, right_type } => {
                vec![left_type.as_ref(), right_type.as_ref()]
            }
            Array { inner_type, .. } => {
                vec![inner_type.as_ref()]
            }
            _ => vec![],
        }
    }

    /// Returns the number of primitive types that are required to represent this [`NadaType`]
    pub fn primitive_elements_count(&self) -> usize {
        let mut count = 0usize;
        let mut inner_types = vec![(self, 1)];
        use NadaType::*;
        while let Some((ty, multiplier)) = inner_types.pop() {
            match ty {
                Integer
                | UnsignedInteger
                | Boolean
                | EcdsaDigestMessage
                | EcdsaPublicKey
                | StoreId
                | SecretInteger
                | SecretUnsignedInteger
                | SecretBoolean
                | SecretBlob
                | ShamirShareInteger
                | ShamirShareUnsignedInteger
                | ShamirShareBoolean
                | EcdsaPrivateKey
                | EcdsaSignature => count = count.wrapping_add(multiplier),
                Array { size, inner_type } => {
                    inner_types.push((inner_type, multiplier.wrapping_mul(*size)));
                }
                Tuple { left_type, right_type } => {
                    inner_types.push((left_type, multiplier));
                    inner_types.push((right_type, multiplier));
                }
                NTuple { types } => {
                    for inner_type in types {
                        inner_types.push((inner_type, multiplier));
                    }
                }
                Object { types } => {
                    for inner_type in types.values() {
                        inner_types.push((inner_type, multiplier));
                    }
                }
            }
        }
        count
    }

    /// Count the shares and public elements in a [`NadaType`].
    pub fn elements_count(&self) -> Result<ElementsCount, CantCountError> {
        use NadaType::*;
        let mut count = ElementsCount { public: 0, share: 0, ecdsa_private_key_shares: 0, ecdsa_signature_shares: 0 };
        let mut inner_types = vec![(self, 1)];
        while let Some((ty, multiplier)) = inner_types.pop() {
            match ty {
                Integer | UnsignedInteger | Boolean | EcdsaDigestMessage | EcdsaPublicKey | StoreId => {
                    count.public = count.public.saturating_add(multiplier)
                }
                SecretInteger
                | SecretUnsignedInteger
                | SecretBoolean
                | ShamirShareInteger
                | ShamirShareUnsignedInteger
                | ShamirShareBoolean => count.share = count.share.saturating_add(multiplier),
                EcdsaPrivateKey => {
                    count.ecdsa_private_key_shares = count.ecdsa_private_key_shares.saturating_add(multiplier)
                }
                EcdsaSignature => {
                    count.ecdsa_signature_shares = count.ecdsa_signature_shares.saturating_add(multiplier)
                }
                Array { inner_type, size } => {
                    inner_types.push((inner_type, multiplier.wrapping_mul(*size)));
                }
                Tuple { left_type, right_type } => {
                    inner_types.push((left_type, multiplier));
                    inner_types.push((right_type, multiplier));
                }
                NTuple { types } => {
                    for inner_type in types {
                        inner_types.push((inner_type, multiplier));
                    }
                }
                Object { types } => {
                    for inner_type in types.values() {
                        inner_types.push((inner_type, multiplier));
                    }
                }
                SecretBlob => return Err(CantCountError::CantCountSecretBlobShares),
            }
        }
        Ok(count)
    }

    /// Returns true if this [`NadaType`] and the other [`NadaType`] contain the same underlying type.
    /// For instance, SecretInteger and Integer have the same underlying type: Integer.
    pub fn has_same_underlying_type(&self, other: &Self) -> bool {
        let self_metadata = NadaTypeMetadata::from(self);
        let other_metadata = NadaTypeMetadata::from(other);
        self_metadata.nada_primitive_type() == other_metadata.nada_primitive_type()
    }

    /// Returns the recursion depth.
    fn recursion_depth(&self) -> usize {
        let mut stack = vec![(self, 1)];
        let mut max_depth = 0;

        while let Some((value, depth)) = stack.pop() {
            use NadaType::*;

            max_depth = max_depth.max(depth);

            match value {
                Integer
                | UnsignedInteger
                | Boolean
                | EcdsaDigestMessage
                | EcdsaPublicKey
                | StoreId
                | SecretInteger
                | SecretUnsignedInteger
                | SecretBoolean
                | SecretBlob
                | ShamirShareInteger
                | ShamirShareUnsignedInteger
                | ShamirShareBoolean
                | EcdsaPrivateKey
                | EcdsaSignature => {}
                Array { inner_type, .. } => {
                    stack.push((inner_type, depth + 1));
                }
                Tuple { left_type, right_type } => {
                    stack.push((left_type, depth + 1));
                    stack.push((right_type, depth + 1));
                }
                NTuple { types } => {
                    for inner_type in types {
                        stack.push((inner_type, depth + 1));
                    }
                }
                Object { types } => {
                    for inner_type in types.values() {
                        stack.push((inner_type, depth + 1));
                    }
                }
            }
        }

        max_depth
    }

    /// Returns a list with the type and every type that it contains.
    /// For instance, for Array { inner_type: SecretInteger, size } this returns
    /// [
    ///   Array { inner_type: SecretInteger, size },
    ///   SecretInteger,
    ///   SecretInteger,
    ///   SecretInteger,
    ///   SecretInteger,
    ///   SecretInteger
    /// ]
    pub fn flatten_inner_types(self) -> Vec<NadaType> {
        let mut flattened_types = vec![];
        let mut types = vec![self];
        while let Some(ty) = types.pop() {
            match &ty {
                NadaType::Integer
                | NadaType::UnsignedInteger
                | NadaType::Boolean
                | NadaType::EcdsaDigestMessage
                | NadaType::SecretInteger
                | NadaType::SecretUnsignedInteger
                | NadaType::SecretBoolean
                | NadaType::SecretBlob
                | NadaType::ShamirShareInteger
                | NadaType::ShamirShareUnsignedInteger
                | NadaType::ShamirShareBoolean
                | NadaType::EcdsaPrivateKey
                | NadaType::EcdsaSignature
                | NadaType::EcdsaPublicKey
                | NadaType::StoreId => flattened_types.push(ty),
                NadaType::Array { inner_type, size } => {
                    types.extend(vec![inner_type.as_ref().clone(); *size]);
                    flattened_types.push(ty);
                }
                NadaType::Tuple { left_type, right_type } => {
                    types.push(*left_type.clone());
                    types.push(*right_type.clone());
                    flattened_types.push(ty);
                }
                NadaType::NTuple { types: inner_types } => {
                    types.extend_from_slice(inner_types);
                    flattened_types.push(ty);
                }
                NadaType::Object { types: inner_types } => {
                    types.extend(inner_types.values().cloned());
                    flattened_types.push(ty);
                }
            }
        }
        flattened_types
    }
}

/// Represents the number of elements of a type.
pub struct ElementsCount {
    /// Number of public elements.
    pub public: usize,
    /// Number of share elements.
    pub share: usize,
    /// The number of ecdsa key share elements.
    pub ecdsa_private_key_shares: usize,
    /// The number of ecdsa signature share elements.
    pub ecdsa_signature_shares: usize,
}

/// Error when trying to count either secret blob or ecdsa private key shares.
#[derive(Error, Debug)]
pub enum CantCountError {
    /// Error when trying to count secret blob shares as Nada Type doesn't know the size of the blob.
    #[error("Can't count secret blob shares from NadaType")]
    CantCountSecretBlobShares,

    /// Error when trying to count ecdsa private keys.
    #[error("Can't count Ecdsa private key shares from NadaType")]
    CantCountEcdsaPrivateKey,
}

impl Display for NadaType {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        use NadaType::*;
        match self {
            Array { inner_type, size } => write!(f, "Array [{inner_type}:{size:?}]"),
            Tuple { left_type, right_type } => write!(f, "Tuple ({left_type}, {right_type})"),
            _ => write!(f, "{self:?}"),
        }
    }
}

impl Display for NadaTypeKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl TryFrom<&NadaTypeMetadata> for NadaType {
    type Error = TypeError;

    fn try_from(value: &NadaTypeMetadata) -> Result<Self, Self::Error> {
        Ok(match value {
            NadaTypeMetadata::PrimitiveType {
                shape: Shape::PublicVariable,
                nada_primitive_type: NadaPrimitiveType::Integer,
                ..
            } => NadaType::Integer,
            NadaTypeMetadata::PrimitiveType {
                shape: Shape::PublicVariable,
                nada_primitive_type: NadaPrimitiveType::UnsignedInteger,
                ..
            } => NadaType::UnsignedInteger,
            NadaTypeMetadata::PrimitiveType {
                shape: Shape::PublicVariable,
                nada_primitive_type: NadaPrimitiveType::Boolean,
                ..
            } => NadaType::Boolean,
            NadaTypeMetadata::PrimitiveType {
                shape: Shape::PublicVariable,
                nada_primitive_type: NadaPrimitiveType::EcdsaDigestMessage,
                ..
            } => NadaType::EcdsaDigestMessage,
            NadaTypeMetadata::PrimitiveType {
                shape: Shape::PublicVariable,
                nada_primitive_type: NadaPrimitiveType::EcdsaPublicKey,
                ..
            } => NadaType::EcdsaPublicKey,
            NadaTypeMetadata::PrimitiveType {
                shape: Shape::PublicVariable,
                nada_primitive_type: NadaPrimitiveType::StoreId,
                ..
            } => NadaType::StoreId,
            NadaTypeMetadata::PrimitiveType {
                shape: Shape::PublicVariable,
                nada_primitive_type: NadaPrimitiveType::Blob,
                ..
            } => return Err(TypeError::unimplemented("public variable blob")),
            NadaTypeMetadata::PrimitiveType {
                shape: Shape::PublicVariable,
                nada_primitive_type: NadaPrimitiveType::EcdsaPrivateKey,
                ..
            } => return Err(TypeError::unimplemented("public variable ecdsa private key")),
            NadaTypeMetadata::PrimitiveType {
                shape: Shape::PublicVariable,
                nada_primitive_type: NadaPrimitiveType::EcdsaSignature,
                ..
            } => return Err(TypeError::unimplemented("public variable ecdsa signature")),
            NadaTypeMetadata::PrimitiveType {
                shape: Shape::Secret,
                nada_primitive_type: NadaPrimitiveType::Integer,
                ..
            } => NadaType::SecretInteger,
            NadaTypeMetadata::PrimitiveType {
                shape: Shape::Secret,
                nada_primitive_type: NadaPrimitiveType::UnsignedInteger,
                ..
            } => NadaType::SecretUnsignedInteger,
            NadaTypeMetadata::PrimitiveType {
                shape: Shape::Secret,
                nada_primitive_type: NadaPrimitiveType::Boolean,
                ..
            } => NadaType::SecretBoolean,
            NadaTypeMetadata::PrimitiveType {
                shape: Shape::Secret,
                nada_primitive_type: NadaPrimitiveType::EcdsaDigestMessage,
                ..
            } => return Err(TypeError::unimplemented("secret variable ecdsa digest message")),
            NadaTypeMetadata::PrimitiveType {
                shape: Shape::Secret,
                nada_primitive_type: NadaPrimitiveType::EcdsaPublicKey,
                ..
            } => return Err(TypeError::unimplemented("secret variable ecdsa public key")),
            NadaTypeMetadata::PrimitiveType {
                shape: Shape::Secret,
                nada_primitive_type: NadaPrimitiveType::StoreId,
                ..
            } => return Err(TypeError::unimplemented("secret variable store id")),
            NadaTypeMetadata::PrimitiveType {
                shape: Shape::Secret,
                nada_primitive_type: NadaPrimitiveType::Blob,
                ..
            } => NadaType::SecretBlob,
            NadaTypeMetadata::PrimitiveType {
                shape: Shape::Secret,
                nada_primitive_type: NadaPrimitiveType::EcdsaPrivateKey,
                ..
            } => NadaType::EcdsaPrivateKey,
            NadaTypeMetadata::PrimitiveType {
                shape: Shape::Secret,
                nada_primitive_type: NadaPrimitiveType::EcdsaSignature,
                ..
            } => NadaType::EcdsaSignature,
            NadaTypeMetadata::PrimitiveType {
                shape: Shape::ShamirShare,
                nada_primitive_type: NadaPrimitiveType::Integer,
                ..
            } => NadaType::ShamirShareInteger,
            NadaTypeMetadata::PrimitiveType {
                shape: Shape::ShamirShare,
                nada_primitive_type: NadaPrimitiveType::UnsignedInteger,
                ..
            } => NadaType::ShamirShareUnsignedInteger,
            NadaTypeMetadata::PrimitiveType {
                shape: Shape::ShamirShare,
                nada_primitive_type: NadaPrimitiveType::Boolean,
                ..
            } => NadaType::ShamirShareBoolean,
            NadaTypeMetadata::PrimitiveType {
                shape: Shape::ShamirShare,
                nada_primitive_type: NadaPrimitiveType::EcdsaDigestMessage,
                ..
            } => return Err(TypeError::unimplemented("shamir share ecdsa digest message")),
            NadaTypeMetadata::PrimitiveType {
                shape: Shape::ShamirShare,
                nada_primitive_type: NadaPrimitiveType::EcdsaPublicKey,
                ..
            } => return Err(TypeError::unimplemented("shamir share ecdsa public key")),
            NadaTypeMetadata::PrimitiveType {
                shape: Shape::ShamirShare,
                nada_primitive_type: NadaPrimitiveType::StoreId,
                ..
            } => return Err(TypeError::unimplemented("shamir share store id")),
            NadaTypeMetadata::PrimitiveType {
                shape: Shape::ShamirShare,
                nada_primitive_type: NadaPrimitiveType::Blob,
                ..
            } => return Err(TypeError::unimplemented("shamir share blob")),
            NadaTypeMetadata::PrimitiveType {
                shape: Shape::ShamirShare,
                nada_primitive_type: NadaPrimitiveType::EcdsaPrivateKey,
                ..
            } => return Err(TypeError::unimplemented("shamir share ecdsa private key")),
            NadaTypeMetadata::PrimitiveType {
                shape: Shape::ShamirShare,
                nada_primitive_type: NadaPrimitiveType::EcdsaSignature,
                ..
            } => return Err(TypeError::unimplemented("shamir share ecdsa signautre")),

            NadaTypeMetadata::Array { size, inner } => {
                NadaType::Array { size: *size, inner_type: Box::new(inner.as_ref().try_into()?) }
            }
            NadaTypeMetadata::Tuple { left, right } => NadaType::Tuple {
                left_type: Box::new(left.as_ref().try_into()?),
                right_type: Box::new(right.as_ref().try_into()?),
            },
            NadaTypeMetadata::NTuple { types } => NadaType::NTuple {
                types: types.iter().map(|inner_type| inner_type.try_into()).collect::<Result<Vec<_>, Self::Error>>()?,
            },
            NadaTypeMetadata::Object { types } => {
                let mut new_types = IndexMap::with_capacity(types.len());
                for (name, inner_type) in types {
                    new_types.insert(name.clone(), inner_type.try_into()?);
                }
                NadaType::Object { types: new_types.into() }
            }
        })
    }
}

/// Type error: can be returned when creating certain types.
#[derive(Error, Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TypeError {
    /// Non-empty vector only.
    #[error("only a non-empty vector is allowed")]
    NonEmptyVecOnly,

    /// Homogeneous vector only.
    #[error("only a vector with homogeneous types (same type variant) is allowed")]
    HomogeneousVecOnly,

    /// Maximum recursion depth exceeded.
    #[error("maximum recursion depth of {} exceeded", MAX_RECURSION_DEPTH)]
    MaxRecursionDepthExceeded,

    /// Zero value is not allowed.
    #[error("providing zero is not possible")]
    ZeroValue,

    /// Zero value is not allowed.
    #[error("{0} is unimplemented")]
    Unimplemented(String),
}

impl TypeError {
    pub fn unimplemented<I: Into<String>>(s: I) -> Self {
        TypeError::Unimplemented(s.into())
    }
}

/// A primitive type that cannot be implemented.
#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(unreachable_code)]
pub struct NeverPrimitiveType(!);

#[cfg(feature = "serde")]
impl serde::Serialize for NeverPrimitiveType {
    fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::Error;

        Err(Error::custom("cannot serialize a never type"))
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for NeverPrimitiveType {
    fn deserialize<D>(_deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        use serde::de::Error;
        Err(Error::custom("cannot deserialize a never type"))
    }
}

#[cfg(test)]
mod tests {
    use crate::NadaType;

    #[test]
    fn test_has_same_underlying_type() {
        assert!(NadaType::Integer.has_same_underlying_type(&NadaType::Integer));
        assert!(NadaType::SecretInteger.has_same_underlying_type(&NadaType::Integer));
        assert!(NadaType::SecretInteger.has_same_underlying_type(&NadaType::SecretInteger));
        assert!(!NadaType::Integer.has_same_underlying_type(&NadaType::SecretBoolean));
    }
}

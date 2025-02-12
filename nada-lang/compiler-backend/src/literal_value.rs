//! Contains a literal

use nada_value::{NadaType, NadaValue, NeverPrimitiveType, PrimitiveTypes};
use num_bigint::{BigInt, BigUint};
use std::str::FromStr;

/// Primitive types for literals.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LiteralPrimitiveTypes;

impl PrimitiveTypes for LiteralPrimitiveTypes {
    type Integer = BigInt;
    type UnsignedInteger = BigUint;
    type Boolean = bool;
    type SecretInteger = NeverPrimitiveType;
    type SecretUnsignedInteger = NeverPrimitiveType;

    type SecretBoolean = NeverPrimitiveType;
    type EcdsaDigestMessage = NeverPrimitiveType;
    type SecretBlob = NeverPrimitiveType;
    type ShamirShareInteger = NeverPrimitiveType;
    type ShamirShareUnsignedInteger = NeverPrimitiveType;
    type ShamirShareBoolean = NeverPrimitiveType;

    type EcdsaPrivateKey = NeverPrimitiveType;
    type EcdsaSignature = NeverPrimitiveType;
    type EcdsaPublicKey = NeverPrimitiveType;

    type StoreId = NeverPrimitiveType;
}

/// Common type for literals
pub type LiteralValue = NadaValue<LiteralPrimitiveTypes>;

/// Literal extension functions.
pub trait LiteralValueExt {
    /// Create a literal from a string and a type.
    fn from_str(value: &str, ty: &NadaType) -> Result<Self, LiteralValueError>
    where
        Self: Sized;
}

impl LiteralValueExt for LiteralValue {
    /// Create a literal from a string and a type.
    fn from_str(value: &str, ty: &NadaType) -> Result<Self, LiteralValueError> {
        use NadaType::*;
        match ty {
            Integer => Ok(Self::new_integer(
                BigInt::from_str(value)
                    .map_err(|_| LiteralValueError::ParsingFailed(value.to_owned(), "integer".to_owned()))?,
            )),
            UnsignedInteger => Ok(Self::new_unsigned_integer(
                BigUint::from_str(value)
                    .map_err(|_| LiteralValueError::ParsingFailed(value.to_owned(), "unsigned integer".to_owned()))?,
            )),
            Boolean => {
                let value = bool::from_str(&value.to_lowercase())
                    .map_err(|_| LiteralValueError::ParsingFailed(value.to_owned(), "boolean".to_owned()))?;
                Ok(Self::new_boolean(value))
            }
            _ => Err(LiteralValueError::Unimplemented(format!("literal of type {ty:?}")))?,
        }
    }
}

/// An error related to literal values.
#[derive(Debug, thiserror::Error)]
pub enum LiteralValueError {
    /// Failed parsing a literal value from a string.
    #[error("failed to parse {0} as a {1} literal value")]
    ParsingFailed(String, String),

    /// Not implemented.
    #[error("not implemented: {0}")]
    Unimplemented(String),
}

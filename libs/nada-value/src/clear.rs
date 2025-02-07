//! Clear types
//!
//! Clear values are the values provided by the user, in clear (plaintext) form,
//! regardless of whether they are secret or not. They represent the data types used at the client / dealer end.

use crate::{NadaInt, NadaUint, NadaValue, NeverPrimitiveType};
use ecdsa_keypair::{privatekey::EcdsaPrivateKey, signature::EcdsaSignature};
use nada_type::PrimitiveTypes;
use std::fmt::Display;

/// Clear values are the values provided by the user, in clear (plaintext) form,
/// regardless of whether they are secret or not. They represent the data types used at the client / dealer end.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "secret-serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Clear;

impl PrimitiveTypes for Clear {
    // Public variables
    type Integer = NadaInt;
    type UnsignedInteger = NadaUint;
    type Boolean = bool;
    type EcdsaDigestMessage = [u8; 32];

    // Abstract secrets
    type SecretInteger = NadaInt;
    type SecretUnsignedInteger = NadaUint;

    type SecretBoolean = bool;
    type SecretBlob = Vec<u8>;

    // Shares
    type ShamirShareInteger = NeverPrimitiveType;
    type ShamirShareUnsignedInteger = NeverPrimitiveType;
    type ShamirShareBoolean = NeverPrimitiveType;

    // Ecdsa private key
    type EcdsaPrivateKey = EcdsaPrivateKey;

    // Ecdsa signature
    type EcdsaSignature = EcdsaSignature;
}

impl Display for NadaValue<Clear> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NadaValue::Integer(value) => write!(f, "{}({})", self.to_type_kind(), value),
            NadaValue::UnsignedInteger(value) => write!(f, "{}({})", self.to_type_kind(), value),
            NadaValue::Boolean(value) => write!(f, "{}({})", self.to_type_kind(), value),
            NadaValue::SecretInteger(value) => write!(f, "{}({})", self.to_type_kind(), value),
            NadaValue::SecretUnsignedInteger(value) => write!(f, "{}({})", self.to_type_kind(), value),

            NadaValue::SecretBoolean(value) => write!(f, "{}({})", self.to_type_kind(), value),
            NadaValue::SecretBlob(value) => {
                write!(f, "Blob({})", value.iter().map(|value| value.to_string()).collect::<Vec<_>>().join(", "))
            }
            NadaValue::EcdsaDigestMessage(value) => {
                write!(
                    f,
                    "EcdsaDigestMessage({})",
                    value.iter().map(|value| value.to_string()).collect::<Vec<_>>().join(", ")
                )
            }
            NadaValue::ShamirShareInteger(_) => write!(f, "{}(NeverType)", self.to_type_kind()),
            NadaValue::ShamirShareUnsignedInteger(_) => write!(f, "{}(NeverType)", self.to_type_kind()),
            NadaValue::ShamirShareBoolean(_) => write!(f, "{}(NeverType)", self.to_type_kind()),

            NadaValue::Array { values, .. } => {
                write!(f, "Array({})", values.iter().map(|value| value.to_string()).collect::<Vec<_>>().join(", "))
            }
            NadaValue::Tuple { left, right } => write!(f, "Tuple({}, {})", left, right),
            NadaValue::EcdsaPrivateKey(value) => write!(f, "{}({})", self.to_type_kind(), value),
            NadaValue::EcdsaSignature(value) => write!(f, "{}({})", self.to_type_kind(), value),
            NadaValue::NTuple { values } => {
                write!(f, "NTuple({})", values.iter().map(|value| value.to_string()).collect::<Vec<_>>().join(", "))
            }
            NadaValue::Object { values } => {
                write!(
                    f,
                    "Object({})",
                    values.iter().map(|(key, value)| format!("{}:{}", key, value)).collect::<Vec<_>>().join(", ")
                )
            }
        }
    }
}

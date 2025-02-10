//! nada value <-> protobuf conversions.

use crate::{
    encrypted::{BlobPrimitiveType, Encoded, Encrypted},
    NadaValue,
};
use ecdsa_keypair::{privatekey::EcdsaPrivateKeyShare, signature::EcdsaSignatureShare};
use generic_ec::{curves::Secp256k1, serde::CurveName, NonZero, Point, Scalar, SecretScalar};
use key_share::{DirtyCoreKeyShare, DirtyKeyInfo, Validate};
use math_lib::modular::{EncodedModularNumber, EncodedModulo};
use nada_type::{NadaType, TypeError};
use node_api::values::proto::value::{self, value::Value, ShamirShare};
use std::collections::HashMap;

/// Encode nada values into protobuf.
pub fn nada_values_to_protobuf(
    values: HashMap<String, NadaValue<Encrypted<Encoded>>>,
) -> Result<Vec<value::NamedValue>, ValueEncodeError> {
    let mut output = Vec::new();
    for (name, value) in values {
        let value = nada_value_to_protobuf(value)?;
        output.push(value::NamedValue { name, value: Some(value) });
    }
    Ok(output)
}

/// Decode nada values from protobuf.
pub fn nada_values_from_protobuf(
    values: Vec<value::NamedValue>,
    modulo: &EncodedModulo,
) -> Result<HashMap<String, NadaValue<Encrypted<Encoded>>>, ValueDecodeError> {
    let mut output = HashMap::new();
    for named_value in values {
        let value = named_value.value.ok_or(ValueDecodeError::NoValue)?;
        let value = nada_value_from_protobuf(value, modulo)?;
        if output.contains_key(&named_value.name) {
            return Err(ValueDecodeError::DuplicateValue(named_value.name));
        }
        output.insert(named_value.name.clone(), value);
    }
    Ok(output)
}

pub(crate) fn nada_value_to_protobuf(value: NadaValue<Encrypted<Encoded>>) -> Result<value::Value, ValueEncodeError> {
    let value = match value {
        NadaValue::Integer(value) => Value::PublicInteger(value::PublicInteger { value: value.into_bytes() }),
        NadaValue::UnsignedInteger(value) => {
            Value::PublicUnsignedInteger(value::PublicInteger { value: value.as_bytes().to_vec() })
        }
        NadaValue::Boolean(value) => Value::PublicBoolean(value::PublicInteger { value: value.into_bytes() }),
        NadaValue::ShamirShareInteger(value) => {
            Value::ShamirShareInteger(value::ShamirShare { value: value.into_bytes() })
        }
        NadaValue::ShamirShareUnsignedInteger(value) => {
            Value::ShamirShareUnsignedInteger(value::ShamirShare { value: value.into_bytes() })
        }
        NadaValue::ShamirShareBoolean(value) => {
            Value::ShamirShareBoolean(value::ShamirShare { value: value.into_bytes() })
        }
        NadaValue::SecretBlob(shares) => Value::ShamirSharesBlob(value::ShamirSharesBlob {
            shares: shares.value.into_iter().map(|s| ShamirShare { value: s.into_bytes() }).collect(),
            original_size: shares.unencoded_size,
        }),
        NadaValue::Array { values, inner_type } => Value::Array(value::Array {
            values: values.into_iter().map(nada_value_to_protobuf).collect::<Result<_, _>>()?,
            inner_type: Some(nada_type_to_protobuf(&inner_type)?),
        }),
        NadaValue::Tuple { left, right } => Value::Tuple(Box::new(value::Tuple {
            left: Some(Box::new(nada_value_to_protobuf(*left)?)),
            right: Some(Box::new(nada_value_to_protobuf(*right)?)),
        })),
        NadaValue::EcdsaPrivateKey(value) => {
            let value = value.as_inner();
            Value::EcdsaPrivateKeyShare(value::EcdsaPrivateKeyShare {
                i: value.i.into(),
                x: value.x.clone().into_inner().as_ref().to_le_bytes().to_vec(),
                shared_public_key: value.key_info.shared_public_key.to_bytes(true).to_vec(),
                public_shares: value.key_info.public_shares.iter().map(|s| s.to_bytes(true).to_vec()).collect(),
            })
        }
        NadaValue::EcdsaDigestMessage(value) => {
            Value::EcdsaMessageDigest(value::EcdsaMessageDigest { digest: value.to_vec() })
        }
        NadaValue::EcdsaPublicKey(value) => {
            Value::EcdsaPublicKey(value::EcdsaPublicKey { public_key: value.0.to_vec() })
        }
        NadaValue::StoreId(value) => Value::StoreId(value::StoreId { store_id: value.to_vec() }),
        NadaValue::EcdsaSignature(value) => Value::EcdsaSignatureShare(value::EcdsaSignatureShare {
            r: value.r.to_le_bytes().to_vec(),
            sigma: value.sigma.to_le_bytes().to_vec(),
        }),
        NadaValue::SecretInteger(_)
        | NadaValue::SecretUnsignedInteger(_)
        | NadaValue::SecretBoolean(_)
        | NadaValue::NTuple { .. }
        | NadaValue::Object { .. } => return Err(ValueEncodeError::UnsupportedType(value.to_type())),
    };
    Ok(value::Value { value: Some(value) })
}

pub(crate) fn nada_value_from_protobuf(
    value: value::Value,
    modulo: &EncodedModulo,
) -> Result<NadaValue<Encrypted<Encoded>>, ValueDecodeError> {
    let value = value.value.ok_or(ValueDecodeError::NoValue)?;
    let value = match value {
        Value::PublicBoolean(value) => {
            NadaValue::new_boolean(EncodedModularNumber::new_unchecked(value.value, *modulo))
        }
        Value::PublicInteger(value) => {
            NadaValue::new_integer(EncodedModularNumber::new_unchecked(value.value, *modulo))
        }
        Value::PublicUnsignedInteger(value) => {
            NadaValue::new_unsigned_integer(EncodedModularNumber::new_unchecked(value.value, *modulo))
        }
        Value::ShamirShareBoolean(value) => {
            NadaValue::new_shamir_share_boolean(EncodedModularNumber::new_unchecked(value.value, *modulo))
        }
        Value::ShamirShareInteger(value) => {
            NadaValue::new_shamir_share_integer(EncodedModularNumber::new_unchecked(value.value, *modulo))
        }
        Value::ShamirShareUnsignedInteger(value) => {
            NadaValue::new_shamir_share_unsigned_integer(EncodedModularNumber::new_unchecked(value.value, *modulo))
        }
        Value::Array(array) => {
            let inner_type = array.inner_type.ok_or(ValueDecodeError::NoType)?;
            let result = NadaValue::new_array(
                nada_type_from_protobuf(&inner_type)?,
                array.values.into_iter().map(|v| nada_value_from_protobuf(v, modulo)).collect::<Result<_, _>>()?,
            );
            result.map_err(|e| {
                match e {
                    TypeError::HomogeneousVecOnly => {
                        ValueDecodeError::InvalidArray("arrays must only contain one type")
                    }
                    TypeError::MaxRecursionDepthExceeded => {
                        ValueDecodeError::InvalidArray("array nested depth is too large")
                    }
                    // These should not happen here so we fall back to some generic error.
                    TypeError::NonEmptyVecOnly | TypeError::ZeroValue | TypeError::Unimplemented(_) => {
                        ValueDecodeError::InvalidArray("unknown error")
                    }
                }
            })?
        }
        Value::Tuple(tuple) => {
            let left = tuple.left.ok_or(ValueDecodeError::NoValue)?;
            let right = tuple.right.ok_or(ValueDecodeError::NoValue)?;
            NadaValue::new_tuple(nada_value_from_protobuf(*left, modulo)?, nada_value_from_protobuf(*right, modulo)?)
                .map_err(|e| {
                    match e {
                        TypeError::MaxRecursionDepthExceeded => {
                            ValueDecodeError::InvalidTuple("tuple nested depth is too large")
                        }
                        // These should not happen here so we fall back to some generic error.
                        TypeError::HomogeneousVecOnly
                        | TypeError::NonEmptyVecOnly
                        | TypeError::ZeroValue
                        | TypeError::Unimplemented(_) => ValueDecodeError::InvalidTuple("unknown error"),
                    }
                })?
        }
        Value::ShamirSharesBlob(shares) => NadaValue::new_secret_blob(BlobPrimitiveType {
            value: shares.shares.into_iter().map(|s| EncodedModularNumber::new_unchecked(s.value, *modulo)).collect(),
            unencoded_size: shares.original_size,
        }),
        Value::EcdsaPrivateKeyShare(share) => {
            let share = DirtyCoreKeyShare {
                i: share.i.try_into().map_err(|_| ValueDecodeError::EcdsaPrivateKeyPartyOverflow(share.i))?,
                key_info: DirtyKeyInfo {
                    curve: CurveName::new(),
                    shared_public_key: non_zero_point_from_bytes(&share.shared_public_key)?,
                    public_shares: share
                        .public_shares
                        .iter()
                        .map(|s| non_zero_point_from_bytes(s))
                        .collect::<Result<_, _>>()?,
                    vss_setup: None,
                },
                x: non_zero_secret_scalar_from_bytes(&share.x)?,
            }
            .validate()
            .map_err(|e| ValueDecodeError::InvalidEcdsaPrivateKey(e.to_string()))?;
            NadaValue::new_ecdsa_private_key(EcdsaPrivateKeyShare::new(share))
        }
        Value::EcdsaSignatureShare(share) => NadaValue::new_ecdsa_signature(EcdsaSignatureShare {
            r: Scalar::from_le_bytes(&share.r).map_err(|_| ValueDecodeError::InvalidEcdsaSignatureScalar("r"))?,
            sigma: Scalar::from_le_bytes(&share.sigma)
                .map_err(|_| ValueDecodeError::InvalidEcdsaSignatureScalar("sigma"))?,
        }),
        Value::EcdsaMessageDigest(digest) => {
            let digest: [u8; 32] =
                digest.digest.try_into().map_err(|_| ValueDecodeError::InvalidEcdsaMessageDigestLength)?;
            NadaValue::new_ecdsa_digest_message(digest)
        }
        Value::EcdsaPublicKey(public_key) => {
            let public_key: [u8; 33] =
                public_key.public_key.try_into().map_err(|_| ValueDecodeError::InvalidStoreIdLength)?;
            NadaValue::new_ecdsa_public_key(public_key)
        }
        Value::StoreId(store_id) => {
            let store_id: [u8; 16] =
                store_id.store_id.try_into().map_err(|_| ValueDecodeError::InvalidStoreIdLength)?;
            NadaValue::new_store_id(store_id)
        }
    };
    Ok(value)
}

fn nada_type_to_protobuf(nada_type: &NadaType) -> Result<value::ValueType, ValueEncodeError> {
    let value_type = match nada_type {
        NadaType::Integer => value::value_type::ValueType::PublicInteger(()),
        NadaType::UnsignedInteger => value::value_type::ValueType::PublicUnsignedInteger(()),
        NadaType::Boolean => value::value_type::ValueType::PublicBoolean(()),
        NadaType::ShamirShareInteger => value::value_type::ValueType::ShamirShareInteger(()),
        NadaType::ShamirShareUnsignedInteger => value::value_type::ValueType::ShamirShareUnsignedInteger(()),
        NadaType::ShamirShareBoolean => value::value_type::ValueType::ShamirShareBoolean(()),
        NadaType::Array { inner_type, size } => value::value_type::ValueType::Array(Box::new(value::ArrayType {
            inner_type: Some(Box::new(nada_type_to_protobuf(inner_type)?)),
            size: u64::try_from(*size).map_err(|_| ValueEncodeError::ArraySizeOverflow(*size))?,
        })),
        NadaType::Tuple { left_type, right_type } => value::value_type::ValueType::Tuple(Box::new(value::TupleType {
            left: Some(Box::new(nada_type_to_protobuf(left_type)?)),
            right: Some(Box::new(nada_type_to_protobuf(right_type)?)),
        })),
        NadaType::EcdsaPrivateKey => value::value_type::ValueType::EcdsaPrivateKeyShare(()),
        NadaType::EcdsaDigestMessage => value::value_type::ValueType::EcdsaMessageDigest(()),
        NadaType::EcdsaSignature => value::value_type::ValueType::EcdsaSignatureShare(()),
        NadaType::EcdsaPublicKey => value::value_type::ValueType::EcdsaPublicKey(()),
        NadaType::StoreId => value::value_type::ValueType::StoreId(()),
        NadaType::SecretInteger
        | NadaType::SecretUnsignedInteger
        | NadaType::SecretBoolean
        | NadaType::SecretBlob
        | NadaType::NTuple { .. }
        | NadaType::Object { .. } => {
            return Err(ValueEncodeError::UnsupportedType(nada_type.clone()));
        }
    };
    Ok(value::ValueType { value_type: Some(value_type) })
}

fn nada_type_from_protobuf(value_type: &value::ValueType) -> Result<NadaType, ValueDecodeError> {
    let value_type = value_type.value_type.as_ref().ok_or(ValueDecodeError::NoType)?;
    let nada_type = match value_type {
        value::value_type::ValueType::PublicInteger(()) => NadaType::Integer,
        value::value_type::ValueType::PublicUnsignedInteger(()) => NadaType::UnsignedInteger,
        value::value_type::ValueType::PublicBoolean(()) => NadaType::Boolean,
        value::value_type::ValueType::ShamirShareInteger(()) => NadaType::ShamirShareInteger,
        value::value_type::ValueType::ShamirShareUnsignedInteger(()) => NadaType::ShamirShareUnsignedInteger,
        value::value_type::ValueType::ShamirShareBoolean(()) => NadaType::ShamirShareBoolean,
        value::value_type::ValueType::Array(array) => {
            let inner_type = array.inner_type.as_ref().ok_or(ValueDecodeError::NoType)?;
            NadaType::Array {
                inner_type: Box::new(nada_type_from_protobuf(inner_type)?),
                size: array.size.try_into().map_err(|_| ValueDecodeError::ArraySizeOverflow(array.size))?,
            }
        }
        value::value_type::ValueType::Tuple(tuple) => {
            let left = tuple.left.as_ref().ok_or(ValueDecodeError::NoType)?;
            let right = tuple.right.as_ref().ok_or(ValueDecodeError::NoType)?;
            NadaType::Tuple {
                left_type: Box::new(nada_type_from_protobuf(left)?),
                right_type: Box::new(nada_type_from_protobuf(right)?),
            }
        }
        value::value_type::ValueType::EcdsaPrivateKeyShare(()) => NadaType::EcdsaPrivateKey,
        value::value_type::ValueType::EcdsaMessageDigest(()) => NadaType::EcdsaDigestMessage,
        value::value_type::ValueType::EcdsaSignatureShare(()) => NadaType::EcdsaSignature,
        value::value_type::ValueType::EcdsaPublicKey(()) => NadaType::EcdsaPublicKey,
        value::value_type::ValueType::StoreId(()) => NadaType::StoreId,
    };
    Ok(nada_type)
}

fn non_zero_point_from_bytes(bytes: &[u8]) -> Result<NonZero<Point<Secp256k1>>, ValueDecodeError> {
    let point = Point::from_bytes(bytes).map_err(|_| ValueDecodeError::InvalidEcdsaPrivateKeyPoint("invalid bytes"))?;
    NonZero::from_point(point).ok_or(ValueDecodeError::InvalidEcdsaPrivateKeyPoint("point is zero"))
}

fn non_zero_secret_scalar_from_bytes(bytes: &[u8]) -> Result<NonZero<SecretScalar<Secp256k1>>, ValueDecodeError> {
    let scalar = SecretScalar::from_le_bytes(bytes)
        .map_err(|_| ValueDecodeError::InvalidEcdsaPrivateKeySecretScalar("invalid bytes"))?;
    NonZero::from_secret_scalar(scalar).ok_or(ValueDecodeError::InvalidEcdsaPrivateKeySecretScalar("scalar is zero"))
}

/// An error encoding a nada value to protobuf.
#[derive(Debug, thiserror::Error)]
pub enum ValueEncodeError {
    /// A nada value can't be converted into protobuf.
    #[error("type {0} can't be converted to protobuf")]
    UnsupportedType(NadaType),

    /// Array size is too large.
    #[error("array size is too large")]
    ArraySizeOverflow(usize),
}

/// An error decoding a nada value from protobuf.
#[derive(Debug, thiserror::Error)]
pub enum ValueDecodeError {
    /// No value was provided.
    #[error("no value provided")]
    NoValue,

    /// No type was provided.
    #[error("no type provided")]
    NoType,

    /// Array size is too large.
    #[error("array size is too large: {0}")]
    ArraySizeOverflow(u64),

    /// Invalid array.
    #[error("invalid array: {0}")]
    InvalidArray(&'static str),

    /// Invalid tuple.
    #[error("invalid tuple: {0}")]
    InvalidTuple(&'static str),

    /// Invalid ecdsa private key point.
    #[error("invalid ecdsa private key point: {0}")]
    InvalidEcdsaPrivateKeyPoint(&'static str),

    /// Invalid ecdsa private key secret scalar.
    #[error("invalid ecdsa private key secret scalar: {0}")]
    InvalidEcdsaPrivateKeySecretScalar(&'static str),

    /// Invalid ecdsa private key.
    #[error("invalid ecdsa private key: {0}")]
    InvalidEcdsaPrivateKey(String),

    /// ECDSA private key party index is too large.
    #[error("ecdsa private key party is too large: {0}")]
    EcdsaPrivateKeyPartyOverflow(u32),

    /// ECDSA signature scalar is invalid.
    #[error("ecdsa scalar {0} is invalid")]
    InvalidEcdsaSignatureScalar(&'static str),

    /// Invalid ECDSA message digest length.
    #[error("ecdsa message digest must be 32 bytes")]
    InvalidEcdsaMessageDigestLength,

    /// Invalid ECDSA public key length.
    #[error("ecdsa public key must be 33 bytes")]
    InvalidEcdsaPublicKeyLength,

    /// Invalid store id length.
    #[error("store id must be 16 bytes")]
    InvalidStoreIdLength,

    /// Duplicate named value.
    #[error("duplicate value: {0}")]
    DuplicateValue(String),
}

impl From<TypeError> for ValueDecodeError {
    fn from(e: TypeError) -> Self {
        match e {
            TypeError::HomogeneousVecOnly => Self::InvalidArray("arrays must only contain one type"),
            TypeError::MaxRecursionDepthExceeded => Self::InvalidArray("array nested depth is too large"),
            // These should not happen here so we fall back to some generic error.
            TypeError::NonEmptyVecOnly | TypeError::ZeroValue | TypeError::Unimplemented(_) => {
                Self::InvalidArray("unknown error")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{clear::Clear, encoders::EncodableWithP, encrypted::nada_values_clear_to_nada_values_encrypted};
    use basic_types::PartyId;
    use ecdsa_keypair::{privatekey::EcdsaPrivateKey, signature::EcdsaSignature};
    use math_lib::modular::U64SafePrime;
    use rand::thread_rng;
    use shamir_sharing::secret_sharer::ShamirSecretSharer;

    // This constructs a map that contains all nada values and serializes them.
    fn generate_nada_values() -> HashMap<String, NadaValue<Encrypted<Encoded>>> {
        let mut values = HashMap::new();
        values.insert(values.len().to_string(), NadaValue::new_integer(42));
        values.insert(values.len().to_string(), NadaValue::new_unsigned_integer(42u32));
        values.insert(values.len().to_string(), NadaValue::new_boolean(true));
        values.insert(values.len().to_string(), NadaValue::new_secret_integer(42));
        values.insert(values.len().to_string(), NadaValue::new_secret_unsigned_integer(42u32));
        values.insert(values.len().to_string(), NadaValue::new_secret_boolean(true));
        values.insert(values.len().to_string(), NadaValue::new_secret_blob(vec![1, 2, 3]));
        values.insert(
            values.len().to_string(),
            NadaValue::<Clear>::new_array_non_empty(vec![
                NadaValue::new_secret_integer(42),
                NadaValue::new_secret_integer(1337),
            ])
            .unwrap(),
        );
        values.insert(
            values.len().to_string(),
            NadaValue::<Clear>::new_tuple(NadaValue::new_secret_boolean(true), NadaValue::new_secret_integer(42))
                .unwrap(),
        );
        values.insert(
            values.len().to_string(),
            NadaValue::new_ecdsa_private_key(
                EcdsaPrivateKey::from_scalar(SecretScalar::<Secp256k1>::random(&mut rand::thread_rng())).unwrap(),
            ),
        );
        values.insert(values.len().to_string(), NadaValue::new_ecdsa_digest_message([42; 32]));
        values.insert(values.len().to_string(), NadaValue::new_ecdsa_public_key([42; 33]));
        values.insert(values.len().to_string(), NadaValue::new_store_id([42; 16]));
        values.insert(
            values.len().to_string(),
            NadaValue::new_ecdsa_signature(EcdsaSignature {
                r: NonZero::from_scalar(Scalar::random(&mut thread_rng())).unwrap(),
                s: NonZero::from_scalar(Scalar::random(&mut thread_rng())).unwrap(),
            }),
        );

        let parties = vec![PartyId::from(vec![1]), PartyId::from(vec![2]), PartyId::from(vec![3])];
        let sharer = ShamirSecretSharer::new(PartyId::from(vec![]), 1, parties).unwrap();
        let jar = nada_values_clear_to_nada_values_encrypted::<U64SafePrime>(values, &sharer).unwrap();

        jar.into_elements().next().unwrap().1.encode().unwrap()
    }

    #[test]
    fn encode_decode() {
        let values = generate_nada_values();
        let encoded = nada_values_to_protobuf(values.clone()).expect("encoding failed");
        let decoded = nada_values_from_protobuf(encoded, &EncodedModulo::U64SafePrime).expect("decoding failed");
        for (name, value) in values {
            let decoded = decoded.get(&name).expect(&format!("{name} not found"));
            assert_eq!(decoded, &value, "{name} differs");
        }
    }
}

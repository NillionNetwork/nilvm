//! encoding and decoding utilities

use crate::{
    encrypted::{BlobPrimitiveType, Encoded, Encrypted},
    errors::{DecodingError, EncodingError},
    NadaValue,
};
use basic_types::jar::PartyJar;
use math_lib::modular::{AsBits, Modular, SafePrime};
use nada_type::PrimitiveTypes;
use std::{collections::HashMap, marker::PhantomData};
use thiserror::Error;

// Those types are aliases for the EncodeVariableState type, that contains a method to encode or decode types.
// This method is used to encode or decode a variable into another variable.
// This operation is applied mainly over the primitive/scalar types, when the type is a compound type,
// for instance an Array. The process has to be applied to the deeper content (a compound type can
// contain another compound type) and then, when the content has been processed, the container will
// be processed.
// TODO remove nada value as last type arg
type EncodingNadaValueEncryptedState<'s, M> = EncodeVariableState<'s, M, Encrypted<M>, NadaValue<Encrypted<Encoded>>>;
type DecodingNadaValueEncryptedState<'s, M> = EncodeVariableState<'s, M, Encrypted<Encoded>, NadaValue<Encrypted<M>>>;

impl<T: SafePrime> Encoder for NadaValue<Encrypted<T>> {
    type Output = NadaValue<Encrypted<Encoded>>;

    fn encode<M>(&self) -> Result<Self::Output, EncodingError> {
        encode_or_decode::<T, _, Self::Output, EncodingError>(self)
    }
}

impl<P: SafePrime> EncodableWithP<P> for HashMap<String, NadaValue<Encrypted<P>>> {
    type Output = Result<HashMap<String, NadaValue<Encrypted<Encoded>>>, EncodingError>;

    #[allow(clippy::unwrap_used)]
    fn encode(&self) -> Self::Output {
        self.iter()
            .map(|(key, value)| (key.clone(), value.encode::<P>()))
            .map(|(id, value)| value.map(|value| (id, value)))
            .collect::<Result<HashMap<_, _>, _>>()
    }
}

impl<T: SafePrime> Decoder<NadaValue<Encrypted<T>>> for NadaValue<Encrypted<Encoded>> {
    fn decode<M>(&self) -> Result<NadaValue<Encrypted<T>>, DecodingError> {
        encode_or_decode::<T, _, NadaValue<Encrypted<T>>, DecodingError>(self)
    }
}

impl<'s, M: SafePrime> TryFrom<EncodingNadaValueEncryptedState<'s, M>> for NadaValue<Encrypted<Encoded>> {
    type Error = EncodingError;

    fn try_from(value: EncodingNadaValueEncryptedState<'s, M>) -> Result<Self, Self::Error> {
        use NadaValue::*;

        Ok(match value.variable_type {
            Integer(value) => Integer(value.encode()),
            UnsignedInteger(value) => UnsignedInteger(value.encode()),
            Boolean(value) => Boolean(value.encode()),
            EcdsaDigestMessage(value) => EcdsaDigestMessage(*value),
            ShamirShareInteger(value) => ShamirShareInteger(value.encode()),
            ShamirShareUnsignedInteger(value) => ShamirShareUnsignedInteger(value.encode()),
            ShamirShareBoolean(value) => ShamirShareBoolean(value.encode()),
            EcdsaPrivateKey(value) => EcdsaPrivateKey(value.clone()),
            EcdsaSignature(value) => EcdsaSignature(value.clone()),
            Array { inner_type, .. } => Self::new_array(inner_type.clone(), value.content)?,
            Tuple { .. } => {
                let mut values = value.content.into_iter();
                let left = values.next().ok_or(EncodingError::OutOfBounds)?;
                let right = values.next().ok_or(EncodingError::OutOfBounds)?;
                Self::new_tuple(left, right)?
            }
            NTuple { .. } => Self::new_n_tuple(value.content)?,
            Object { values } => {
                // This works because we use an IndexMap that keeps insertion order.
                Self::new_object(
                    values.keys().zip(value.content.into_iter()).map(|(key, content)| (key.clone(), content)).collect(),
                )?
            }
            SecretBlob(BlobPrimitiveType { value, unencoded_size }) => Self::new_secret_blob(BlobPrimitiveType::new(
                value.iter().map(|v| v.encode()).collect(),
                *unencoded_size,
            )),
            SecretBoolean(_) | SecretInteger(_) | SecretUnsignedInteger(_) => unreachable!(),
        })
    }
}

impl<'s, M: SafePrime> TryFrom<DecodingNadaValueEncryptedState<'s, M>> for NadaValue<Encrypted<M>> {
    type Error = DecodingError;

    fn try_from(value: DecodingNadaValueEncryptedState<'s, M>) -> Result<Self, Self::Error> {
        use NadaValue::*;

        Ok(match value.variable_type {
            Integer(value) => Integer(value.try_decode()?),
            UnsignedInteger(value) => UnsignedInteger(value.try_decode()?),
            Boolean(value) => Boolean(value.try_decode()?),
            EcdsaDigestMessage(value) => EcdsaDigestMessage(*value),
            ShamirShareInteger(value) => ShamirShareInteger(value.try_decode()?),
            ShamirShareUnsignedInteger(value) => ShamirShareUnsignedInteger(value.try_decode()?),
            ShamirShareBoolean(value) => ShamirShareBoolean(value.try_decode()?),

            EcdsaPrivateKey(value) => EcdsaPrivateKey(value.clone()),
            EcdsaSignature(value) => EcdsaSignature(value.clone()),
            Array { inner_type, .. } => Self::new_array(inner_type.clone(), value.content)?,
            Tuple { .. } => {
                let mut values = value.content.into_iter();
                let left = values.next().ok_or(DecodingError::OutOfBounds)?;
                let right = values.next().ok_or(DecodingError::OutOfBounds)?;
                Self::new_tuple(left, right)?
            }
            NTuple { .. } => Self::new_n_tuple(value.content)?,
            Object { values } => {
                // This works because we use an IndexMap that keeps insertion order.
                Self::new_object(
                    values.keys().zip(value.content.into_iter()).map(|(key, content)| (key.clone(), content)).collect(),
                )?
            }
            SecretBlob(BlobPrimitiveType { value, unencoded_size }) => Self::new_secret_blob(BlobPrimitiveType::new(
                value.iter().map(|v| v.try_decode()).collect::<Result<_, _>>()?,
                *unencoded_size,
            )),
            SecretInteger(_) | SecretUnsignedInteger(_) | SecretBoolean(_) => unreachable!(),
        })
    }
}

impl<'s, M: SafePrime> From<&'s NadaValue<Encrypted<M>>> for EncodingNadaValueEncryptedState<'s, M> {
    fn from(variable_type: &'s NadaValue<Encrypted<M>>) -> Self {
        Self { variable_type, content: vec![], _modular: PhantomData }
    }
}

impl<'s, M: SafePrime> From<&'s NadaValue<Encrypted<Encoded>>> for DecodingNadaValueEncryptedState<'s, M> {
    fn from(variable_type: &'s NadaValue<Encrypted<Encoded>>) -> Self {
        Self { variable_type, content: vec![], _modular: PhantomData }
    }
}

impl<T: SafePrime> Encoder for PartyJar<NadaValue<Encrypted<T>>> {
    type Output = PartyJar<NadaValue<Encrypted<Encoded>>>;

    #[allow(clippy::unwrap_used)]
    fn encode<M>(&self) -> Result<Self::Output, EncodingError> {
        let mut result = PartyJar::new(self.stored_party_count());
        for (id, value) in self.elements() {
            // Note: we unwrap here because we are going from one Jar to another, so we know there can't be any duplicates.
            // Note: this is not optimal because we are doing a binary_search_by for every element, but our elements are already unique.
            result.add_element(id.clone(), value.encode::<T>()?).unwrap();
        }
        Ok(result)
    }
}

impl<T: SafePrime> Decoder<HashMap<String, NadaValue<Encrypted<T>>>>
    for HashMap<String, NadaValue<Encrypted<Encoded>>>
{
    fn decode<P>(&self) -> Result<HashMap<String, NadaValue<Encrypted<T>>>, DecodingError> {
        let mut decoded_values = HashMap::new();
        for (key, value) in self {
            decoded_values.insert(key.clone(), value.decode::<T>()?);
        }
        Ok(decoded_values)
    }
}

/// This method is used to encode or decode a variable into another variable.
/// This operation is applied mainly over the primitive/scalar types, when the type is a compound type,
/// for instance an Array. The process has to be applied to the deeper content (a compound type can
/// contain another compound type) and then, when the content has been processed, the container will
/// be processed.
pub(crate) fn encode_or_decode<'s, M: Modular, T: PrimitiveTypes + 's, Content, E>(
    variable_type: &'s NadaValue<T>,
) -> Result<Content, E>
where
    EncodeVariableState<'s, M, T, Content>: From<&'s NadaValue<T>>,
    Content: TryFrom<EncodeVariableState<'s, M, T, Content>, Error = E>,
    E: From<EncodeVariableError>,
{
    // We use an stack of states to keep the states of the different elements that are being processed.
    let mut states: Vec<EncodeVariableState<'s, M, T, Content>> = vec![variable_type.into()];
    let mut result = None;
    while let Some(state) = states.pop() {
        if let Some(next) = state.next() {
            // The current state is a compound type and it contains elements that haven't been processed.
            // We push back the state into the stack and at the top the first contained element that
            // has not yet been processed.
            states.push(state);
            states.push(next.into());
        } else if let Some(mut container) = states.pop() {
            // The current state doesn't contain more elements to process, but it is contained by other variable.
            // We will process it and push it into the container.
            container.try_push_content(Content::try_from(state)?)?;
            states.push(container);
        } else {
            // The current state is encoded and it doesn't have a container. It will be the result.
            result = Some(Content::try_from(state)?);
        }
    }
    // TODO Maybe we should have any validation here to check if the result match the type.
    Ok(result.ok_or(EncodeVariableError::MissingData)?)
}

pub(crate) struct EncodeVariableState<'s, M: Modular, T: PrimitiveTypes, C> {
    pub(crate) variable_type: &'s NadaValue<T>,
    pub(crate) content: Vec<C>,
    pub(crate) _modular: PhantomData<M>,
}

impl<'s, M: Modular, T: PrimitiveTypes, C> EncodeVariableState<'s, M, T, C> {
    pub fn next(&self) -> Option<&'s NadaValue<T>> {
        use NadaValue::*;

        match self.variable_type {
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
            | EcdsaSignature(_) => None,
            Tuple { left, right } => {
                if self.content.is_empty() {
                    Some(left.as_ref())
                } else if self.content.len() == 1 {
                    Some(right.as_ref())
                } else {
                    None
                }
            }
            Array { values, .. } => values.get(self.content.len()),
            NTuple { values } => values.get(self.content.len()),
            Object { values } => {
                // This works because we use an IndexMap that keeps insertion order.
                values.get_index(self.content.len()).map(|(_, value)| value)
            }
        }
    }

    pub fn try_push_content(&mut self, content: C) -> Result<(), EncodeVariableError> {
        use NadaValue::*;

        match self.variable_type {
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
            | EcdsaSignature(_) => {
                Err(EncodeVariableError::UnsupportedContainment(format!("{:?}", self.variable_type.to_type_kind())))
            }
            Array { .. } => {
                self.content.push(content);
                Ok(())
            }
            Tuple { .. } => {
                self.content.push(content);
                Ok(())
            }
            NTuple { .. } => {
                self.content.push(content);
                Ok(())
            }
            Object { .. } => {
                self.content.push(content);
                Ok(())
            }
        }
    }
}

/// Errors that occur during a variable transformation
#[derive(Error, Debug)]
pub enum EncodeVariableError {
    /// This error occurs when data is missing in the source variable and the variable transformation
    /// can not obtain a transformation result
    #[error("the variable could not be transformed, data was missing")]
    MissingData,

    /// This error occurs when trying to add an element as content of a non-composite type
    #[error("{0} can not contain elements")]
    UnsupportedContainment(String),
}

/// This trait provides the encoding behaviour
pub trait Encoder {
    /// Encoding output type.
    type Output;

    /// This function is used to encode an item.
    fn encode<T: Modular>(&self) -> Result<Self::Output, EncodingError>;
}

/// Workaround missing P in Encodable trait.
/// TODO: remove
pub trait EncodableWithP<P: SafePrime> {
    /// Encoding output.
    type Output;

    /// Encode this value.
    fn encode(&self) -> Self::Output;
}

/// This trait provides the decoding behaviour
pub trait Decoder<O> {
    /// This function is used to decode an item.
    fn decode<T: Modular>(&self) -> Result<O, DecodingError>;
}

/// Calculates the size of a blob chunk base on the [Modular] size.
pub fn blob_chunk_size<T: Modular>() -> usize {
    let prime_bytes = T::MODULO.bits().div_ceil(8);
    prime_bytes.wrapping_sub(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        encoders::{Decoder, Encoder},
        encrypted::{Encoded, Encrypted},
        NadaValue,
    };
    use anyhow::Error;
    use math_lib::modular::{ModularNumber, SafePrime, U128SafePrime, U256SafePrime, U64SafePrime};
    use rstest::rstest;

    #[rstest]
    #[case(U64SafePrime, 7)]
    #[case(U128SafePrime, 15)]
    #[case(U256SafePrime, 31)]
    fn test_blob_chunk_size<T: Modular>(#[case] _prime: T, #[case] expected: usize) {
        assert_eq!(blob_chunk_size::<T>(), expected);
    }

    fn encode_encrypted_value_test<T: SafePrime>(value: NadaValue<Encrypted<T>>) -> Result<(), Error> {
        let encoded: NadaValue<Encrypted<Encoded>> = value.encode::<T>()?;
        assert_eq!(value, encoded.decode::<T>()?);
        Ok(())
    }

    #[rstest]
    #[case(115058, U64SafePrime)]
    #[case(115058, U128SafePrime)]
    #[case(115058, U256SafePrime)]
    fn encode_integer<T: SafePrime>(#[case] value: u64, #[case] _p: T) -> Result<(), Error> {
        encode_encrypted_value_test::<T>(NadaValue::new_integer(ModularNumber::from_u64(value)))
    }

    #[rstest]
    #[case(115058, U64SafePrime)]
    #[case(115058, U128SafePrime)]
    #[case(115058, U256SafePrime)]
    fn encode_shamir_share_integer<T: SafePrime>(#[case] value: u64, #[case] _p: T) -> Result<(), Error> {
        encode_encrypted_value_test::<T>(NadaValue::new_shamir_share_integer(ModularNumber::from_u64(value)))
    }

    #[rstest]
    #[case(115058, U64SafePrime)]
    #[case(115058, U128SafePrime)]
    #[case(115058, U256SafePrime)]
    fn encode_unsigned_integer<T: SafePrime>(#[case] value: u64, #[case] _p: T) -> Result<(), Error> {
        encode_encrypted_value_test::<T>(NadaValue::new_unsigned_integer(ModularNumber::from_u64(value)))
    }

    #[rstest]
    #[case(115058, U64SafePrime)]
    #[case(115058, U128SafePrime)]
    #[case(115058, U256SafePrime)]
    fn encode_shamir_share_unsigned_integer<T: SafePrime>(#[case] value: u64, #[case] _p: T) -> Result<(), Error> {
        encode_encrypted_value_test::<T>(NadaValue::new_shamir_share_unsigned_integer(ModularNumber::from_u64(value)))
    }

    #[rstest]
    #[case(false, U64SafePrime)]
    #[case(false, U128SafePrime)]
    #[case(false, U256SafePrime)]
    #[case(true, U64SafePrime)]
    #[case(true, U128SafePrime)]
    #[case(true, U256SafePrime)]
    fn encode_boolean<T: SafePrime>(#[case] value: bool, #[case] _p: T) -> Result<(), Error> {
        encode_encrypted_value_test::<T>(NadaValue::new_integer(ModularNumber::from_u64(value as u64)))
    }

    #[rstest]
    #[case(false, U64SafePrime)]
    #[case(false, U128SafePrime)]
    #[case(false, U256SafePrime)]
    #[case(true, U64SafePrime)]
    #[case(true, U128SafePrime)]
    #[case(true, U256SafePrime)]
    fn encode_shamir_share_boolean<T: SafePrime>(#[case] value: bool, #[case] _p: T) -> Result<(), Error> {
        encode_encrypted_value_test::<T>(NadaValue::new_shamir_share_boolean(ModularNumber::from_u64(value as u64)))
    }

    #[rstest]
    #[case(115058, U64SafePrime)]
    #[case(115058, U128SafePrime)]
    #[case(115058, U256SafePrime)]
    fn encode_secret_blob<T: SafePrime>(#[case] value: u64, #[case] _p: T) -> Result<(), Error> {
        let blob_value = BlobPrimitiveType { value: vec![ModularNumber::from_u64(value)], unencoded_size: 1 };
        encode_encrypted_value_test::<T>(NadaValue::new_secret_blob(blob_value))
    }

    #[rstest]
    #[case(U64SafePrime)]
    #[case(U128SafePrime)]
    #[case(U256SafePrime)]
    fn encode_ecdsa_digest_message<T: SafePrime>(#[case] _p: T) -> Result<(), Error> {
        let digest_value = [
            1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29,
            30, 31, 32,
        ];
        encode_encrypted_value_test::<T>(NadaValue::new_ecdsa_digest_message(digest_value))
    }

    #[rstest]
    #[case(vec![115058, 7, 549513185], U64SafePrime)]
    #[case(vec![115058, 7, 549513185], U128SafePrime)]
    #[case(vec![115058, 7, 549513185], U256SafePrime)]
    fn encode_array<T: SafePrime>(#[case] values: Vec<u64>, #[case] _p: T) -> Result<(), Error> {
        let inner_values: Vec<NadaValue<Encrypted<T>>> = values
            .into_iter()
            .map(|value| NadaValue::new_shamir_share_integer(ModularNumber::from_u64(value)))
            .collect();
        encode_encrypted_value_test::<T>(NadaValue::new_array_non_empty(inner_values)?)
    }

    #[rstest]
    #[case(115058, 7, U64SafePrime)]
    #[case(115058, 7, U128SafePrime)]
    #[case(115058, 7, U256SafePrime)]
    fn encode_tuple<T: SafePrime>(#[case] left: u64, #[case] right: u64, #[case] _p: T) -> Result<(), Error> {
        let left: NadaValue<Encrypted<T>> = NadaValue::new_shamir_share_integer(ModularNumber::from_u64(left));
        let right: NadaValue<Encrypted<T>> = NadaValue::new_shamir_share_integer(ModularNumber::from_u64(right));
        encode_encrypted_value_test::<T>(NadaValue::new_tuple(left, right)?)
    }

    #[rstest]
    #[case(vec![115058, 7, 549513185], U64SafePrime)]
    #[case(vec![115058, 7, 549513185], U128SafePrime)]
    #[case(vec![115058, 7, 549513185], U256SafePrime)]
    fn encode_ntuple<T: SafePrime>(#[case] values: Vec<u64>, #[case] _p: T) -> Result<(), Error> {
        let inner_values: Vec<NadaValue<Encrypted<T>>> = values
            .into_iter()
            .map(|value| NadaValue::new_shamir_share_integer(ModularNumber::from_u64(value)))
            .collect();
        encode_encrypted_value_test::<T>(NadaValue::new_n_tuple(inner_values)?)
    }

    #[rstest]
    #[case(vec![("a".to_string(), 115058), ("b".to_string(), 7), ("c".to_string(), 549513185)], U64SafePrime)]
    #[case(vec![("a".to_string(), 115058), ("b".to_string(), 7), ("c".to_string(), 549513185)], U128SafePrime)]
    #[case(vec![("a".to_string(), 115058), ("b".to_string(), 7), ("c".to_string(), 549513185)], U256SafePrime)]
    fn encode_object<T: SafePrime>(#[case] values: Vec<(String, u64)>, #[case] _p: T) -> Result<(), Error> {
        use indexmap::IndexMap;

        let inner_values: IndexMap<String, NadaValue<Encrypted<T>>> = values
            .into_iter()
            .map(|(key, value)| (key, NadaValue::new_shamir_share_integer(ModularNumber::from_u64(value))))
            .collect();
        encode_encrypted_value_test::<T>(NadaValue::new_object(inner_values)?)
    }

    #[rstest]
    #[case::encode_blob_4_u64(4, U64SafePrime)]
    #[case::encode_blob_8_u64(8, U64SafePrime)]
    #[case::encode_blob_0_u256(0, U256SafePrime)]
    #[case::encode_blob_4_u256(4, U256SafePrime)]
    #[case::encode_blob_10_u256(10, U256SafePrime)]
    #[case::encode_blob_31_u256(31, U256SafePrime)]
    #[case::encode_blob_32_u256(32, U256SafePrime)]
    #[case::encode_blob_62_u256(62, U256SafePrime)]
    #[case::encode_blob_64_u256(64, U256SafePrime)]
    #[case::encode_blob_100_u256(100, U256SafePrime)]
    #[case::encode_blob_256_u256(256, U256SafePrime)]
    #[case::encode_blob_4000_u256(4000, U256SafePrime)]
    fn test_blob_encoding<T: SafePrime>(#[case] secret_size: u64, #[case] _prime: T) {
        let secret = BlobPrimitiveType::new(
            (1..=secret_size).map(|value| ModularNumber::from_u64(value)).collect(),
            secret_size,
        );
        let secret: NadaValue<Encrypted<T>> = NadaValue::new_secret_blob(secret);
        let encoded = secret.encode::<T>().expect("encoding failed");
        let decoded = encoded.decode::<T>().expect("decoding failed");
        assert_eq!(decoded, secret);
    }

    #[rstest]
    #[case::single_zero(vec![0])]
    #[case::leading_zero(vec![0, 1])]
    #[case::trailing_zero(vec![1, 0])]
    #[case::leading_trailing(vec![0, 0, 0, 0, 0, 115, 116, 32, 101, 109, 0, 0, 0, 0, 0])]
    #[case::single_chunk_zeroes(vec![0; 7])]
    #[case::two_chunks_zeroes(vec![0; 8])]
    #[case::two_full_zero_chunks(vec![0; 14])]
    #[case::one_chunk_just_zeroes(vec![0, 0, 0, 0, 0, 0, 0, 1])]
    fn test_blob_edge_cases(#[case] value: Vec<u8>) {
        let secret = BlobPrimitiveType::new(
            value.iter().map(|value| ModularNumber::from_u64(*value as u64)).collect(),
            value.len() as u64,
        );
        let secret: NadaValue<Encrypted<U64SafePrime>> = NadaValue::new_secret_blob(secret);
        let encoded = secret.encode::<U64SafePrime>().expect("encoding failed");
        let decoded = encoded.decode::<U64SafePrime>().expect("decoding failed");
        assert_eq!(decoded, secret);
    }
}

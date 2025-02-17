//! Parsing utilities.

use crate::named::Named;
use anyhow::anyhow;
use base64::{prelude::BASE64_STANDARD, Engine};
use nada_value::{clear::Clear, BigInt, BigUint, NadaType, NadaValue};
use std::str::FromStr;

/// Allows a type to act as a parser for another type.
pub(crate) trait Parse {
    type Output;

    /// Parse a single element out of a string.
    fn parse(&self, input: &str) -> anyhow::Result<Self::Output> {
        let (name, value) = split_name_value(input)?;
        self.parse_named(name, value)
    }

    fn parse_named(&self, name: &str, value: &str) -> anyhow::Result<Self::Output>;

    /// Parse a sequence of elements out of a sequence of strings.
    fn parse_all<I, O>(&self, inputs: I) -> anyhow::Result<Vec<Self::Output>>
    where
        I: IntoIterator<Item = O>,
        O: AsRef<str>,
    {
        inputs.into_iter().map(|s| self.parse(s.as_ref())).collect()
    }

    /// Parse a sequence of elements out of a sequence of named strings.
    fn parse_named_all<I, S, O>(&self, inputs: I) -> anyhow::Result<Vec<Self::Output>>
    where
        I: IntoIterator<Item = (S, O)>,
        S: AsRef<str>,
        O: AsRef<str>,
    {
        inputs.into_iter().map(|(name, value)| self.parse_named(name.as_ref(), value.as_ref())).collect()
    }
}

pub(crate) fn value_from_str<T>(value: &str) -> anyhow::Result<T>
where
    T: FromStr,
    T::Err: std::error::Error,
{
    T::from_str(value).map_err(|e| anyhow!("failed parsing secret '{value}': {e}"))
}

impl Parse for NadaType {
    type Output = Named<NadaValue<Clear>>;

    fn parse_named(&self, name: &str, value: &str) -> anyhow::Result<Self::Output> {
        use NadaType::*;

        let value = match &self {
            Integer => NadaValue::new_integer(value_from_str::<BigInt>(value)?),
            UnsignedInteger => NadaValue::new_unsigned_integer(value_from_str::<BigUint>(value)?),
            SecretInteger => NadaValue::new_secret_integer(value_from_str::<BigInt>(value)?),
            SecretUnsignedInteger => NadaValue::new_secret_unsigned_integer(value_from_str::<BigUint>(value)?),
            Array { inner_type, .. } => parse_array(inner_type.as_ref(), value)?,
            SecretBlob => {
                let value = BASE64_STANDARD.decode(value).map_err(|e| anyhow!("invalid base64 blob: {e}"))?;
                NadaValue::new_secret_blob(value)
            }
            EcdsaDigestMessage => {
                let value = BASE64_STANDARD.decode(value).map_err(|e| anyhow!("invalid base64 message digest: {e}"))?;
                let value: [u8; 32] = value.try_into().map_err(|_| anyhow!("message digest must be 32 bytes long"))?;
                NadaValue::new_ecdsa_digest_message(value)
            }
            EddsaMessage => {
                let value = BASE64_STANDARD.decode(value).map_err(|e| anyhow!("invalid base64 message: {e}"))?;
                NadaValue::new_eddsa_message(value)
            }
            EddsaPublicKey => {
                let value = BASE64_STANDARD.decode(value).map_err(|e| anyhow!("invalid base64 public key: {e}"))?;
                let value: [u8; 32] = value.try_into().map_err(|_| anyhow!("public key must be 32 bytes long"))?;
                NadaValue::new_eddsa_public_key(value)
            }
            Boolean
            | Tuple { .. }
            | NTuple { .. }
            | Object { .. }
            | SecretBoolean
            | ShamirShareInteger
            | ShamirShareUnsignedInteger
            | ShamirShareBoolean
            | EcdsaPrivateKey
            | EcdsaPublicKey
            | StoreId
            | EcdsaSignature
            | EddsaPrivateKey
            | EddsaSignature => Err(anyhow!("{} value", value))?,
        };
        Ok(Named { name: name.to_string(), value })
    }
}

/// Utility function that parses an array.
///
/// It expects a string with comma-separated values.
fn parse_array(inner_type: &NadaType, value: &str) -> anyhow::Result<NadaValue<Clear>> {
    use NadaType::*;
    let mut values = vec![];
    for element in value.split(',') {
        let element_value = match inner_type {
            Integer => NadaValue::new_integer(value_from_str::<BigInt>(element)?),
            UnsignedInteger => NadaValue::new_unsigned_integer(value_from_str::<BigUint>(element)?),
            SecretInteger => NadaValue::new_secret_integer(value_from_str::<BigInt>(element)?),
            SecretUnsignedInteger => NadaValue::new_secret_unsigned_integer(value_from_str::<BigUint>(element)?),

            _ => Err(anyhow!("{} secret", element))?,
        };
        values.push(element_value);
    }

    anyhow::Ok(NadaValue::Array { inner_type: inner_type.clone(), values })
}

fn split_name_value(input: &str) -> anyhow::Result<(&str, &str)> {
    let split_public_variable = input.split_once('=').ok_or_else(|| anyhow!("inputs must have name=value format"))?;
    Ok(split_public_variable)
}

#[cfg(test)]
mod test {
    use super::Parse;
    use crate::named::Named;
    use nada_value::{clear::Clear, NadaType, NadaValue};
    use rstest::rstest;

    fn new_integer(value: i32) -> NadaValue<Clear> {
        NadaValue::new_integer(value)
    }

    fn new_unsigned_integer(value: u32) -> NadaValue<Clear> {
        NadaValue::new_unsigned_integer(value)
    }

    fn new_secret_integer(value: i32) -> NadaValue<Clear> {
        NadaValue::new_secret_integer(value)
    }

    fn new_secret_unsigned_integer(value: u32) -> NadaValue<Clear> {
        NadaValue::new_secret_unsigned_integer(value)
    }

    #[rstest]
    #[case(NadaType::Integer, "42", NadaValue::new_integer(42))]
    #[case(NadaType::UnsignedInteger, "42", NadaValue::new_unsigned_integer(42u32))]
    #[case(NadaType::SecretInteger, "42", NadaValue::new_secret_integer(42))]
    #[case(NadaType::SecretUnsignedInteger, "13", NadaValue::new_secret_unsigned_integer(13u32))]
    #[case(NadaType::SecretBlob, "cG90YXRv", NadaValue::new_secret_blob("potato".as_bytes().to_vec()))]
    fn parse_secret(#[case] nada_type: NadaType, #[case] string_repr: &str, #[case] expected: NadaValue<Clear>) {
        let string_repr = format!("value={string_repr}");
        let value: Named<NadaValue<Clear>> = nada_type.parse(&string_repr).expect("parsing failed");
        assert_eq!(value.name, "value");
        assert_eq!(value.value, expected);
    }

    #[rstest]
    #[case(NadaType::Integer, "1,2,3", vec![new_integer(1), new_integer(2), new_integer(3)])]
    #[case(NadaType::UnsignedInteger, "4,2,1", vec![new_unsigned_integer(4), new_unsigned_integer(2), new_unsigned_integer(1)],)]
    #[case(NadaType::SecretInteger, "1,2,3", vec![new_secret_integer(1), new_secret_integer(2), new_secret_integer(3)])]
    #[case(NadaType::SecretUnsignedInteger, "4,2,1", vec![new_secret_unsigned_integer(4), new_secret_unsigned_integer(2), new_secret_unsigned_integer(1)],)]
    fn parse_secret_array(
        #[case] inner_type: NadaType,
        #[case] string_repr: &str,
        #[case] expected: Vec<NadaValue<Clear>>,
    ) {
        let expected_array = NadaValue::Array { inner_type: inner_type.clone(), values: expected };

        let string_repr = format!("value={string_repr}");
        let value: Named<NadaValue<Clear>> =
            NadaType::Array { inner_type: Box::new(inner_type), size: 0 }.parse(&string_repr).expect("parsing failed");
        assert_eq!(value.name, "value");
        assert_eq!(value.value, expected_array);
    }
}

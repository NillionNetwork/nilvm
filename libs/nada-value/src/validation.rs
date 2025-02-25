//! This crate implements utilities to validate that a value's encoding matches with the program input types

use crate::NadaValue;
use nada_type::{NadaType, PrimitiveTypes};
use std::collections::HashMap;

fn check_encrypted_type(expected: &NadaType, found: &NadaType) -> Result<(), EncryptedValueValidationError> {
    let mut inner_types = vec![(expected, found)];
    while let Some((expected, found)) = inner_types.pop() {
        match (expected, found) {
            (NadaType::Integer, NadaType::Integer)
            | (NadaType::UnsignedInteger, NadaType::UnsignedInteger)
            | (NadaType::Boolean, NadaType::Boolean)
            | (NadaType::SecretInteger, NadaType::ShamirShareInteger)
            | (NadaType::SecretUnsignedInteger, NadaType::ShamirShareUnsignedInteger)
            | (NadaType::SecretBoolean, NadaType::ShamirShareBoolean)
            | (NadaType::SecretBlob, NadaType::SecretBlob)
            | (NadaType::EcdsaPrivateKey, NadaType::EcdsaPrivateKey)
            | (NadaType::EcdsaDigestMessage, NadaType::EcdsaDigestMessage)
            | (NadaType::EcdsaSignature, NadaType::EcdsaSignature)
            | (NadaType::EcdsaPublicKey, NadaType::EcdsaPublicKey)
            | (NadaType::EddsaPrivateKey, NadaType::EddsaPrivateKey)
            | (NadaType::EddsaMessage, NadaType::EddsaMessage)
            | (NadaType::EddsaSignature, NadaType::EddsaSignature)
            | (NadaType::EddsaPublicKey, NadaType::EddsaPublicKey)
            | (NadaType::StoreId, NadaType::StoreId) => {}
            (
                NadaType::Array { inner_type: expected_inner_type, .. },
                NadaType::Array { inner_type: found_inner_type, .. },
            ) => {
                inner_types.push((expected_inner_type, found_inner_type));
            }
            (
                NadaType::Tuple { left_type: expected_left, right_type: expected_right },
                NadaType::Tuple { left_type: found_left, right_type: found_right },
            ) => {
                inner_types.push((expected_left, found_left));
                inner_types.push((expected_right, found_right));
            }
            (expected, found) => {
                return Err(EncryptedValueValidationError::NoMatch(expected.to_string(), found.to_string()));
            }
        }
    }
    Ok(())
}

/// Validate the encrypted values match with the expected value types.
pub fn validate_encrypted_values<T: PrimitiveTypes>(
    values: &HashMap<String, NadaValue<T>>,
    requested_inputs: &HashMap<String, NadaType>,
) -> Result<(), EncryptedValueValidationError> {
    let mut unexpected_values = Vec::new();
    let mut missing_values = Vec::new();
    for (input, input_ty) in requested_inputs {
        if let Some(value) = values.get(input) {
            check_encrypted_type(input_ty, &value.to_type())?
        } else {
            missing_values.push(input.clone());
        }
    }
    for input in values.keys() {
        if requested_inputs.get(input).is_none() {
            unexpected_values.push(input.clone())
        }
    }
    if !missing_values.is_empty() {
        return Err(EncryptedValueValidationError::MissingValues(missing_values));
    }
    if !unexpected_values.is_empty() {
        return Err(EncryptedValueValidationError::UnexpectedValues(unexpected_values));
    }
    Ok(())
}

/// An error returned by the secret validation.
#[derive(Debug, PartialEq, thiserror::Error)]
pub enum EncryptedValueValidationError {
    /// Input not found into the program inputs
    #[error("unexpected values found: {0:?}")]
    UnexpectedValues(Vec<String>),

    /// Program inputs are missing
    #[error("missing values: {0:?}")]
    MissingValues(Vec<String>),

    /// Secret encoding type doesn't match with input's type
    #[error("types don't match: {0} != {1}")]
    NoMatch(String, String),
}

#[cfg(test)]
mod tests {
    use super::{validate_encrypted_values, EncryptedValueValidationError};
    use crate::{clear::Clear, validation::check_encrypted_type, NadaType, NadaValue};
    use anyhow::Result;
    use rstest::rstest;

    #[test]
    fn secret_integer() -> Result<()> {
        check_encrypted_type(&NadaType::SecretInteger, &NadaType::ShamirShareInteger)?;
        Ok(())
    }

    #[test]
    fn secret_unsigned_integer() -> Result<()> {
        check_encrypted_type(&NadaType::SecretUnsignedInteger, &NadaType::ShamirShareUnsignedInteger)?;
        Ok(())
    }

    #[rstest]
    #[case::missing(&["A"], &["A", "B"], EncryptedValueValidationError::MissingValues(vec!["B".to_string()]))]
    #[case::extra(&["A", "C"], &["A"], EncryptedValueValidationError::UnexpectedValues(vec!["C".to_string()]))]
    fn invalid_values(
        #[case] inputs: &[&str],
        #[case] required: &[&str],
        #[case] error: EncryptedValueValidationError,
    ) {
        let inputs = inputs.iter().map(|name| (name.to_string(), NadaValue::new_integer(42))).collect();
        let required = required.iter().map(|name| (name.to_string(), NadaType::Integer)).collect();
        let found_error = validate_encrypted_values::<Clear>(&inputs, &required).expect_err("not an error");
        assert_eq!(found_error, error);
    }
}

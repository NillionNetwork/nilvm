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
            | (NadaType::EcdsaSignature, NadaType::EcdsaSignature) => {}
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
    for (input, value) in values.iter() {
        if let Some(input_ty) = requested_inputs.get(input) {
            check_encrypted_type(input_ty, &value.to_type())?
        } else {
            unexpected_values.push(input.clone())
        }
    }
    if !unexpected_values.is_empty() {
        return Err(EncryptedValueValidationError::UnexpectedValues(unexpected_values));
    }
    Ok(())
}

/// An error returned by the secret validation.
#[derive(Debug, thiserror::Error)]
pub enum EncryptedValueValidationError {
    /// Input not found into the program inputs
    #[error("unexpected values found: {0:?}")]
    UnexpectedValues(Vec<String>),

    /// Secret encoding type doesn't match with input's type
    #[error("types don't match: {0} != {1}")]
    NoMatch(String, String),
}

#[cfg(test)]
mod tests {
    use anyhow::Result;

    use crate::NadaType;

    use crate::validation::check_encrypted_type;

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
}

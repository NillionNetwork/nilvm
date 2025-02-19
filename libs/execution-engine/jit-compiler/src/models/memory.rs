//! This crate implements common entities for the different memory levels of the jit-compiler

use nada_type::NadaType;
use std::fmt::{Display, Formatter};

/// Indicates the referenced memory by the address
#[derive(Clone, Debug, Copy, Default, PartialEq, PartialOrd, Ord, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum AddressType {
    /// The address refers to the input memory
    Input,
    /// The address refers to the output memory
    Output,
    /// The address refers to the heap memory
    #[default]
    Heap,
    /// The address refers to the literals memory
    Literals,
}

impl Display for AddressType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let addr_name = match self {
            AddressType::Input => "iaddr",
            AddressType::Output => "oaddr",
            AddressType::Heap => "addr",
            AddressType::Literals => "laddr",
        };
        write!(f, "{addr_name}")
    }
}

/// These errors are thrown during the calculation of address count for a type.
#[derive(Debug, thiserror::Error)]
pub enum AddressCountError {
    /// This error is throw when the size of the object is greater than we are allowing
    #[error("memory overflow")]
    MemoryOverflow,
}

/// Calculates the number of addresses of the [`NadaType`] in memory
///
/// This is the number of "addresses" that are required to store the type in the Bytecode Memory registry.
/// - For scalar types, size is 1
/// - For compound types, it calculates the total "size" by recursively invoking [`crate::models::bytecode::address_count`] for each element and combining the returned values. And adds 1 to account for the `New` instruction
pub fn address_count(ty: &NadaType) -> Result<usize, AddressCountError> {
    let mut address_count = 0usize;
    let mut inner_types = vec![(ty, 1)];
    while let Some((inner_type, multiplier)) = inner_types.pop() {
        address_count = address_count.checked_add(multiplier).ok_or(AddressCountError::MemoryOverflow)?;
        match inner_type {
            NadaType::Integer
            | NadaType::UnsignedInteger
            | NadaType::Boolean
            | NadaType::EcdsaDigestMessage
            | NadaType::EcdsaPublicKey
            | NadaType::StoreId
            | NadaType::SecretInteger
            | NadaType::SecretUnsignedInteger
            | NadaType::SecretBoolean
            | NadaType::SecretBlob
            | NadaType::ShamirShareInteger
            | NadaType::ShamirShareUnsignedInteger
            | NadaType::ShamirShareBoolean
            | NadaType::EcdsaPrivateKey
            | NadaType::EcdsaSignature
            | NadaType::EddsaPrivateKey
            | NadaType::EddsaPublicKey
            | NadaType::EddsaSignature
            | NadaType::EddsaMessage => {}
            NadaType::Array { size, inner_type } => {
                let multiplier = multiplier.checked_mul(*size).ok_or(AddressCountError::MemoryOverflow)?;
                inner_types.push((inner_type, multiplier));
            }
            NadaType::Tuple { left_type, right_type } => {
                inner_types.push((left_type, multiplier));
                inner_types.push((right_type, multiplier));
            }
            NadaType::NTuple { types } => {
                for inner_type in types {
                    inner_types.push((inner_type, multiplier));
                }
            }
            NadaType::Object { types } => {
                for inner_type in types.0.values() {
                    inner_types.push((inner_type, multiplier));
                }
            }
        }
    }
    Ok(address_count)
}

/// Returns the number of addresses that are required to represent this [`NadaType`] that is
/// calculated in runtime.
/// The only difference between result elements and inputs is that the inner elements that a compound
/// value contains are represented as a pointer instead of a full value.
pub fn result_element_address_count(ty: &NadaType) -> usize {
    match ty {
        NadaType::Integer
        | NadaType::UnsignedInteger
        | NadaType::Boolean
        | NadaType::EcdsaDigestMessage
        | NadaType::EcdsaPublicKey
        | NadaType::StoreId
        | NadaType::SecretInteger
        | NadaType::SecretUnsignedInteger
        | NadaType::SecretBoolean
        | NadaType::SecretBlob
        | NadaType::ShamirShareInteger
        | NadaType::ShamirShareUnsignedInteger
        | NadaType::ShamirShareBoolean
        | NadaType::EcdsaPrivateKey
        | NadaType::EcdsaSignature
        | NadaType::EddsaPrivateKey
        | NadaType::EddsaPublicKey
        | NadaType::EddsaSignature
        | NadaType::EddsaMessage => 1,
        // The inner elements for the compound types that are calculated in runtime are
        // represented as pointers. This means we do not need to traverse the type in depth.
        NadaType::Array { size, .. } => (*size).wrapping_add(1),
        NadaType::Tuple { .. } => 3,
        NadaType::NTuple { types } => types.len().wrapping_add(1),
        NadaType::Object { types } => types.len().wrapping_add(1),
    }
}

#[cfg(test)]
mod tests {
    use crate::models::memory::{address_count, result_element_address_count};
    use nada_type::NadaType;
    use rstest::rstest;

    #[rstest]
    #[case(NadaType::Integer, 1)]
    #[case(NadaType::UnsignedInteger, 1)]
    #[case(NadaType::Boolean, 1)]
    #[case(NadaType::SecretInteger, 1)]
    #[case(NadaType::SecretUnsignedInteger, 1)]
    #[case(NadaType::SecretBoolean, 1)]
    #[case(NadaType::ShamirShareInteger, 1)]
    #[case(NadaType::ShamirShareUnsignedInteger, 1)]
    #[case(NadaType::ShamirShareBoolean, 1)]
    #[case(NadaType::Array { inner_type: Box::new(NadaType::ShamirShareInteger), size: 5 }, 6)]
    #[case(NadaType::Array {
        inner_type: Box::new(NadaType::Array { inner_type: Box::new(NadaType::ShamirShareInteger), size: 5 })
        , size: 5
    }, 31)]
    #[case(NadaType::Tuple {
    left_type: Box::new(NadaType::ShamirShareInteger),
    right_type: Box::new(NadaType::ShamirShareInteger)
    }, 3)]
    fn test_input_address_count(#[case] ty: NadaType, #[case] expected_address_count: usize) {
        assert_eq!(address_count(&ty).unwrap(), expected_address_count);
    }

    #[rstest]
    #[case(NadaType::Integer, 1)]
    #[case(NadaType::UnsignedInteger, 1)]
    #[case(NadaType::Boolean, 1)]
    #[case(NadaType::SecretInteger, 1)]
    #[case(NadaType::SecretUnsignedInteger, 1)]
    #[case(NadaType::SecretBoolean, 1)]
    #[case(NadaType::ShamirShareInteger, 1)]
    #[case(NadaType::ShamirShareUnsignedInteger, 1)]
    #[case(NadaType::ShamirShareBoolean, 1)]
    #[case(NadaType::Array { inner_type: Box::new(NadaType::ShamirShareInteger), size: 5 }, 6)]
    #[case(NadaType::Array {
    inner_type: Box::new(NadaType::Array { inner_type: Box::new(NadaType::ShamirShareInteger), size: 5 })
    , size: 5
    }, 6)]
    #[case(NadaType::Tuple {
    left_type: Box::new(NadaType::ShamirShareInteger),
    right_type: Box::new(NadaType::ShamirShareInteger)
    }, 3)]
    fn test_result_address_count(#[case] ty: NadaType, #[case] address_count: usize) {
        assert_eq!(result_element_address_count(&ty), address_count);
    }
}

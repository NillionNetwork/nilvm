//! Clear modular types
//!
//! Clear modular values are the values provided by the user, in modular form.
//! Note: This type should be used for testing purposes only.

use crate::{
    clear::Clear,
    errors::{ClearModularError, NonPrimitiveValue},
    NadaValue, NeverPrimitiveType,
};
use math_lib::modular::{Modular, ModularNumber};
use nada_type::{NadaType, PrimitiveTypes};
use num_bigint::BigUint;
use std::{fmt::Debug, marker::PhantomData, ops::Mul};

/// Clear modular values are the values provided by the user, in modular form.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "secret-serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ClearModular<M: Modular> {
    _modular: PhantomData<M>,
}

impl<M: Modular> PrimitiveTypes for ClearModular<M> {
    // Public variables
    type Integer = ModularNumber<M>;
    type UnsignedInteger = ModularNumber<M>;
    type Boolean = ModularNumber<M>;
    type EcdsaDigestMessage = NeverPrimitiveType;
    type EcdsaPublicKey = NeverPrimitiveType;
    type StoreId = NeverPrimitiveType;

    // Abstract secrets
    type SecretInteger = ModularNumber<M>;
    type SecretUnsignedInteger = ModularNumber<M>;
    type SecretBoolean = ModularNumber<M>;
    type SecretBlob = NeverPrimitiveType;

    // Shares
    type ShamirShareInteger = NeverPrimitiveType;
    type ShamirShareUnsignedInteger = NeverPrimitiveType;
    type ShamirShareBoolean = NeverPrimitiveType;

    // Ecdsa Private Key
    type EcdsaPrivateKey = NeverPrimitiveType;

    // Ecdsa Signature
    type EcdsaSignature = NeverPrimitiveType;
}

impl<T: Modular> NadaValue<ClearModular<T>> {
    /// Build a `NadaValue<ClearModular<T>>` from an iterator of the `ModularNumber<T>` and a `NadaType`
    pub fn from_iter<I>(values: I, ty: NadaType) -> Result<Self, ClearModularError>
    where
        I: IntoIterator<Item = ModularNumber<T>>,
        <I as IntoIterator>::IntoIter: DoubleEndedIterator,
    {
        let mut values = Vec::from_iter(values.into_iter().rev());
        let mut flattened_types = ty.flatten_inner_types();
        let mut resultant_values = vec![];
        while let Some(ty) = flattened_types.pop() {
            match ty {
                NadaType::Integer => {
                    let value = values.pop().ok_or(ClearModularError::NotEnoughValues)?;
                    resultant_values.push(Self::new_integer(value));
                }
                NadaType::UnsignedInteger => {
                    let value = values.pop().ok_or(ClearModularError::NotEnoughValues)?;
                    resultant_values.push(Self::new_unsigned_integer(value));
                }
                NadaType::Boolean => {
                    let value = values.pop().ok_or(ClearModularError::NotEnoughValues)?;
                    resultant_values.push(Self::new_boolean(value));
                }
                NadaType::SecretInteger => {
                    let value = values.pop().ok_or(ClearModularError::NotEnoughValues)?;
                    resultant_values.push(Self::new_secret_integer(value));
                }
                NadaType::SecretUnsignedInteger => {
                    let value = values.pop().ok_or(ClearModularError::NotEnoughValues)?;
                    resultant_values.push(Self::new_secret_unsigned_integer(value));
                }

                NadaType::SecretBoolean => {
                    let value = values.pop().ok_or(ClearModularError::NotEnoughValues)?;
                    resultant_values.push(Self::new_secret_boolean(value));
                }
                NadaType::Array { size, .. } => {
                    let mut inner_values = vec![];
                    for _ in 0..size {
                        let value = resultant_values.pop().ok_or(ClearModularError::NotEnoughValues)?;
                        inner_values.push(value);
                    }
                    resultant_values.push(NadaValue::new_array_non_empty(inner_values)?)
                }
                NadaType::Tuple { .. } => {
                    let left = resultant_values.pop().ok_or(ClearModularError::NotEnoughValues)?;
                    let right = resultant_values.pop().ok_or(ClearModularError::NotEnoughValues)?;
                    resultant_values.push(NadaValue::new_tuple(left, right)?)
                }
                NadaType::NTuple { types } => {
                    let mut inner_values = vec![];
                    for _ in 0..types.len() {
                        let value = resultant_values.pop().ok_or(ClearModularError::NotEnoughValues)?;
                        inner_values.push(value);
                    }
                    resultant_values.push(NadaValue::new_n_tuple(inner_values)?)
                }
                NadaType::Object { types } => {
                    let mut inner_values = vec![];
                    for key in types.0.into_keys() {
                        let value = resultant_values.pop().ok_or(ClearModularError::NotEnoughValues)?;
                        inner_values.push((key, value));
                    }
                    resultant_values.push(NadaValue::new_object(inner_values.into_iter().collect())?)
                }
                NadaType::SecretBlob
                | NadaType::ShamirShareInteger
                | NadaType::ShamirShareUnsignedInteger
                | NadaType::ShamirShareBoolean
                | NadaType::EcdsaPrivateKey
                | NadaType::EcdsaDigestMessage
                | NadaType::EcdsaSignature
                | NadaType::EcdsaPublicKey
                | NadaType::StoreId => unreachable!(),
            }
        }
        resultant_values.pop().ok_or(ClearModularError::NotEnoughValues)
    }
}

impl<T: Modular> TryFrom<NadaValue<ClearModular<T>>> for ModularNumber<T> {
    type Error = NonPrimitiveValue;

    fn try_from(value: NadaValue<ClearModular<T>>) -> Result<Self, Self::Error> {
        match value {
            NadaValue::Integer(v)
            | NadaValue::UnsignedInteger(v)
            | NadaValue::Boolean(v)
            | NadaValue::SecretInteger(v)
            | NadaValue::SecretUnsignedInteger(v)
            | NadaValue::SecretBoolean(v) => Ok(v),
            NadaValue::Array { .. } | NadaValue::Tuple { .. } | NadaValue::NTuple { .. } | NadaValue::Object { .. } => {
                Err(NonPrimitiveValue)
            }
            NadaValue::SecretBlob(_)
            | NadaValue::ShamirShareInteger(_)
            | NadaValue::ShamirShareUnsignedInteger(_)
            | NadaValue::ShamirShareBoolean(_)
            | NadaValue::EcdsaPrivateKey(_)
            | NadaValue::EcdsaDigestMessage(_)
            | NadaValue::EcdsaSignature(_)
            | NadaValue::EcdsaPublicKey(_)
            | NadaValue::StoreId(_) => unreachable!(),
        }
    }
}

impl<T: Modular> TryFrom<NadaValue<Clear>> for NadaValue<ClearModular<T>> {
    type Error = ClearModularError;

    fn try_from(value: NadaValue<Clear>) -> Result<Self, Self::Error> {
        let ty = value.to_type();
        let mut inner_values = vec![value];
        let mut modular_values = vec![];
        while let Some(value) = inner_values.pop() {
            match value {
                NadaValue::Integer(value) | NadaValue::SecretInteger(value) => {
                    modular_values.push(ModularNumber::try_from(&value)?);
                }
                NadaValue::UnsignedInteger(value) | NadaValue::SecretUnsignedInteger(value) => {
                    modular_values.push(ModularNumber::try_from(&value)?);
                }
                NadaValue::Array { values, .. } => {
                    inner_values.extend(values.into_iter().rev());
                }
                NadaValue::Tuple { left, right } => {
                    inner_values.push(*right);
                    inner_values.push(*left);
                }
                NadaValue::NTuple { values } => {
                    inner_values.extend(values.into_iter().rev());
                }
                NadaValue::Object { values } => {
                    inner_values.extend(values.into_values().rev());
                }
                NadaValue::Boolean(value) | NadaValue::SecretBoolean(value) => {
                    let value = BigUint::from(value as u32);
                    modular_values.push(ModularNumber::try_from(&value)?);
                }
                NadaValue::SecretBlob(_)
                | NadaValue::ShamirShareInteger(_)
                | NadaValue::ShamirShareUnsignedInteger(_)
                | NadaValue::ShamirShareBoolean(_)
                | NadaValue::EcdsaPrivateKey(_)
                | NadaValue::EcdsaDigestMessage(_)
                | NadaValue::EcdsaSignature(_)
                | NadaValue::EcdsaPublicKey(_)
                | NadaValue::StoreId(_) => unreachable!(),
            }
        }
        NadaValue::from_iter(modular_values, ty)
    }
}

impl<T: Modular> Mul<NadaValue<ClearModular<T>>> for NadaValue<ClearModular<T>> {
    type Output = Result<NadaValue<ClearModular<T>>, ClearModularError>;

    fn mul(self, rhs: NadaValue<ClearModular<T>>) -> Self::Output {
        match (self, rhs) {
            (NadaValue::Integer(l), NadaValue::Integer(r)) => Ok(NadaValue::new_integer(l * &r)),
            (NadaValue::UnsignedInteger(l), NadaValue::UnsignedInteger(r)) => {
                Ok(NadaValue::new_unsigned_integer(l * &r))
            }
            (NadaValue::Boolean(l), NadaValue::Boolean(r)) => Ok(NadaValue::new_boolean(l * &r)),
            (NadaValue::Integer(l), NadaValue::SecretInteger(r))
            | (NadaValue::SecretInteger(l), NadaValue::Integer(r))
            | (NadaValue::SecretInteger(l), NadaValue::SecretInteger(r)) => Ok(NadaValue::new_secret_integer(l * &r)),

            (NadaValue::SecretUnsignedInteger(l), NadaValue::UnsignedInteger(r))
            | (NadaValue::UnsignedInteger(l), NadaValue::SecretUnsignedInteger(r))
            | (NadaValue::SecretUnsignedInteger(l), NadaValue::SecretUnsignedInteger(r)) => {
                Ok(crate::clear_modular::NadaValue::new_secret_unsigned_integer(l * &r))
            }
            (NadaValue::Boolean(l), NadaValue::SecretBoolean(r))
            | (NadaValue::SecretBoolean(l), NadaValue::Boolean(r))
            | (NadaValue::SecretBoolean(l), NadaValue::SecretBoolean(r)) => Ok(NadaValue::new_secret_boolean(l * &r)),
            (NadaValue::Array { .. }, _)
            | (_, NadaValue::Array { .. })
            | (NadaValue::Tuple { .. }, _)
            | (_, NadaValue::Tuple { .. }) => Err(ClearModularError::Unsupported("compound types".to_string())),
            (left, rhs) => Err(ClearModularError::Unsupported(format!("{left:?} * {rhs:?}"))),
        }
    }
}
#[cfg(test)]
mod tests {
    use crate::{clear_modular::ClearModular, NadaValue};
    use anyhow::Error;
    use math_lib::modular::{ModularNumber, U64SafePrime};
    use nada_type::NadaType;
    use num_bigint::BigInt;
    use rstest::rstest;

    type Prime = U64SafePrime;

    fn new_secret_integers(values: Vec<u64>) -> Vec<NadaValue<ClearModular<Prime>>> {
        values.into_iter().map(|value| NadaValue::new_secret_integer(ModularNumber::from_u64(value))).collect()
    }

    #[test]
    fn from_iter_array() -> Result<(), Error> {
        let size = 3usize;

        let values: Vec<_> = (0..(size * size)).into_iter().map(|v| ModularNumber::from_u64(v as u64)).collect();

        let matrix_inner_type = NadaType::Array { inner_type: Box::new(NadaType::SecretInteger), size };
        let matrix_type = NadaType::Array { inner_type: Box::new(matrix_inner_type.clone()), size };
        let matrix: NadaValue<ClearModular<Prime>> = NadaValue::from_iter(values.clone(), matrix_type)?;

        let mut flattened_values = matrix.flatten_inner_values().into_iter();
        use NadaType::*;
        assert_eq!(
            flattened_values.next(),
            Some(NadaValue::Array {
                inner_type: Array { inner_type: Box::new(SecretInteger), size: 3 },
                values: vec![
                    NadaValue::Array { inner_type: SecretInteger, values: new_secret_integers(vec![8, 7, 6]) },
                    NadaValue::Array { inner_type: SecretInteger, values: new_secret_integers(vec![5, 4, 3]) },
                    NadaValue::Array { inner_type: SecretInteger, values: new_secret_integers(vec![2, 1, 0]) }
                ]
            })
        );
        let mut value = ModularNumber::from_u64(0u64);
        for index in 0..size as u64 {
            let multiplier: u64 = index * size as u64;
            let expected_value = NadaValue::Array {
                inner_type: NadaType::SecretInteger,
                values: new_secret_integers(vec![multiplier + 2, multiplier + 1, multiplier]),
            };
            assert_eq!(flattened_values.next(), Some(expected_value));
            for _ in 0..size {
                let expected_value: NadaValue<ClearModular<Prime>> = NadaValue::new_secret_integer(value);
                assert_eq!(flattened_values.next(), Some(expected_value));
                value = value + &ModularNumber::ONE;
            }
        }

        Ok(())
    }

    enum SecretVariant {
        Public,
        Secret,
    }

    fn into_integer_nada_value(input: (i64, SecretVariant)) -> NadaValue<ClearModular<Prime>> {
        match input.1 {
            Public => NadaValue::new_integer(ModularNumber::try_from(&BigInt::from(input.0)).unwrap()),
            Secret => NadaValue::new_secret_integer(ModularNumber::try_from(&BigInt::from(input.0)).unwrap()),
        }
    }

    fn into_unsigned_integer_nada_value(input: (u32, SecretVariant)) -> NadaValue<ClearModular<Prime>> {
        match input.1 {
            Public => NadaValue::new_unsigned_integer(ModularNumber::from_u32(input.0)),
            Secret => NadaValue::new_secret_unsigned_integer(ModularNumber::from_u32(input.0)),
        }
    }

    use SecretVariant::*;

    #[rstest]
    #[case((-2, Public), (4,Public), (-8, Public))]
    #[case((-2, Public), (4,Secret), (-8, Secret))]
    #[case((-2, Secret), (4,Public), (-8, Secret))]
    #[case((-2, Secret), (4,Secret), (-8, Secret))]

    fn test_product_integers(
        #[case] left: (i64, SecretVariant),
        #[case] right: (i64, SecretVariant),
        #[case] expected_result: (i64, SecretVariant),
    ) {
        assert_eq!(
            into_integer_nada_value(expected_result),
            (into_integer_nada_value(left) * into_integer_nada_value(right)).unwrap()
        );
    }

    #[rstest]
    #[case((2, Public), (4,Public), (8, Public))]
    #[case((2, Public), (4,Secret), (8, Secret))]
    #[case((2, Secret), (4,Public), (8, Secret))]
    #[case((2, Secret), (4,Secret), (8, Secret))]

    fn test_product_unsigned_integers(
        #[case] left: (u32, SecretVariant),
        #[case] right: (u32, SecretVariant),
        #[case] expected_result: (u32, SecretVariant),
    ) {
        assert_eq!(
            into_unsigned_integer_nada_value(expected_result),
            (into_unsigned_integer_nada_value(left) * into_unsigned_integer_nada_value(right)).unwrap()
        );
    }
}

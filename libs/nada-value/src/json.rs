//! Utilities to parse Nada values from untyped JSON objects.

use crate::{clear::Clear, NadaInt, NadaUint, NadaValue};
use anyhow::{anyhow, Context, Result};
use nada_type::NadaType;
use num_bigint::{BigInt, BigUint};
use num_traits::ToPrimitive;
use serde_json::{Number, Value as JsonValue};
use std::collections::HashMap;

/// Creates a map of Nada values from an untyped JSON object.
/// it uses the provided types to parse the JSON values.
pub fn nada_values_from_untyped_json(
    types: HashMap<String, NadaType>,
    json_value: JsonValue,
) -> Result<HashMap<String, NadaValue<Clear>>> {
    let serde_json::Value::Object(mut value) = json_value else {
        return Err(anyhow!("Invalid json root, it should be an object"));
    };
    let mut nada_values = HashMap::with_capacity(value.len());
    for (name, nada_type) in types {
        let json_value =
            value.remove(&name).ok_or_else(|| anyhow!("expected key '{name}' not found in json root object"))?;
        let nada_value =
            NadaValue::from_untyped_json(&nada_type, json_value).with_context(|| format!("in key '{name}'"))?;
        nada_values.insert(name, nada_value);
    }
    Ok(nada_values)
}

/// Creates a map of Nada values from an untyped JSON object.
/// it uses the provided types to parse the JSON values.
/// Missing keys in json are ignored.
pub fn nada_values_from_untyped_json_partial(
    types: HashMap<String, NadaType>,
    json_value: JsonValue,
) -> Result<HashMap<String, NadaValue<Clear>>> {
    let serde_json::Value::Object(value) = json_value else {
        return Err(anyhow!("Invalid json root, it should be an object"));
    };
    let mut nada_values = HashMap::with_capacity(value.len());
    for (name, json_value) in value {
        let nada_type = types.get(&name).ok_or_else(|| anyhow!("expected key '{name}' not found in values"))?;
        let nada_value =
            NadaValue::from_untyped_json(nada_type, json_value).with_context(|| format!("in key '{name}'"))?;
        nada_values.insert(name, nada_value);
    }
    Ok(nada_values)
}

/// Transforms a map of Nada values into a JSON object.
pub fn nada_values_to_json(values: HashMap<String, NadaValue<Clear>>) -> Result<JsonValue> {
    let mut json_values = serde_json::Map::with_capacity(values.len());
    for (name, value) in values {
        json_values.insert(name, value.to_json_value()?);
    }
    Ok(JsonValue::Object(json_values))
}

impl TryFrom<JsonValue> for NadaInt {
    type Error = anyhow::Error;

    fn try_from(value: JsonValue) -> Result<Self> {
        let value = match value {
            JsonValue::Number(n) => {
                let n = n.as_i64().ok_or_else(|| anyhow!("Invalid json value for integer"))?;
                BigInt::from(n)
            }
            JsonValue::String(s) => s.parse::<BigInt>()?,
            _ => return Err(anyhow!("Invalid json value for integer")),
        };
        Ok(NadaInt::from(value))
    }
}

impl TryFrom<JsonValue> for NadaUint {
    type Error = anyhow::Error;

    fn try_from(value: JsonValue) -> Result<Self> {
        let value = match value {
            JsonValue::Number(n) => {
                let n = n.as_u64().ok_or_else(|| anyhow!("Invalid json value for unsigned integer"))?;
                BigUint::from(n)
            }
            JsonValue::String(s) => s.parse::<BigUint>()?,
            _ => return Err(anyhow!("Invalid json value for unsigned integer")),
        };
        Ok(NadaUint::from(value))
    }
}

impl NadaValue<Clear> {
    /// Creates a Nada value from an untyped JSON object.
    /// it uses the provided type to parse the JSON value.
    pub fn from_untyped_json(nada_type: &NadaType, value: JsonValue) -> Result<NadaValue<Clear>> {
        let mut values = vec![(nada_type, value)];
        let mut results = vec![];

        while let Some((nada_type, value)) = values.pop() {
            let result = match nada_type {
                NadaType::Integer => {
                    let value = NadaInt::try_from(value)?;
                    Some(NadaValue::new_integer(value))
                }
                NadaType::UnsignedInteger => {
                    let value = NadaUint::try_from(value)?;
                    Some(NadaValue::new_unsigned_integer(value))
                }
                NadaType::Boolean => {
                    let JsonValue::Bool(b) = value else { return Err(anyhow!("Invalid json value for boolean")) };
                    Some(NadaValue::new_boolean(b))
                }
                NadaType::SecretInteger => {
                    let value = NadaInt::try_from(value)?;
                    Some(NadaValue::new_secret_integer(value))
                }
                NadaType::SecretUnsignedInteger => {
                    let value = NadaUint::try_from(value)?;
                    Some(NadaValue::new_secret_unsigned_integer(value))
                }

                NadaType::SecretBoolean => {
                    let JsonValue::Bool(b) = value else {
                        return Err(anyhow!("Invalid json value for secret boolean"));
                    };
                    Some(NadaValue::new_secret_boolean(b))
                }
                NadaType::SecretBlob => {
                    let JsonValue::Array(values) = value else {
                        return Err(anyhow!("Invalid json value for secret blob, expected array"));
                    };
                    let blob = values
                        .into_iter()
                        .map(|v| match v {
                            JsonValue::Number(n) => {
                                let n = n
                                    .as_u64()
                                    .ok_or_else(|| anyhow!("Invalid json value for secret blob, expect number"))?;
                                Ok(u8::try_from(n)
                                    .context("Invalid json value for secret blob, expect number 0-255")?)
                            }
                            _ => Err(anyhow!("Invalid json value for secret blob")),
                        })
                        .collect::<Result<Vec<_>>>()?;
                    Some(NadaValue::new_secret_blob(blob))
                }
                NadaType::ShamirShareInteger
                | NadaType::ShamirShareUnsignedInteger
                | NadaType::ShamirShareBoolean
                | NadaType::EcdsaPrivateKey
                | NadaType::EcdsaDigestMessage
                | NadaType::EcdsaSignature => return Err(anyhow!("Unsupported type: {:?}", nada_type)),
                NadaType::Array { inner_type, size } => {
                    let JsonValue::Array(inner_values) = value else {
                        return Err(anyhow!("Invalid json value for {nada_type:?}, expected array",));
                    };
                    let json_len = inner_values.len();
                    if json_len != *size {
                        return Err(anyhow!("Invalid size for {nada_type:?}, expected {size} got {json_len}"));
                    }
                    for v in inner_values.into_iter().rev() {
                        values.push((inner_type.as_ref(), v));
                    }
                    None
                }
                NadaType::Tuple { right_type, left_type } => {
                    let JsonValue::Array(mut inner_values) = value else {
                        return Err(anyhow!("Invalid json value for {nada_type:?}, expected array of two elements"));
                    };
                    let left = inner_values.pop().ok_or_else(|| {
                        anyhow!("Invalid json value for {nada_type:?}, expected array of two elements")
                    })?;
                    let right = inner_values.pop().ok_or_else(|| {
                        anyhow!("Invalid json value for {nada_type:?}, expected array of two elements")
                    })?;
                    values.push((left_type.as_ref(), left));
                    values.push((right_type.as_ref(), right));
                    None
                }
                NadaType::NTuple { types } => {
                    let JsonValue::Array(inner_values) = value else {
                        return Err(anyhow!("Invalid json value for {nada_type:?}, expected array"));
                    };
                    let json_len = inner_values.len();
                    if json_len != types.len() {
                        return Err(anyhow!("Invalid size for {nada_type:?}, expected {} got {json_len}", types.len()));
                    }
                    for (v, inner_type) in inner_values.into_iter().zip(types.iter()).rev() {
                        values.push((inner_type, v));
                    }
                    None
                }
                NadaType::Object { types } => {
                    let JsonValue::Object(inner_values) = value else {
                        return Err(anyhow!("Invalid json value for {nada_type:?}, expected object"));
                    };
                    for key in inner_values.keys() {
                        if !types.contains_key(key) {
                            return Err(anyhow!(
                                "Unexpected key {key} in json object, expected keys: {:?}",
                                types.keys()
                            ));
                        }
                    }
                    for key in types.keys() {
                        if !inner_values.contains_key(key) {
                            return Err(anyhow!(
                                "Expected key {key} in json object, expected keys: {:?}",
                                types.keys()
                            ));
                        }
                    }
                    let json_len = inner_values.len();
                    if json_len != types.len() {
                        return Err(anyhow!("Invalid size for {nada_type:?}, expected {} got {json_len}", types.len()));
                    }
                    for (v, inner_type) in inner_values.values().zip(types.values()).rev() {
                        values.push((inner_type, v.clone()));
                    }
                    None
                }
            };
            results.push((nada_type, result));
        }

        let mut values = vec![];
        while let Some((nada_type, value)) = results.pop() {
            match nada_type {
                NadaType::Integer
                | NadaType::UnsignedInteger
                | NadaType::Boolean
                | NadaType::SecretInteger
                | NadaType::SecretUnsignedInteger
                | NadaType::SecretBoolean => {
                    values.push(value.ok_or_else(|| anyhow!("This should not happen it is a bug"))?);
                }
                NadaType::SecretBlob
                | NadaType::ShamirShareInteger
                | NadaType::ShamirShareUnsignedInteger
                | NadaType::ShamirShareBoolean
                | NadaType::EcdsaPrivateKey
                | NadaType::EcdsaDigestMessage
                | NadaType::EcdsaSignature => return Err(anyhow!("Unsupported type: {:?}", nada_type)),
                NadaType::Array { inner_type, size } => {
                    let mut array_values = vec![];
                    for _ in 0..*size {
                        let value = values.pop().ok_or_else(|| anyhow!("This should not happen it is a bug"))?;
                        array_values.push(value);
                    }
                    let value = NadaValue::new_array(*inner_type.clone(), array_values)?;
                    values.push(value);
                }
                NadaType::Tuple { .. } => {
                    let left = values.pop().ok_or_else(|| anyhow!("This should not happen it is a bug"))?;
                    let right = values.pop().ok_or_else(|| anyhow!("This should not happen it is a bug"))?;
                    let value = NadaValue::new_tuple(left, right)?;
                    values.push(value);
                }
                NadaType::NTuple { types } => {
                    let mut tuple_values = vec![];
                    for _ in 0..types.len() {
                        let value = values.pop().ok_or_else(|| anyhow!("This should not happen it is a bug"))?;
                        tuple_values.push(value);
                    }
                    let value = NadaValue::new_n_tuple(tuple_values)?;
                    values.push(value);
                }
                NadaType::Object { types } => {
                    let mut object_values = vec![];
                    for key in types.keys() {
                        let value = values.pop().ok_or_else(|| anyhow!("This should not happen it is a bug"))?;
                        object_values.push((key.clone(), value));
                    }
                    let value = NadaValue::new_object(object_values.into_iter().collect())?;
                    values.push(value);
                }
            }
        }
        debug_assert!(values.len() == 1);
        values.pop().ok_or_else(|| anyhow!("This should not happen it is a bug"))
    }

    /// Transforms the Nada value into a JSON value.
    pub fn to_json_value(&self) -> Result<JsonValue> {
        let result = match self {
            NadaValue::Integer(integer) | NadaValue::SecretInteger(integer) => {
                if let Some(integer) = integer.to_i64() {
                    JsonValue::Number(Number::from(integer))
                } else {
                    JsonValue::String(integer.to_string())
                }
            }
            NadaValue::UnsignedInteger(unsigned) | NadaValue::SecretUnsignedInteger(unsigned) => {
                if let Some(unsigned) = unsigned.to_i64() {
                    JsonValue::Number(Number::from(unsigned))
                } else {
                    JsonValue::String(unsigned.to_string())
                }
            }
            NadaValue::Boolean(boolean) | NadaValue::SecretBoolean(boolean) => JsonValue::Bool(*boolean),
            NadaValue::SecretBlob(blob) => {
                JsonValue::Array(blob.iter().map(|b| JsonValue::Number(Number::from(*b))).collect())
            }
            NadaValue::ShamirShareInteger(_)
            | NadaValue::ShamirShareUnsignedInteger(_)
            | NadaValue::ShamirShareBoolean(_)
            | NadaValue::EcdsaPrivateKey(_)
            | NadaValue::EcdsaDigestMessage(_)
            | NadaValue::EcdsaSignature(_) => return Err(anyhow!("Unsupported type: {:?}", self)),
            NadaValue::Array { values, .. } => {
                JsonValue::Array(values.iter().map(|v| v.to_json_value()).collect::<Result<_, _>>()?)
            }
            NadaValue::Tuple { right, left } => JsonValue::Array(vec![left.to_json_value()?, right.to_json_value()?]),
            NadaValue::NTuple { values } => {
                JsonValue::Array(values.iter().map(|v| v.to_json_value()).collect::<Result<_, _>>()?)
            }
            NadaValue::Object { values } => JsonValue::Object(
                values
                    .iter()
                    .map(|(k, v)| v.to_json_value().map(|v| (k.clone(), v)))
                    .collect::<Result<serde_json::map::Map<_, _>, _>>()?,
            ),
        };

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        json::{nada_values_from_untyped_json, nada_values_to_json},
        NadaValue,
    };
    use anyhow::Result;
    use indexmap::IndexMap;
    use nada_type::NadaType;
    use std::collections::HashMap;

    #[test]
    fn test_from_json_untyped() -> Result<()> {
        let json = serde_json::json!({
            "int": -32,
            "uint": 42,
            "bool": true,
            "secret_int": -32,
            "secret_uint": 42,
            "secret_bool": false,
            "array": [1, 2, 3],
            "array_2d": [[1, 2], [3, 4]],
            "tuple": [true, false],
            "ntuple": [true, false],
            "object": {"a": 42, "b": true}
        });

        let nada_types: HashMap<_, _> = HashMap::from([
            ("int".to_string(), NadaType::Integer),
            ("uint".to_string(), NadaType::UnsignedInteger),
            ("bool".to_string(), NadaType::Boolean),
            ("secret_int".to_string(), NadaType::SecretInteger),
            ("secret_uint".to_string(), NadaType::SecretUnsignedInteger),
            ("secret_bool".to_string(), NadaType::SecretBoolean),
            ("array".to_string(), NadaType::Array { inner_type: Box::new(NadaType::Integer), size: 3 }),
            (
                "array_2d".to_string(),
                NadaType::Array {
                    inner_type: Box::new(NadaType::Array { inner_type: Box::new(NadaType::Integer), size: 2 }),
                    size: 2,
                },
            ),
            (
                "tuple".to_string(),
                NadaType::Tuple { left_type: Box::new(NadaType::Boolean), right_type: Box::new(NadaType::Boolean) },
            ),
            ("ntuple".to_string(), NadaType::NTuple { types: vec![NadaType::Boolean, NadaType::Boolean] }),
            (
                "object".to_string(),
                NadaType::Object {
                    types: IndexMap::from([("a".to_string(), NadaType::Integer), ("b".to_string(), NadaType::Boolean)])
                        .into(),
                },
            ),
        ]);

        let expected_result = HashMap::from([
            ("int".to_string(), NadaValue::new_integer(-32)),
            ("uint".to_string(), NadaValue::new_unsigned_integer(42u64)),
            ("bool".to_string(), NadaValue::new_boolean(true)),
            ("secret_int".to_string(), NadaValue::new_secret_integer(-32)),
            ("secret_uint".to_string(), NadaValue::new_secret_unsigned_integer(42u64)),
            ("secret_bool".to_string(), NadaValue::new_secret_boolean(false)),
            (
                "array".to_string(),
                NadaValue::new_array_non_empty(vec![
                    NadaValue::new_integer(1),
                    NadaValue::new_integer(2),
                    NadaValue::new_integer(3),
                ])?,
            ),
            (
                "array_2d".to_string(),
                NadaValue::new_array_non_empty(vec![
                    NadaValue::new_array(
                        NadaType::Integer,
                        vec![NadaValue::new_integer(1), NadaValue::new_integer(2)],
                    )?,
                    NadaValue::new_array(
                        NadaType::Integer,
                        vec![NadaValue::new_integer(3), NadaValue::new_integer(4)],
                    )?,
                ])?,
            ),
            ("tuple".to_string(), NadaValue::new_tuple(NadaValue::new_boolean(true), NadaValue::new_boolean(false))?),
            (
                "ntuple".to_string(),
                NadaValue::new_n_tuple(vec![NadaValue::new_boolean(true), NadaValue::new_boolean(false)])?,
            ),
            (
                "object".to_string(),
                NadaValue::new_object(IndexMap::from([
                    ("a".to_string(), NadaValue::new_integer(42)),
                    ("b".to_string(), NadaValue::new_boolean(true)),
                ]))?,
            ),
        ]);

        let result = nada_values_from_untyped_json(nada_types, json).unwrap();
        assert_eq!(expected_result, result);
        Ok(())
    }
    #[test]
    fn test_to_json() -> Result<()> {
        let json = serde_json::json!({
            "int": -32,
            "uint": 42,
            "bool": true,
            "secret_int": -32,
            "secret_uint": 42,
            "secret_bool": false,
            "array": [1, 2, 3],
            "array_2d": [[1, 2], [3, 4]] ,
            "tuple": [true, false],
            "ntuple": [true, false],
            "object": {"a": 42, "b": true}
        });

        let nada_types: HashMap<_, _> = HashMap::from([
            ("int".to_string(), NadaType::Integer),
            ("uint".to_string(), NadaType::UnsignedInteger),
            ("bool".to_string(), NadaType::Boolean),
            ("secret_int".to_string(), NadaType::SecretInteger),
            ("secret_uint".to_string(), NadaType::SecretUnsignedInteger),
            ("secret_bool".to_string(), NadaType::SecretBoolean),
            ("array".to_string(), NadaType::Array { inner_type: Box::new(NadaType::Integer), size: 3 }),
            (
                "array_2d".to_string(),
                NadaType::Array {
                    inner_type: Box::new(NadaType::Array { inner_type: Box::new(NadaType::Integer), size: 2 }),
                    size: 2,
                },
            ),
            (
                "tuple".to_string(),
                NadaType::Tuple { left_type: Box::new(NadaType::Boolean), right_type: Box::new(NadaType::Boolean) },
            ),
            ("ntuple".to_string(), NadaType::NTuple { types: vec![NadaType::Boolean, NadaType::Boolean] }),
            (
                "object".to_string(),
                NadaType::Object {
                    types: IndexMap::from([("a".to_string(), NadaType::Integer), ("b".to_string(), NadaType::Boolean)])
                        .into(),
                },
            ),
        ]);
        let result = nada_values_from_untyped_json(nada_types, json.clone()).unwrap();
        let result = nada_values_to_json(result).unwrap();
        assert_eq!(result, json);
        Ok(())
    }
}

//! NadaValues.

use crate::errors::{JsResult, ValueError};
use js_sys::{Object, Uint8Array};
use nillion_client_core::{
    generic_ec::{curves::Secp256k1, NonZero, Scalar},
    privatekey::EcdsaPrivateKey,
    signature,
    values::{BigInt, BigUint, Clear, Encoded, Encrypted, PartyJar},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use wasm_bindgen::{prelude::wasm_bindgen, JsError, JsValue};

/// NadaValue
///
/// This type represents a value in the Nillion network. This class provides utilities
/// to encode numerical and binary values. It also provides methods to decode
/// the value into a numerical form.
///
/// @hideconstructor
#[wasm_bindgen(inspectable)]
#[derive(Serialize, Deserialize)]
#[cfg_attr(test, derive(Debug, Clone, PartialEq))]
pub struct NadaValue(pub(crate) nillion_client_core::values::NadaValue<Clear>);

#[wasm_bindgen]
impl NadaValue {
    /// Create a new secret integer value.
    ///
    /// @param {string} value - The value must be a valid string representation of an integer.
    /// @return {NadaValue} The encoded secret corresponding to the value provided
    ///
    /// @example
    /// const value = NadaValue.new_secret_integer("-23");
    #[wasm_bindgen(skip_jsdoc)]
    pub fn new_secret_integer(value: &str) -> JsResult<NadaValue> {
        let value: BigInt = value.parse().map_err(|e| ValueError::new_err(&format!("Invalid integer value: {e}")))?;
        let value = nillion_client_core::values::NadaValue::new_secret_integer(value);
        Ok(Self(value))
    }

    /// Create a new secret unsigned integer value.
    ///
    /// @param {string} value - The value must be a valid string representation of an unsigned integer.
    /// @return {NadaValue} The encoded secret corresponding to the value provided
    ///
    /// @example
    /// const value = NadaValue.new_secret_unsigned_integer("23");
    #[wasm_bindgen(skip_jsdoc)]
    pub fn new_secret_unsigned_integer(value: &str) -> JsResult<NadaValue> {
        let value: BigUint =
            value.parse().map_err(|e| ValueError::new_err(&format!("Invalid unsigned integer value: {e}")))?;
        let value = nillion_client_core::values::NadaValue::new_secret_unsigned_integer(value);
        Ok(Self(value))
    }

    /// Create a new secret blob.
    ///
    /// @param {Uint8Array} value - The blob in binary (byte array) encoded format
    /// @return {NadaValue} The encoded secret corresponding to the value provided
    ///
    /// @example
    /// const value = NadaValue.new_secret_blob([1,0,1,222,21]);
    #[wasm_bindgen(skip_jsdoc)]
    pub fn new_secret_blob(value: Vec<u8>) -> NadaValue {
        let secret = nillion_client_core::values::NadaValue::new_secret_blob(value);
        Self(secret)
    }

    /// Create a new public integer with the provided value.
    ///
    /// @param {string} value - The value must be a valid string representation of an integer.
    /// @return {NadaValue} The encoded public variable corresponding to the value provided
    ///
    /// @example
    /// const value = NadaValue.new_public_integer("-23");
    #[wasm_bindgen(skip_jsdoc)]
    pub fn new_public_integer(value: &str) -> JsResult<NadaValue> {
        let value: BigInt = value.parse().map_err(|e| ValueError::new_err(&format!("Invalid integer value: {e}")))?;
        let value = nillion_client_core::values::NadaValue::new_integer(value);
        Ok(Self(value))
    }

    /// Create a new public unsigned integer with the provided value.
    ///
    /// @param {string} value - The value must be a valid string representation of an unsigned integer.
    /// @return {NadaValue} The encoded public variable corresponding to the value provided
    ///
    /// @example
    /// const value = NadaValue.new_public_unsigned_integer("23");
    #[wasm_bindgen(skip_jsdoc)]
    pub fn new_public_unsigned_integer(value: &str) -> JsResult<NadaValue> {
        let value: BigUint =
            value.parse().map_err(|e| ValueError::new_err(&format!("Invalid unsigned integer value: {e}")))?;
        let value = nillion_client_core::values::NadaValue::new_unsigned_integer(value);
        Ok(Self(value))
    }

    /// Create a new ecdsa private key
    ///
    /// @param {Uint8Array} value - The ecdsa private key in binary (byte array) encoded format
    /// @return {NadaValue} The encoded secret corresponding to the value provided
    ///
    /// @example
    /// const value = NadaValue.new_ecdsa_private_key([1,0,1,222,21,...]);
    #[wasm_bindgen(skip_jsdoc)]
    pub fn new_ecdsa_private_key(value: Vec<u8>) -> JsResult<NadaValue> {
        let private_key = EcdsaPrivateKey::from_bytes(&value)
            .map_err(|e| ValueError::new_err(&format!("Invalid ecdsa private key: {e}")))?;
        let secret = nillion_client_core::values::NadaValue::new_ecdsa_private_key(private_key);
        Ok(Self(secret))
    }

    /// Create a new ecdsa digest message.
    ///
    /// @param {Uint8Array} value - The ecdsa digest message in binary (byte array) encoded format
    /// @return {NadaValue} The encoded secret corresponding to the value provided
    ///
    /// @example
    /// const value = NadaValue.new_ecdsa_digest_message([1,0,1,222,21,...]);
    #[wasm_bindgen(skip_jsdoc)]
    pub fn new_ecdsa_digest_message(value: Vec<u8>) -> JsResult<NadaValue> {
        let array: [u8; 32] =
            value.try_into().map_err(|_| ValueError::new_err("Message digest must be exactly 32 bytes long"))?;
        let secret = nillion_client_core::values::NadaValue::new_ecdsa_digest_message(array);
        Ok(Self(secret))
    }

    /// Create a new ecdsa signature.
    ///
    /// @param {Uint8Array} r - The r component of the signature in binary (byte array) encoded format
    /// @param {Uint8Array} s - The s component of the signature in binary (byte array) encoded format
    /// @return {NadaValue} The encoded secret corresponding to the value provided
    ///
    /// @example
    /// const value = NadaValue::new_ecdsa_signature(EcdsaSignature { r, s });
    #[wasm_bindgen(skip_jsdoc)]
    pub fn new_ecdsa_signature(r: Vec<u8>, s: Vec<u8>) -> JsResult<NadaValue> {
        let r = try_into_scalar(&r, "r")?;
        let s = try_into_scalar(&s, "s")?;
        Ok(Self(nillion_client_core::values::NadaValue::new_ecdsa_signature(signature::EcdsaSignature { r, s })))
    }

    /// Convert this value into a byte array.
    ///
    /// This is only valid for secret blob values.
    /// @return {Uint8Array} the byte array contained in this value.
    /// @throws {Error} if the value is not a secret blob.
    ///
    /// @example
    /// const value = NadaValue.new_secret_blob([1,0,1,222,21]);
    /// const byteArray = value.into_byte_array();
    #[wasm_bindgen(skip_jsdoc)]
    pub fn into_byte_array(self) -> Result<Vec<u8>, JsError> {
        match self.0 {
            nillion_client_core::values::NadaValue::SecretBlob(value) => Ok(value.to_vec()),
            nillion_client_core::values::NadaValue::EcdsaPrivateKey(value) => Ok(value.to_bytes()),
            nillion_client_core::values::NadaValue::EcdsaDigestMessage(value) => Ok(value.into()),
            _ => Err(JsError::new("value does not contain a byte array")),
        }
    }

    /// Convert this value into a byte array.
    ///
    /// This is only valid for secret blob values.
    /// @return {Uint8Array} the byte array contained in this value.
    /// @throws {Error} if the value is not a secret blob.
    ///
    /// @example
    /// const value = NadaValue.new_secret_blob([1,0,1,222,21]);
    /// const byteArray = value.into_byte_array();
    #[wasm_bindgen(skip_jsdoc)]
    pub fn try_into_signature(self) -> Result<EcdsaSignature, JsError> {
        if let nillion_client_core::values::NadaValue::EcdsaSignature(signature) = self.0 {
            let signature::EcdsaSignature { r, s } = signature;
            let r = Scalar::to_be_bytes(&r).to_vec();
            let s = Scalar::to_be_bytes(&s).to_vec();
            Ok(EcdsaSignature::new(r, s))
        } else {
            Err(JsError::new("value is not a ecdsa signature"))
        }
    }

    /// Convert this value into a string representation of the underlying numeric value.
    ///
    /// This only works for numeric secret values, such as integers and unsigned integers.
    /// @return {string} a string representation of the underlying numeric value
    ///
    /// @example
    /// const value = NadaValue.new_public_integer("23");
    /// const integer_value = value.into_integer();
    #[wasm_bindgen(skip_jsdoc)]
    pub fn into_integer(self) -> Result<String, JsError> {
        use nillion_client_core::values::NadaValue::*;

        match self.0 {
            SecretInteger(value) => Ok(value.to_string()),
            SecretUnsignedInteger(value) => Ok(value.to_string()),
            Integer(value) => Ok(value.to_string()),
            UnsignedInteger(value) => Ok(value.to_string()),
            _ => Err(JsError::new("value is not a number")),
        }
    }

    /// Return the Nada type represented by this instance.
    ///
    /// @example
    /// const value = NadaValue.new_secret_integer("42");
    /// console.log(value.type()); // "SecretInteger"
    #[wasm_bindgen(skip_jsdoc)]
    pub fn type_name(&self) -> JsResult<String> {
        use nillion_client_core::values::NadaValue::*;
        let type_str = match self.0 {
            Integer(_) => "PublicInteger",
            UnsignedInteger(_) => "PublicUnsignedInteger",
            SecretInteger(_) => "SecretInteger",
            SecretUnsignedInteger(_) => "SecretUnsignedInteger",
            SecretBlob(_) => "SecretBlob",
            EcdsaPrivateKey(_) => "EcdsaPrivateKey",
            EcdsaDigestMessage(_) => "EcdsaDigestMessage",
            EcdsaSignature(_) => "EcdsaSignature",
            _ => Err(JsError::new(&format!("Unsupported type {:?}", self.0)))?,
        };
        Ok(type_str.into())
    }
}

fn try_into_scalar(bytes: &[u8], parameter: &str) -> JsResult<NonZero<Scalar<Secp256k1>>> {
    let scalar = Scalar::from_be_bytes(bytes).map_err(|_| ValueError::new_err(&format!("Ecdsa signature parameter {parameter}: Format error as the encoded integer is larger than group order. Note that byte representation should be in big-endian format.")))?;
    NonZero::from_scalar(scalar)
        .ok_or_else(|| ValueError::new_err(&format!("Ecdsa signature parameter {parameter}: value cannot be 0")))
}

/// A collection of named values.
#[wasm_bindgen(inspectable)]
#[cfg_attr(test, derive(Debug, Clone, PartialEq))]
pub struct NadaValues(pub(crate) HashMap<String, nillion_client_core::values::NadaValue<Clear>>);

#[wasm_bindgen]
impl NadaValues {
    /// Creates a new empty instance of NadaValues.
    ///
    /// @example
    /// const values = new NadaValues();
    #[wasm_bindgen(constructor)]
    #[allow(clippy::new_without_default)]
    pub fn new() -> JsResult<NadaValues> {
        Ok(Self(Default::default()))
    }

    /// Add an encoded value to the NadaValues collection.
    ///
    /// @param {string} name - The name of the value
    /// @param {NadaValue} input - The value to be added
    ///
    /// @example
    /// values.insert("my_value", NadaValue.new_public_integer("23"));
    #[wasm_bindgen(skip_jsdoc)]
    pub fn insert(&mut self, name: String, input: &NadaValue) {
        self.0.insert(name, input.0.clone());
    }

    /// Get the number of values.
    ///
    /// @example
    /// const length = values.length;
    #[wasm_bindgen(getter)]
    pub fn length(&self) -> usize {
        self.0.len()
    }

    /// Convert NadaValues into a JS object
    ///
    /// @example
    /// const nadaValues = new NadaValues();
    /// nadaValues.insert("foo", NadaValue::new_secret_integer("42"));
    /// const values = nadaValues.to_record();
    /// console.log(values); // { foo: { type: "SecretInteger", value: "42" } }
    #[wasm_bindgen]
    pub fn to_record(&self) -> JsResult<JsValue> {
        let js_obj = Object::new();
        for (name, value) in &self.0 {
            let inner_obj = Object::new();

            let wrapped = NadaValue(value.clone());
            let nada_type = wrapped.type_name()?;

            js_sys::Reflect::set(&inner_obj, &JsValue::from("type"), &JsValue::from(&nada_type))
                .map_err(|e| JsError::new(&format!("Failed to set type: {:?}", e)))?;

            let js_value = match value {
                nillion_client_core::values::NadaValue::SecretBlob(_)
                | nillion_client_core::values::NadaValue::EcdsaPrivateKey(_)
                | nillion_client_core::values::NadaValue::EcdsaDigestMessage(_) => {
                    let byte_array = wrapped.into_byte_array()?;
                    let uint8_array = to_byte_array(&byte_array);
                    JsValue::from(uint8_array)
                }
                nillion_client_core::values::NadaValue::EcdsaSignature(_) => {
                    JsValue::from(wrapped.try_into_signature()?)
                }
                _ => JsValue::from(wrapped.into_integer()?),
            };

            js_sys::Reflect::set(&inner_obj, &JsValue::from("value"), &js_value)
                .map_err(|e| JsError::new(&format!("Failed to set value: {:?}", e)))?;

            js_sys::Reflect::set(&js_obj, &JsValue::from(name), &JsValue::from(inner_obj))
                .map_err(|e| JsError::new(&format!("Failed to set property: {:?}", e)))?;
        }
        Ok(JsValue::from(js_obj))
    }
}

/// A ecdsa signature
#[wasm_bindgen(inspectable)]
#[derive(Clone)]
pub struct EcdsaSignature {
    /// r component of the signature in binary format
    r: Vec<u8>,
    /// s component of the signature in binary format
    s: Vec<u8>,
}

#[wasm_bindgen]
impl EcdsaSignature {
    /// Construct a new instance the components.
    #[wasm_bindgen(constructor)]
    pub fn new(r: Vec<u8>, s: Vec<u8>) -> Self {
        Self { r, s }
    }

    /// Access r component of the signature
    pub fn r(&self) -> Uint8Array {
        to_byte_array(&self.r)
    }

    /// Access s component of the signature
    pub fn s(&self) -> Uint8Array {
        to_byte_array(&self.s)
    }
}

/// A party identifier.
#[wasm_bindgen(inspectable)]
#[derive(Clone)]
pub struct PartyId(Vec<u8>);

#[wasm_bindgen]
impl PartyId {
    /// Construct a new instance using the given identifier.
    #[wasm_bindgen(constructor)]
    pub fn new(id: Vec<u8>) -> Self {
        Self(id)
    }

    /// Access party id's underlying bytes.
    pub fn to_byte_array(&self) -> Uint8Array {
        to_byte_array(&self.0)
    }
}

/// Access party id's underlying bytes.
pub fn to_byte_array(bytes: &[u8]) -> Uint8Array {
    let uint8_array = Uint8Array::new_with_length(bytes.len() as u32);
    uint8_array.copy_from(bytes);
    uint8_array
}

/// A secret masker.
///
/// This allows masking and unmasking secrets.
#[wasm_bindgen]
pub struct SecretMasker(nillion_client_core::values::SecretMasker);

#[wasm_bindgen]
impl SecretMasker {
    /// Construct a new masker that uses a 64 bit safe prime under the hood.
    pub fn new_64_bit_safe_prime(polynomial_degree: u64, parties: Vec<PartyId>) -> JsResult<SecretMasker> {
        let parties = parties.into_iter().map(|p| nillion_client_core::values::PartyId::from(p.0)).collect();
        let masker = nillion_client_core::values::SecretMasker::new_64_bit_safe_prime(polynomial_degree, parties)
            .map_err(|e| ValueError::new_err(&format!("failed to create secret masker: {e}")))?;
        Ok(Self(masker))
    }

    /// Construct a new masker that uses a 128 bit safe prime under the hood.
    pub fn new_128_bit_safe_prime(polynomial_degree: u64, parties: Vec<PartyId>) -> JsResult<SecretMasker> {
        let parties = parties.into_iter().map(|p| nillion_client_core::values::PartyId::from(p.0)).collect();
        let masker = nillion_client_core::values::SecretMasker::new_128_bit_safe_prime(polynomial_degree, parties)
            .map_err(|e| ValueError::new_err(&format!("failed to create secret masker: {e}")))?;
        Ok(Self(masker))
    }

    /// Construct a new masker that uses a 256 bit safe prime under the hood.
    pub fn new_256_bit_safe_prime(polynomial_degree: u64, parties: Vec<PartyId>) -> JsResult<SecretMasker> {
        let parties = parties.into_iter().map(|p| nillion_client_core::values::PartyId::from(p.0)).collect();
        let masker = nillion_client_core::values::SecretMasker::new_256_bit_safe_prime(polynomial_degree, parties)
            .map_err(|e| ValueError::new_err(&format!("failed to create secret masker: {e}")))?;
        Ok(Self(masker))
    }

    /// Mask a set of values.
    pub fn mask(&self, values: NadaValues) -> JsResult<Vec<PartyShares>> {
        let shares = self.0.mask(values.0).map_err(|e| ValueError::new_err(&format!("failed to mask values: {e}")))?;
        let shares = shares
            .into_iter()
            .map(|(party, shares)| PartyShares {
                party: PartyId(party.as_ref().to_vec()),
                shares: EncryptedNadaValues(shares),
            })
            .collect();
        Ok(shares)
    }

    /// Unmask a set of encrypted values.
    pub fn unmask(&self, shares: Vec<PartyShares>) -> JsResult<NadaValues> {
        let shares = shares.into_iter().map(|party_shares| {
            (nillion_client_core::values::PartyId::from(party_shares.party.0), party_shares.shares.0)
        });
        let jar = PartyJar::new_with_elements(shares)
            .map_err(|e| ValueError::new_err(&format!("failed to unmask shares: {e}")))?;
        let values = self.0.unmask(jar).map_err(|e| ValueError::new_err(&format!("faile to unmask shares: {e}")))?;
        Ok(NadaValues(values))
    }

    /// Classify the given cleartext values.
    ///
    /// This allows getting the totals per value type which is a required parameter when storing values.
    pub fn classify_values(&self, values: &NadaValues) -> NadaValuesClassification {
        let nillion_client_core::values::NadaValuesClassification {
            shares,
            public,
            ecdsa_private_key_shares,
            ecdsa_signature_shares,
        } = self.0.classify_values(&values.0);
        NadaValuesClassification { shares, public, ecdsa_private_key_shares, ecdsa_signature_shares }
    }
}

/// The classification of a set of nada values.
#[wasm_bindgen]
pub struct NadaValuesClassification {
    /// The number of shares
    pub shares: u64,

    /// The number of public values
    pub public: u64,

    /// The number of ecdsa key shares
    pub ecdsa_private_key_shares: u64,

    /// The number of ecdsa signatures shares
    pub ecdsa_signature_shares: u64,
}

/// The shares for a party.
#[wasm_bindgen]
pub struct PartyShares {
    party: PartyId,
    shares: EncryptedNadaValues,
}

#[wasm_bindgen]
impl PartyShares {
    /// Construct a PartyShares instance with the values provided.
    #[wasm_bindgen(constructor)]
    pub fn new(party: PartyId, shares: EncryptedNadaValues) -> JsResult<PartyShares> {
        Ok(PartyShares { party, shares })
    }

    /// Get the party this shares are for.
    #[wasm_bindgen(getter)]
    pub fn party(&self) -> PartyId {
        self.party.clone()
    }

    /// Get the shares.
    #[wasm_bindgen(getter)]
    pub fn shares(&self) -> EncryptedNadaValues {
        self.shares.clone()
    }
}

/// Encode a set of values.
#[wasm_bindgen]
pub fn encode_values(values: &EncryptedNadaValues) -> JsResult<Vec<u8>> {
    let bytes = nillion_client_core::values::encode_values(&values.0)
        .map_err(|e| ValueError::new_err(&format!("failed to encode values: {e}")))?;
    Ok(bytes)
}

/// Decode a set of values.
#[wasm_bindgen]
pub fn decode_values(bincode_bytes: &[u8]) -> JsResult<EncryptedNadaValues> {
    let values = nillion_client_core::values::decode_values(bincode_bytes)
        .map_err(|e| ValueError::new_err(&format!("failed to decode values: {e}")))?;
    Ok(EncryptedNadaValues(values))
}

/// Compute the size of the given values.
#[wasm_bindgen]
pub fn compute_values_size(values: &NadaValues) -> JsResult<u64> {
    let count = nillion_client_core::values::compute_values_size(&values.0)
        .map_err(|e| ValueError::new_err(&format!("failed to encode values: {e}")))?;
    Ok(count)
}

/// A set of encrypted nada values.
#[wasm_bindgen]
#[derive(Clone)]
#[cfg_attr(test, derive(Debug, PartialEq))]
pub struct EncryptedNadaValues(HashMap<String, nillion_client_core::values::NadaValue<Encrypted<Encoded>>>);

#[cfg(test)]
mod test {
    use super::*;
    use wasm_bindgen::JsValue;
    use wasm_bindgen_test::*;

    fn make_masker() -> SecretMasker {
        SecretMasker::new_64_bit_safe_prime(1, vec![PartyId(vec![1]), PartyId(vec![2]), PartyId(vec![3])])
            .map_err(JsValue::from)
            .expect("failed to build masker")
    }

    #[wasm_bindgen_test]
    fn secret_integer() {
        let secret = NadaValue::new_secret_integer("-42").map_err(JsValue::from).unwrap();
        assert_eq!(secret.into_integer().map_err(JsValue::from), Ok("-42".to_string()));
    }

    #[wasm_bindgen_test]
    fn secret_unsigned_integer() {
        let secret = NadaValue::new_secret_unsigned_integer("42").map_err(JsValue::from).unwrap();
        assert_eq!(secret.into_integer().map_err(JsValue::from), Ok("42".to_string()));
    }

    #[wasm_bindgen_test]
    fn secret_blob() {
        let contents = b"hi mom".to_vec();
        let secret = NadaValue::new_secret_blob(contents.clone());
        assert_eq!(secret.into_byte_array().map_err(JsValue::from), Ok(contents));
    }

    #[wasm_bindgen_test]
    fn new_secrets() {
        let secrets = NadaValues::new();
        assert_eq!(secrets.map_err(JsValue::from).unwrap().length(), 0);
    }

    #[wasm_bindgen_test]
    fn insert_secret() {
        let mut secrets = NadaValues::new().map_err(JsValue::from).unwrap();
        let secret = NadaValue::new_secret_integer("42").map_err(JsValue::from).unwrap();
        secrets.insert("my_secret".to_string(), &secret);
        assert_eq!(secrets.length(), 1);
    }

    #[wasm_bindgen_test]
    fn integer() {
        let variable = NadaValue::new_public_integer("-42").map_err(JsValue::from).unwrap();
        assert_eq!(variable.into_integer().map_err(JsValue::from), Ok("-42".to_string()));
    }

    #[wasm_bindgen_test]
    fn unsigned_integer() {
        let variable = NadaValue::new_public_unsigned_integer("42").map_err(JsValue::from).unwrap();
        assert_eq!(variable.into_integer().map_err(JsValue::from), Ok("42".to_string()));
    }

    #[wasm_bindgen_test]
    fn mask_unmask() -> Result<(), JsValue> {
        let mut values = NadaValues::new()?;
        values.insert("a".into(), &NadaValue::new_secret_integer("42")?);
        values.insert("b".into(), &NadaValue::new_secret_blob(vec![1, 2, 3]));
        values.insert("c".into(), &NadaValue::new_secret_unsigned_integer("1337")?);

        let masker = make_masker();
        let masked_values = masker.mask(values.clone())?;
        let unmasked_values = masker.unmask(masked_values)?;
        assert_eq!(unmasked_values, values);
        Ok(())
    }

    #[wasm_bindgen_test]
    fn value_classification() -> Result<(), JsValue> {
        let mut values = NadaValues::new()?;
        values.insert("a".into(), &NadaValue::new_secret_integer("42")?);
        values.insert("b".into(), &NadaValue::new_secret_blob(vec![1, 2, 3]));
        values.insert("c".into(), &NadaValue::new_secret_unsigned_integer("1337")?);
        values.insert("d".into(), &NadaValue::new_public_integer("101")?);

        let masker = make_masker();
        let NadaValuesClassification { shares, public, ecdsa_private_key_shares, ecdsa_signature_shares } =
            masker.classify_values(&values);
        assert_eq!(shares, 3);
        assert_eq!(public, 1);
        assert_eq!(ecdsa_private_key_shares, 0);
        assert_eq!(ecdsa_signature_shares, 0);
        Ok(())
    }

    #[wasm_bindgen_test]
    fn encode_decode() -> Result<(), JsValue> {
        let mut values = NadaValues::new()?;
        values.insert("a".into(), &NadaValue::new_secret_integer("42")?);
        values.insert("b".into(), &NadaValue::new_secret_blob(vec![1, 2, 3]));

        let masker = make_masker();
        let values = masker.mask(values.clone())?.into_iter().next().unwrap().shares;
        let encoded = encode_values(&values)?;
        let decoded = decode_values(&encoded)?;
        assert_eq!(decoded, values);
        Ok(())
    }
}

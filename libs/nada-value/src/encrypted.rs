//! Encrypted types
//!
//! Encrypted types represent the encrypted values of secrets that are used by the Nillion nodes.
//! They are implemented as Shares.
//!
//! Encrypted types can be represented in Encoded format (`<Encoded>`) or in Modular number representation (`<T>`). The generic type
//! `<T>` for Modular numbers is used to represent the prime used in the modular encoding.
use std::{
    collections::{BTreeMap, HashMap},
    marker::PhantomData,
};

use basic_types::{jar::PartyJar, PartyId};
use ecdsa_keypair::{
    privatekey::{EcdsaPrivateKey, EcdsaPrivateKeyShare},
    publickey::EcdsaPublicKeyArray,
    signature::EcdsaSignatureShare,
};
use math_lib::modular::{EncodedModularNumber, ModularNumber, SafePrime};
use shamir_sharing::{
    protocol::PolyDegree,
    secret_sharer::{PartyShares, SafePrimeSecretSharer, SecretSharer, SecretSharerProperties, ShamirSecretSharer},
};

use crate::{
    clear::Clear,
    encoders::blob_chunk_size,
    errors::{ClearToEncryptedError, DecodingError, EncodingError, EncryptedToClearError},
    NadaValue, NeverPrimitiveType,
};
use nada_type::{NadaType, PrimitiveTypes, TypeError};

/// Share generic over the Prime
pub type Share<T> = ModularNumber<T>;
/// Share with Prime encoded
pub type EncodedShare = EncodedModularNumber;

/// Represents a blob: an array of bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "secret-serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BlobPrimitiveType<T> {
    /// Value.
    pub value: Vec<T>,

    /// Size.
    pub unencoded_size: u64,
}

impl<T> BlobPrimitiveType<T> {
    /// Returns a new blob primitive type.
    pub fn new(value: Vec<T>, unencoded_size: u64) -> Self {
        Self { value, unencoded_size }
    }
}

/// Encoded marker struct.
///
/// Marker struct for encoded data types.
#[derive(PartialEq, Eq, Debug, Clone)]
#[cfg_attr(feature = "secret-serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Encoded;

/// NadaValue that are in encrypted form
#[derive(PartialEq, Eq, Debug, Clone)]
#[cfg_attr(feature = "secret-serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Encrypted<T>(PhantomData<T>);

impl PrimitiveTypes for Encrypted<Encoded> {
    // Public variables
    type Integer = EncodedModularNumber;
    type UnsignedInteger = EncodedModularNumber;
    type Boolean = EncodedModularNumber;
    type EcdsaDigestMessage = [u8; 32];
    type EcdsaPublicKey = EcdsaPublicKeyArray;
    type StoreId = [u8; 16];

    // Abstract secrets
    type SecretInteger = NeverPrimitiveType;
    type SecretUnsignedInteger = NeverPrimitiveType;
    type SecretBoolean = NeverPrimitiveType;
    type SecretBlob = BlobPrimitiveType<EncodedShare>;

    // Shares
    type ShamirShareInteger = EncodedShare;
    type ShamirShareUnsignedInteger = EncodedShare;
    type ShamirShareBoolean = EncodedShare;

    // Ecdsa type shares: these shares do not depend on ModularNumber, so the encoded version
    // is the same as the non encoded version. These ecdsa shares do not
    // depend on any generic prime.
    // Ecdsa Private Key
    type EcdsaPrivateKey = EcdsaPrivateKeyShare;

    // Ecdsa Signature
    type EcdsaSignature = EcdsaSignatureShare;
}

impl<T: SafePrime> PrimitiveTypes for Encrypted<T> {
    // Public variables
    type Integer = ModularNumber<T>;
    type UnsignedInteger = ModularNumber<T>;
    type Boolean = ModularNumber<T>;
    type EcdsaDigestMessage = [u8; 32];
    type EcdsaPublicKey = EcdsaPublicKeyArray;
    type StoreId = [u8; 16];

    // Abstract secrets
    type SecretInteger = NeverPrimitiveType;
    type SecretUnsignedInteger = NeverPrimitiveType;
    type SecretBoolean = NeverPrimitiveType;
    type SecretBlob = BlobPrimitiveType<Share<T>>;

    // Shares
    type ShamirShareInteger = Share<T>;
    type ShamirShareUnsignedInteger = Share<T>;
    type ShamirShareBoolean = Share<T>;

    // Ecdsa type shares: these shares do not depend on ModularNumber, so the encoded version
    // is the same as the non encoded version. These ecdsa shares do not
    // depend on any generic prime.
    // Ecdsa Private Key
    type EcdsaPrivateKey = EcdsaPrivateKeyShare;

    // Ecdsa Signature
    type EcdsaSignature = EcdsaSignatureShare;
}

/// Converts a hash map of clear nada values into a party jar or encrypted encoded nada values.
#[allow(clippy::type_complexity)]
pub fn nada_values_clear_to_nada_values_encrypted<T>(
    values: HashMap<String, NadaValue<Clear>>,
    secret_sharer: &ShamirSecretSharer<T>,
) -> Result<PartyJar<HashMap<String, NadaValue<Encrypted<T>>>>, ClearToEncryptedError>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    let mut values_by_party: HashMap<PartyId, HashMap<String, NadaValue<Encrypted<T>>>> = HashMap::new();
    for (input_id, value) in values {
        let encrypted_value = nada_value_clear_to_nada_value_encrypted(&value, secret_sharer)?;

        for (party, value) in encrypted_value.into_elements() {
            let values = values_by_party.entry(party).or_default();
            values.insert(input_id.clone(), value);
        }
    }

    Ok(PartyJar::new_with_elements(values_by_party)?)
}

fn clear_to_flattened_primitive_encrypted<T>(
    value: &NadaValue<Clear>,
    secret_sharer: &ShamirSecretSharer<T>,
) -> Result<Vec<PartyJar<NadaValue<Encrypted<T>>>>, ClearToEncryptedError>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    let mut inner_values = vec![value];
    let mut flattened_values = vec![];
    while let Some(inner_value) = inner_values.pop() {
        match inner_value {
            // Public variables: convert into modular number.
            // Note that we don't officially support storing public variables at the moment.
            NadaValue::Integer(value) => {
                let value = NadaValue::new_integer(ModularNumber::try_from(value)?);
                let party_values = secret_sharer.parties().into_iter().map(|p| (p, value.clone()));
                flattened_values.push(PartyJar::new_with_elements(party_values)?);
            }
            NadaValue::UnsignedInteger(value) => {
                let value = NadaValue::new_unsigned_integer(ModularNumber::try_from(value)?);
                let party_values = secret_sharer.parties().into_iter().map(|p| (p, value.clone()));
                flattened_values.push(PartyJar::new_with_elements(party_values)?);
            }
            NadaValue::Boolean(value) => {
                let value = NadaValue::new_boolean(if *value { ModularNumber::ONE } else { ModularNumber::ZERO });
                let party_values = secret_sharer.parties().into_iter().map(|p| (p, value.clone()));
                flattened_values.push(PartyJar::new_with_elements(party_values)?);
            }
            NadaValue::EcdsaDigestMessage(value) => {
                let value = NadaValue::new_ecdsa_digest_message(*value);
                let party_values = secret_sharer.parties().into_iter().map(|p| (p, value.clone()));
                flattened_values.push(PartyJar::new_with_elements(party_values)?);
            }
            NadaValue::EcdsaPublicKey(value) => {
                let value = NadaValue::new_ecdsa_public_key(value.clone());
                let party_values = secret_sharer.parties().into_iter().map(|p| (p, value.clone()));
                flattened_values.push(PartyJar::new_with_elements(party_values)?);
            }
            NadaValue::StoreId(value) => {
                let value = NadaValue::new_store_id(*value);
                let party_values = secret_sharer.parties().into_iter().map(|p| (p, value.clone()));
                flattened_values.push(PartyJar::new_with_elements(party_values)?);
            }

            // Secrets -> Shares
            NadaValue::SecretInteger(_)
            | NadaValue::SecretUnsignedInteger(_)
            | NadaValue::SecretBoolean(_)
            | NadaValue::SecretBlob(_)
            | NadaValue::EcdsaPrivateKey(_)
            | NadaValue::EcdsaSignature(_) => flattened_values.push(nada_value_to_share(inner_value, secret_sharer)?),

            NadaValue::Array { values, .. } => inner_values.extend(values.iter().rev()),
            NadaValue::Tuple { left, right } => {
                inner_values.push(right.as_ref());
                inner_values.push(left.as_ref());
            }
            NadaValue::NTuple { values } => inner_values.extend(values.iter().rev()),
            NadaValue::Object { values } => inner_values.extend(values.values().rev()),
            NadaValue::ShamirShareInteger(_)
            | NadaValue::ShamirShareUnsignedInteger(_)
            | NadaValue::ShamirShareBoolean(_) => unreachable!(),
        }
    }
    Ok(flattened_values)
}

#[allow(clippy::type_complexity)]
fn generate_party_inner_values<T>(
    count: usize,
    resultant_values: &mut Vec<PartyJar<NadaValue<Encrypted<T>>>>,
) -> Result<HashMap<PartyId, Vec<NadaValue<Encrypted<T>>>>, ClearToEncryptedError>
where
    T: SafePrime,
{
    let mut party_inner_values: HashMap<PartyId, Vec<NadaValue<Encrypted<T>>>> = HashMap::new();
    for _ in 0..count {
        let inner_party_jar = resultant_values.pop().ok_or(ClearToEncryptedError::NotEnoughValues)?;
        for (party_id, inner_value) in inner_party_jar.into_elements() {
            party_inner_values.entry(party_id).or_default().push(inner_value);
        }
    }
    Ok(party_inner_values)
}

/// Converts a clear nada value to an encrypted encoded nada value.
pub fn nada_value_clear_to_nada_value_encrypted<T>(
    value: &NadaValue<Clear>,
    secret_sharer: &ShamirSecretSharer<T>,
) -> Result<PartyJar<NadaValue<Encrypted<T>>>, ClearToEncryptedError>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    let mut flattened_types = value.to_type().flatten_inner_types();
    let mut flattened_values = clear_to_flattened_primitive_encrypted(value, secret_sharer)?;

    // Reconstructs the values from inner primitive values.
    let mut resultant_values = vec![];
    while let Some(ty) = flattened_types.pop() {
        match ty {
            NadaType::Integer
            | NadaType::UnsignedInteger
            | NadaType::Boolean
            | NadaType::EcdsaDigestMessage
            | NadaType::EcdsaPublicKey
            | NadaType::StoreId
            | NadaType::SecretBlob
            | NadaType::SecretInteger
            | NadaType::SecretUnsignedInteger
            | NadaType::SecretBoolean
            | NadaType::EcdsaPrivateKey
            | NadaType::EcdsaSignature => {
                resultant_values.push(flattened_values.pop().ok_or(ClearToEncryptedError::NotEnoughValues)?)
            }
            NadaType::Array { size, .. } => {
                let party_inner_values = generate_party_inner_values(size, &mut resultant_values)?;
                let mut party_jar = PartyJar::new(secret_sharer.party_count());
                for (party, inner_values) in party_inner_values {
                    let inner_type =
                        inner_values.first().map(|value| value.to_type()).ok_or(TypeError::NonEmptyVecOnly)?;
                    party_jar.add_element(party, NadaValue::new_array(inner_type, inner_values)?)?;
                }
                resultant_values.push(party_jar)
            }
            NadaType::Tuple { .. } => {
                let left_party_jar = resultant_values.pop().ok_or(ClearToEncryptedError::NotEnoughValues)?;
                let right_party_jar = resultant_values.pop().ok_or(ClearToEncryptedError::NotEnoughValues)?;
                let mut party_jar = PartyJar::new(secret_sharer.party_count());
                for ((party, left), (_, right)) in left_party_jar.into_elements().zip(right_party_jar.into_elements()) {
                    party_jar.add_element(party, NadaValue::new_tuple(left, right)?)?;
                }
                resultant_values.push(party_jar)
            }
            NadaType::NTuple { types } => {
                let party_inner_values = generate_party_inner_values(types.len(), &mut resultant_values)?;
                let mut party_jar = PartyJar::new(secret_sharer.party_count());
                for (party, inner_values) in party_inner_values {
                    party_jar.add_element(party, NadaValue::new_n_tuple(inner_values)?)?;
                }
                resultant_values.push(party_jar)
            }
            NadaType::Object { types } => {
                let party_inner_values = generate_party_inner_values(types.len(), &mut resultant_values)?;
                let mut party_jar = PartyJar::new(secret_sharer.party_count());
                for (party, inner_values) in party_inner_values {
                    party_jar.add_element(
                        party,
                        NadaValue::new_object(types.keys().cloned().zip(inner_values.into_iter()).collect())?,
                    )?;
                }
                resultant_values.push(party_jar)
            }
            NadaType::ShamirShareInteger | NadaType::ShamirShareUnsignedInteger | NadaType::ShamirShareBoolean => {
                unreachable!()
            }
        }
    }
    resultant_values.pop().ok_or(ClearToEncryptedError::NotEnoughValues)
}

/// Converts a modular clear nada value to a encrypted encode nada value (share only)
pub fn nada_value_to_share<T>(
    value: &NadaValue<Clear>,
    secret_sharer: &ShamirSecretSharer<T>,
) -> Result<PartyJar<NadaValue<Encrypted<T>>>, ClearToEncryptedError>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    match value {
        NadaValue::SecretInteger(value) => {
            let party_shares: PartyShares<Share<T>> =
                secret_sharer.generate_shares(&value.try_into()?, PolyDegree::T)?;
            let mut party_jar = PartyJar::new(secret_sharer.party_count());
            for (party, share) in party_shares.into_iter() {
                let share: NadaValue<Encrypted<T>> = NadaValue::new_shamir_share_integer(share);
                party_jar.add_element(party, share)?;
            }
            Ok(party_jar)
        }
        NadaValue::SecretUnsignedInteger(value) => {
            let party_shares: PartyShares<Share<T>> =
                secret_sharer.generate_shares(&value.try_into()?, PolyDegree::T)?;
            let mut party_jar = PartyJar::new(secret_sharer.party_count());
            for (party, share) in party_shares.into_iter() {
                let share: NadaValue<Encrypted<T>> = NadaValue::new_shamir_share_unsigned_integer(share);
                party_jar.add_element(party, share)?;
            }
            Ok(party_jar)
        }
        NadaValue::SecretBoolean(value) => {
            let value = if *value { ModularNumber::ONE } else { ModularNumber::ZERO };
            let party_shares: PartyShares<Share<T>> = secret_sharer.generate_shares(&value, PolyDegree::T)?;
            let mut party_jar = PartyJar::new(secret_sharer.party_count());
            for (party, share) in party_shares.into_iter() {
                let share: NadaValue<Encrypted<T>> = NadaValue::new_shamir_share_boolean(share);
                party_jar.add_element(party, share)?;
            }
            Ok(party_jar)
        }
        NadaValue::SecretBlob(value) => {
            let chunk_size = blob_chunk_size::<T>();
            let chunks: Result<Vec<_>, _> = value.chunks(chunk_size).map(ModularNumber::<T>::try_from).collect();
            let values = chunks.map_err(|_| EncodingError::PrimeTooSmall)?;
            let unencoded_size = value.len().try_into().map_err(|_| EncodingError::OutOfBounds)?;

            let mut party_jar = PartyJar::new(secret_sharer.party_count());
            let mut party_shares: PartyShares<Vec<Share<T>>> = PartyShares::default();
            for value in values {
                let partial_party_shares: PartyShares<Share<T>> =
                    secret_sharer.generate_shares(&value, PolyDegree::T)?;
                for (party, share) in partial_party_shares {
                    party_shares.entry(party).or_default().push(share);
                }
            }
            for (party, share) in party_shares.into_iter() {
                let share: NadaValue<Encrypted<T>> =
                    NadaValue::new_secret_blob(BlobPrimitiveType::new(share, unencoded_size));
                party_jar.add_element(party, share)?;
            }
            Ok(party_jar)
        }
        NadaValue::EcdsaPrivateKey(value) => {
            let n_usize = secret_sharer.party_count();
            let n_u16 = n_usize.try_into().map_err(|_| ClearToEncryptedError::TooManyPartiesForEcdsaSignature)?;
            let mut party_jar = PartyJar::new(n_usize);

            let party_ids = secret_sharer.parties();
            let shares: Vec<EcdsaPrivateKeyShare> = value.generate_shares(n_u16)?;
            let zipped: Vec<(PartyId, EcdsaPrivateKeyShare)> = party_ids.into_iter().zip(shares).collect();

            // Note: Each [`EcdsaPrivateKeyShare`] corresponds to an indexed party that must consistently align
            // across all signature-related protocols (e.g., distributed key generation, auxiliary information generation,
            // and signature generation). The `.parties()` method ensures that the list of parties is sorted in ascending order.
            // This alignment is maintained across all signature protocols to ensure consistency.
            for (party_id, share) in zipped {
                let share: NadaValue<Encrypted<T>> = NadaValue::new_ecdsa_private_key(share);
                party_jar.add_element(party_id, share)?;
            }

            Ok(party_jar)
        }
        NadaValue::EcdsaSignature(value) => {
            let share_count_usize = secret_sharer.party_count();
            let share_count_u16 =
                share_count_usize.try_into().map_err(|_| ClearToEncryptedError::TooManyPartiesForEcdsaSignature)?;
            let mut party_jar = PartyJar::new(share_count_usize);

            let party_ids = secret_sharer.parties();
            let shares: Vec<EcdsaSignatureShare> = value.generate_shares(share_count_u16)?;
            let zipped: Vec<(PartyId, EcdsaSignatureShare)> = party_ids.into_iter().zip(shares).collect();

            for (party_id, share) in zipped {
                let share: NadaValue<Encrypted<T>> = NadaValue::new_ecdsa_signature(share);
                party_jar.add_element(party_id, share)?;
            }

            Ok(party_jar)
        }
        NadaValue::Integer(_)
        | NadaValue::UnsignedInteger(_)
        | NadaValue::Boolean(_)
        | NadaValue::EcdsaDigestMessage(_)
        | NadaValue::EcdsaPublicKey(_)
        | NadaValue::StoreId(_)
        | NadaValue::ShamirShareInteger(_)
        | NadaValue::ShamirShareUnsignedInteger(_)
        | NadaValue::ShamirShareBoolean(_)
        | NadaValue::Array { .. }
        | NadaValue::Tuple { .. }
        | NadaValue::NTuple { .. }
        | NadaValue::Object { .. } => unreachable!(),
    }
}

/// Map of nada values conversion from encrypted to clear
pub fn nada_values_encrypted_to_nada_values_clear<T>(
    party_values: PartyJar<HashMap<String, NadaValue<Encrypted<Encoded>>>>,
    sharer: &ShamirSecretSharer<T>,
) -> Result<HashMap<String, NadaValue<Clear>>, EncryptedToClearError>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    // Transpose the shares map so the indexing goes from input -> party id -> shares.
    let mut value_fragments: HashMap<String, PartyJar<NadaValue<Encrypted<Encoded>>>> = HashMap::new();
    // collect all shares
    for (party_id, values) in party_values.into_elements() {
        for (input_id, shares) in values {
            value_fragments
                .entry(input_id)
                .or_insert(PartyJar::new(sharer.party_count()))
                .add_element(party_id.clone(), shares)?;
        }
    }

    let mut clear_values = HashMap::new();
    for (input, fragments) in value_fragments {
        clear_values.insert(input, nada_value_encrypted_to_nada_value_clear(fragments, sharer)?);
    }

    Ok(clear_values)
}

/// Nada value conversion from encrypted to clear
pub fn nada_value_encrypted_to_nada_value_clear<T>(
    party_jar: PartyJar<NadaValue<Encrypted<Encoded>>>,
    sharer: &ShamirSecretSharer<T>,
) -> Result<NadaValue<Clear>, EncryptedToClearError>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    let result_ty = party_jar.elements().next().ok_or(EncryptedToClearError::PartyJarEmpty)?.1.to_type();
    let mut flattened_types = result_ty.flatten_inner_types();
    // Transforms the inner values into primitive nada values in clear form
    let mut flattened_clear_values = encrypted_to_flattened_primitive_clear(party_jar, sharer)?;

    // Reconstructs the values from inner primitive values.
    let mut resultant_values = vec![];
    while let Some(ty) = flattened_types.pop() {
        match ty {
            NadaType::Integer
            | NadaType::UnsignedInteger
            | NadaType::Boolean
            | NadaType::EcdsaDigestMessage
            | NadaType::EcdsaPublicKey
            | NadaType::StoreId
            | NadaType::SecretBlob
            | NadaType::ShamirShareInteger
            | NadaType::ShamirShareUnsignedInteger
            | NadaType::ShamirShareBoolean
            | NadaType::EcdsaPrivateKey
            | NadaType::EcdsaSignature => {
                resultant_values.push(flattened_clear_values.pop().ok_or(EncryptedToClearError::NotEnoughValues)?)
            }
            NadaType::Array { size, .. } => {
                let mut inner_values = vec![];
                for _ in 0..size {
                    inner_values.push(resultant_values.pop().ok_or(EncryptedToClearError::NotEnoughValues)?);
                }
                resultant_values.push(NadaValue::new_array_non_empty(inner_values)?)
            }
            NadaType::Tuple { .. } => {
                let right = resultant_values.pop().ok_or(EncryptedToClearError::NotEnoughValues)?;
                let left = resultant_values.pop().ok_or(EncryptedToClearError::NotEnoughValues)?;
                resultant_values.push(NadaValue::new_tuple(left, right)?)
            }
            NadaType::NTuple { types } => {
                let mut inner_values = vec![];
                for _ in 0..types.len() {
                    inner_values.push(resultant_values.pop().ok_or(EncryptedToClearError::NotEnoughValues)?);
                }
                resultant_values.push(NadaValue::new_n_tuple(inner_values)?)
            }
            NadaType::Object { types } => {
                let mut inner_values = vec![];
                for _ in 0..types.len() {
                    inner_values.push(resultant_values.pop().ok_or(EncryptedToClearError::NotEnoughValues)?);
                }
                resultant_values
                    .push(NadaValue::new_object(types.keys().cloned().zip(inner_values.into_iter()).collect())?)
            }

            NadaType::SecretInteger | NadaType::SecretUnsignedInteger | NadaType::SecretBoolean => unreachable!(),
        }
    }
    resultant_values.pop().ok_or(EncryptedToClearError::NotEnoughValues)
}

/// Transforms an encrypted nada value into a list of primitive nada value in clear form. These
/// primitive values represents the inner values of the source value in clear form.
fn encrypted_to_flattened_primitive_clear<T>(
    party_jar: PartyJar<NadaValue<Encrypted<Encoded>>>,
    sharer: &ShamirSecretSharer<T>,
) -> Result<Vec<NadaValue<Clear>>, EncryptedToClearError>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    let mut inner_jars = vec![party_jar];
    let mut flattened_values = vec![];
    while let Some(inner_jar) = inner_jars.pop() {
        let party_values: HashMap<PartyId, NadaValue<Encrypted<Encoded>>> = inner_jar.into_elements().collect();
        let inner_ty = party_values.values().next().ok_or(EncryptedToClearError::PartyJarEmpty)?.to_type();
        match inner_ty {
            NadaType::Integer
            | NadaType::UnsignedInteger
            | NadaType::Boolean
            | NadaType::EcdsaDigestMessage
            | NadaType::EcdsaPublicKey
            | NadaType::StoreId => {
                flattened_values.push(encrypted_values_to_public_variable(party_values.into_values())?);
            }
            NadaType::SecretBlob => flattened_values.push(encrypted_values_to_secret_blob(party_values, sharer)?),
            NadaType::ShamirShareInteger => {
                flattened_values.push(encrypted_values_to_secret(party_values, sharer, NadaType::SecretInteger)?)
            }
            NadaType::ShamirShareUnsignedInteger => flattened_values.push(encrypted_values_to_secret(
                party_values,
                sharer,
                NadaType::SecretUnsignedInteger,
            )?),
            NadaType::ShamirShareBoolean => {
                flattened_values.push(encrypted_values_to_secret(party_values, sharer, NadaType::SecretBoolean)?)
            }
            NadaType::EcdsaPrivateKey => flattened_values.push(encrypted_values_to_ecdsa_private_key(party_values)?),
            NadaType::EcdsaSignature => flattened_values.push(encrypted_values_to_ecdsa_signature(party_values)?),

            NadaType::Array { .. } => {
                // The map of party elements for each element of the array. The key corresponds to the index
                // We will use this map to generate the different party jars
                let mut party_elements_map = BTreeMap::new();
                // collect all the elements for the party jars.
                let party_jar_size = party_values.len();
                for (party_id, party_value) in party_values {
                    let NadaValue::Array { values, .. } = party_value else {
                        return Err(EncryptedToClearError::InvalidType(party_value.to_type()));
                    };
                    for (index, value) in values.into_iter().enumerate() {
                        party_elements_map
                            .entry(index)
                            .or_insert(PartyJar::new(party_jar_size))
                            .add_element(party_id.clone(), value)?;
                    }
                }
                inner_jars.extend(party_elements_map.into_values().rev());
            }
            NadaType::Tuple { .. } => {
                let mut party_jar_left: PartyJar<NadaValue<Encrypted<Encoded>>> = PartyJar::new(party_values.len());
                let mut party_jar_right: PartyJar<NadaValue<Encrypted<Encoded>>> = PartyJar::new(party_values.len());
                for (party_id, value) in party_values {
                    let NadaValue::Tuple { left, right } = value else {
                        return Err(EncryptedToClearError::InvalidType(value.to_type()));
                    };
                    party_jar_left.add_element(party_id.clone(), *left)?;
                    party_jar_right.add_element(party_id.clone(), *right)?;
                }
                inner_jars.push(party_jar_left);
                inner_jars.push(party_jar_right);
            }
            NadaType::NTuple { .. } => {
                // The map of party elements for each element of the ntuple. The key corresponds to the index
                // We will use this map to generate the different party jars
                let mut party_elements_map = BTreeMap::new();
                // collect all the elements for the party jars.
                let party_jar_size = party_values.len();
                for (party_id, party_value) in party_values {
                    let NadaValue::NTuple { values } = party_value else {
                        return Err(EncryptedToClearError::InvalidType(party_value.to_type()));
                    };
                    for (index, value) in values.into_iter().enumerate() {
                        party_elements_map
                            .entry(index)
                            .or_insert(PartyJar::new(party_jar_size))
                            .add_element(party_id.clone(), value)?;
                    }
                }
                inner_jars.extend(party_elements_map.into_values().rev());
            }
            NadaType::Object { .. } => {
                // The map of party elements for each element of the ntuple. The key corresponds to the index
                // We will use this map to generate the different party jars
                let mut party_elements_map = BTreeMap::new();
                // collect all the elements for the party jars.
                let party_jar_size = party_values.len();
                for (party_id, party_value) in party_values {
                    let NadaValue::Object { values } = party_value else {
                        return Err(EncryptedToClearError::InvalidType(party_value.to_type()));
                    };
                    for (index, value) in values.into_values().enumerate() {
                        party_elements_map
                            .entry(index)
                            .or_insert(PartyJar::new(party_jar_size))
                            .add_element(party_id.clone(), value)?;
                    }
                }
                inner_jars.extend(party_elements_map.into_values().rev());
            }
            NadaType::SecretInteger | NadaType::SecretUnsignedInteger | NadaType::SecretBoolean => unreachable!(),
        }
    }
    Ok(flattened_values)
}

/// Transforms an iterator of `NadaValue<Encrypted<Encoded>>` into a `NadaValue<Clear>`
fn encrypted_values_to_public_variable<T, I>(values: I) -> Result<NadaValue<Clear>, EncryptedToClearError>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    I: IntoIterator<Item = NadaValue<Encrypted<Encoded>>>,
{
    let mut values = values.into_iter();
    let Some(value) = values.next() else { return Err(EncryptedToClearError::PartyValuesNotFound) };
    let ty = value.to_type();
    // This checks that all nodes return the same public value for consistency
    if !values.all(|v| v == value) {
        // Nodes should not return different values, otherwise it means at least one node is compromised
        return Err(EncryptedToClearError::Missmatch(ty.to_string()));
    }
    match value {
        NadaValue::Integer(value) => Ok(NadaValue::new_integer(&value.try_decode::<T>()?)),
        NadaValue::UnsignedInteger(value) => Ok(NadaValue::new_unsigned_integer(&value.try_decode::<T>()?)),
        NadaValue::Boolean(value) => Ok(NadaValue::new_boolean(!value.try_decode::<T>()?.is_zero())),
        NadaValue::EcdsaDigestMessage(value) => Ok(NadaValue::new_ecdsa_digest_message(value)),
        NadaValue::EcdsaPublicKey(value) => Ok(NadaValue::new_ecdsa_public_key(value)),
        NadaValue::StoreId(value) => Ok(NadaValue::new_store_id(value)),
        NadaValue::SecretInteger(_)
        | NadaValue::SecretUnsignedInteger(_)
        | NadaValue::SecretBoolean(_)
        | NadaValue::SecretBlob(_)
        | NadaValue::ShamirShareInteger(_)
        | NadaValue::ShamirShareUnsignedInteger(_)
        | NadaValue::ShamirShareBoolean(_)
        | NadaValue::Array { .. }
        | NadaValue::Tuple { .. }
        | NadaValue::EcdsaPrivateKey(_)
        | NadaValue::NTuple { .. }
        | NadaValue::Object { .. }
        | NadaValue::EcdsaSignature(_) => Err(EncryptedToClearError::InvalidType(ty)),
    }
}

/// Transforms an encrypted shamir share into a secret
fn encrypted_values_to_secret<T>(
    party_values: HashMap<PartyId, NadaValue<Encrypted<Encoded>>>,
    sharer: &ShamirSecretSharer<T>,
    result_type: NadaType,
) -> Result<NadaValue<Clear>, EncryptedToClearError>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    let mut shares = vec![];
    for (party_id, value) in party_values {
        let alpha_share = match value {
            NadaValue::ShamirShareInteger(alpha_share)
            | NadaValue::ShamirShareUnsignedInteger(alpha_share)
            | NadaValue::ShamirShareBoolean(alpha_share) => alpha_share,
            NadaValue::Integer(_)
            | NadaValue::UnsignedInteger(_)
            | NadaValue::Boolean(_)
            | NadaValue::EcdsaDigestMessage(_)
            | NadaValue::EcdsaPublicKey(_)
            | NadaValue::StoreId(_)
            | NadaValue::SecretInteger(_)
            | NadaValue::SecretUnsignedInteger(_)
            | NadaValue::SecretBoolean(_)
            | NadaValue::SecretBlob(_)
            | NadaValue::Array { .. }
            | NadaValue::Tuple { .. }
            | NadaValue::EcdsaPrivateKey(_)
            | NadaValue::NTuple { .. }
            | NadaValue::Object { .. }
            | NadaValue::EcdsaSignature(_) => return Err(EncryptedToClearError::InvalidType(value.to_type())),
        };
        shares.push((party_id, ModularNumber::<T>::try_from_encoded(&alpha_share)?));
    }
    let value =
        sharer.recover(shares).map_err(|_| EncryptedToClearError::SharedSecretRecovery(result_type.to_string()))?;
    match result_type {
        NadaType::SecretInteger => Ok(NadaValue::new_secret_integer(&value)),
        NadaType::SecretUnsignedInteger => Ok(NadaValue::new_secret_unsigned_integer(&value)),
        NadaType::SecretBoolean => Ok(NadaValue::new_secret_boolean(!value.is_zero())),
        NadaType::Integer
        | NadaType::UnsignedInteger
        | NadaType::Boolean
        | NadaType::EcdsaDigestMessage
        | NadaType::EcdsaPublicKey
        | NadaType::StoreId
        | NadaType::SecretBlob
        | NadaType::ShamirShareInteger
        | NadaType::ShamirShareUnsignedInteger
        | NadaType::ShamirShareBoolean
        | NadaType::Array { .. }
        | NadaType::Tuple { .. }
        | NadaType::EcdsaPrivateKey
        | NadaType::NTuple { .. }
        | NadaType::Object { .. }
        | NadaType::EcdsaSignature => Err(EncryptedToClearError::InvalidType(result_type)),
    }
}

/// Transforms ecdsa private key shares into a private ecdsa key
fn encrypted_values_to_ecdsa_private_key(
    values: HashMap<PartyId, NadaValue<Encrypted<Encoded>>>,
) -> Result<NadaValue<Clear>, EncryptedToClearError> {
    let mut ecdsa_private_key_shares = vec![];
    for ecdsa_private_key_share in values.into_values() {
        match ecdsa_private_key_share {
            NadaValue::EcdsaPrivateKey(share) => ecdsa_private_key_shares.push(share),
            _ => return Err(EncryptedToClearError::InvalidType(ecdsa_private_key_share.to_type())),
        }
    }

    let ecdsa_private_key = EcdsaPrivateKey::recover(ecdsa_private_key_shares)
        .map_err(|_| EncryptedToClearError::SharedSecretRecovery(NadaType::EcdsaPrivateKey.to_string()))?;
    Ok(NadaValue::new_ecdsa_private_key(ecdsa_private_key))
}

/// Transforms ecdsa signature shares into an ecdsa signature
fn encrypted_values_to_ecdsa_signature(
    values: HashMap<PartyId, NadaValue<Encrypted<Encoded>>>,
) -> Result<NadaValue<Clear>, EncryptedToClearError> {
    let mut ecdsa_signature_shares = vec![];
    for ecdsa_signature_share in values.into_values() {
        match ecdsa_signature_share {
            NadaValue::EcdsaSignature(share) => ecdsa_signature_shares.push(share),
            _ => return Err(EncryptedToClearError::InvalidType(ecdsa_signature_share.to_type())),
        }
    }

    let ecdsa_signature = EcdsaSignatureShare::recover(&ecdsa_signature_shares)
        .ok_or(EncryptedToClearError::SharedSecretRecovery(NadaType::EcdsaSignature.to_string()))?;
    Ok(NadaValue::new_ecdsa_signature(ecdsa_signature))
}

/// Transforms an encrypted value into a secret blob
fn encrypted_values_to_secret_blob<T>(
    values: HashMap<PartyId, NadaValue<Encrypted<Encoded>>>,
    sharer: &ShamirSecretSharer<T>,
) -> Result<NadaValue<Clear>, EncryptedToClearError>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    let Some(NadaValue::SecretBlob(first_blob)) = values.values().next() else {
        return Err(EncryptedToClearError::PartyValuesNotFound);
    };
    let mut remaining = first_blob.unencoded_size;
    let mut blob_shares = vec![vec![]; first_blob.value.len()];

    for (party_id, blob) in values {
        let NadaValue::SecretBlob(blob) = blob else {
            return Err(EncryptedToClearError::InvalidType(blob.to_type()));
        };
        for (idx, share) in blob.value.iter().enumerate() {
            blob_shares
                .get_mut(idx)
                .ok_or(EncryptedToClearError::WrongBlobSize)?
                .push((party_id.clone(), ModularNumber::<T>::try_from_encoded(share)?));
        }
    }

    // Transforms the encrypted value into a blob secret.
    let blob_chunks_mod = blob_shares
        .into_iter()
        .map(|chunk_shares| {
            sharer.recover(chunk_shares).map_err(|_| EncryptedToClearError::SharedSecretRecovery(String::from("blob")))
        })
        .collect::<Result<Vec<ModularNumber<T>>, _>>()?;

    let mut decoded_array: Vec<u8> = vec![];
    let max_chunk_size = blob_chunk_size::<T>();
    for chunk in &blob_chunks_mod {
        let chunk: Vec<u8> = chunk.into();
        let chunk_size = remaining.min(max_chunk_size.try_into().map_err(|_| DecodingError::OutOfBounds)?);
        decoded_array.extend(chunk.iter().take(chunk_size.try_into().map_err(|_| DecodingError::OutOfBounds)?));
        remaining = remaining.wrapping_sub(chunk_size);
    }
    Ok(NadaValue::SecretBlob(decoded_array))
}

#[cfg(test)]
mod tests {
    use crate::{
        clear::Clear,
        encoders::Encoder,
        encrypted::{
            nada_value_clear_to_nada_value_encrypted, nada_value_encrypted_to_nada_value_clear, nada_value_to_share,
            BlobPrimitiveType,
        },
        NadaValue,
    };
    use anyhow::Error;
    use basic_types::PartyId;
    use ecdsa_keypair::{privatekey::EcdsaPrivateKey, publickey::EcdsaPublicKeyArray, signature::EcdsaSignature};
    use generic_ec::{curves::Secp256k1, NonZero, Scalar, SecretScalar};
    use math_lib::modular::{ModularNumber, U64SafePrime};
    use rand_chacha::rand_core::OsRng;
    use rstest::rstest;
    use shamir_sharing::secret_sharer::{test_secret_sharer, ShamirSecretSharer};

    type Prime = U64SafePrime;

    pub fn secret_sharer() -> ShamirSecretSharer<U64SafePrime> {
        let local_party_id = PartyId::from(10);
        let parties = vec![local_party_id.clone(), PartyId::from(20), PartyId::from(30)];
        ShamirSecretSharer::<U64SafePrime>::new(local_party_id, 1, parties).unwrap()
    }

    #[test]
    fn integer_to_encrypted() -> Result<(), Error> {
        let secret_sharer = test_secret_sharer::<Prime>();
        let value: NadaValue<Clear> = NadaValue::new_integer(4);
        let party_jar = nada_value_clear_to_nada_value_encrypted(&value, &secret_sharer)?;
        for (_, value) in party_jar.into_elements() {
            assert_eq!(value, NadaValue::Integer(ModularNumber::from_u64(4)));
        }
        Ok(())
    }

    #[test]
    fn unsigned_integer_to_encrypted() -> Result<(), Error> {
        let secret_sharer = test_secret_sharer::<Prime>();

        let value: NadaValue<Clear> = NadaValue::new_unsigned_integer(4u64);
        let party_jar = nada_value_clear_to_nada_value_encrypted(&value, &secret_sharer)?;
        for (_, value) in party_jar.into_elements() {
            assert_eq!(value, NadaValue::UnsignedInteger(ModularNumber::from_u64(4)));
        }
        Ok(())
    }

    #[test]
    fn boolean_to_encrypted() -> Result<(), Error> {
        let secret_sharer = test_secret_sharer::<Prime>();

        let value: NadaValue<Clear> = NadaValue::new_boolean(true);
        let party_jar = nada_value_clear_to_nada_value_encrypted(&value, &secret_sharer)?;
        for (_, value) in party_jar.into_elements() {
            assert_eq!(value, NadaValue::Boolean(ModularNumber::from_u64(1)));
        }
        Ok(())
    }

    #[test]
    fn secret_integer_to_encrypted() -> Result<(), Error> {
        let secret_sharer = test_secret_sharer::<Prime>();

        let value: NadaValue<Clear> = NadaValue::new_secret_integer(4);
        let party_jar = nada_value_clear_to_nada_value_encrypted(&value, &secret_sharer)?;
        for (_, value) in party_jar.into_elements() {
            assert!(matches!(value, NadaValue::ShamirShareInteger(_)));
        }
        Ok(())
    }

    #[test]
    fn secret_unsigned_integer_to_encrypted() -> Result<(), Error> {
        let secret_sharer = test_secret_sharer::<Prime>();
        let value: NadaValue<Clear> = NadaValue::new_secret_unsigned_integer(4u64);
        let party_jar = nada_value_clear_to_nada_value_encrypted(&value, &secret_sharer)?;
        for (_, value) in party_jar.into_elements() {
            assert!(matches!(value, NadaValue::ShamirShareUnsignedInteger(_)));
        }
        Ok(())
    }

    #[test]
    fn secret_ecdsa_key_to_encrypted() -> Result<(), Error> {
        let mut csprng = OsRng;
        let secret_sharer = test_secret_sharer::<Prime>();

        // Create ecdsa private key Nada value
        let sk = SecretScalar::<Secp256k1>::random(&mut csprng);
        let ecdsa_sk = EcdsaPrivateKey::from_scalar(sk).unwrap();
        let ecdsa_sk = NadaValue::new_ecdsa_private_key(ecdsa_sk);

        let party_jar = nada_value_clear_to_nada_value_encrypted(&ecdsa_sk, &secret_sharer)?;
        for (_, value) in party_jar.into_elements() {
            assert!(matches!(value, NadaValue::EcdsaPrivateKey(_)));
        }
        Ok(())
    }

    #[test]
    fn secret_ecdsa_signature_to_encrypted() -> Result<(), Error> {
        let mut csprng = OsRng;
        let secret_sharer = test_secret_sharer::<Prime>();

        // Create ecdsa signature Nada value
        let r = NonZero::from_scalar(Scalar::<Secp256k1>::random(&mut csprng)).unwrap();
        let s = NonZero::from_scalar(Scalar::<Secp256k1>::random(&mut csprng)).unwrap();
        let ecdsa_sig = EcdsaSignature { r, s };
        let ecdsa_sig = NadaValue::new_ecdsa_signature(ecdsa_sig);

        let party_jar = nada_value_clear_to_nada_value_encrypted(&ecdsa_sig, &secret_sharer)?;
        for (_, value) in party_jar.into_elements() {
            assert!(matches!(value, NadaValue::EcdsaSignature(_)));
        }
        Ok(())
    }

    #[test]
    fn secret_boolean_to_encrypted() -> Result<(), Error> {
        let secret_sharer = test_secret_sharer::<Prime>();
        let value: NadaValue<Clear> = NadaValue::new_secret_boolean(true);
        let party_jar = nada_value_clear_to_nada_value_encrypted(&value, &secret_sharer)?;
        for (_, value) in party_jar.into_elements() {
            assert!(matches!(value, NadaValue::ShamirShareBoolean(_)));
        }
        Ok(())
    }

    #[test]
    fn secret_blob_to_encrypted() -> Result<(), Error> {
        let secret_sharer = test_secret_sharer::<Prime>();
        let value: NadaValue<Clear> = NadaValue::new_secret_blob(vec![45, 13, 189]);
        let party_jar = nada_value_clear_to_nada_value_encrypted(&value, &secret_sharer)?;
        for (_, value) in party_jar.into_elements() {
            assert!(matches!(&value, NadaValue::SecretBlob(BlobPrimitiveType { .. })));
            let NadaValue::SecretBlob(blob_primitive_type) = value else {
                unreachable!();
            };
            assert_eq!(blob_primitive_type.value.len(), 1);
            assert_eq!(blob_primitive_type.unencoded_size, 3);
        }
        Ok(())
    }

    #[test]
    fn ecdsa_digest_msg_to_encrypted() -> Result<(), Error> {
        let secret_sharer = test_secret_sharer::<Prime>();
        let digest_value = [
            1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29,
            30, 31, 32,
        ];
        let value: NadaValue<Clear> = NadaValue::new_ecdsa_digest_message(digest_value);
        let party_jar = nada_value_clear_to_nada_value_encrypted(&value, &secret_sharer)?;
        for (_, value) in party_jar.into_elements() {
            let NadaValue::EcdsaDigestMessage(digest_primitive_type) = value else {
                panic!("not an ecdsa digest message");
            };
            assert_eq!(digest_primitive_type.len(), 32);
        }
        Ok(())
    }

    #[test]
    fn ecdsa_public_key_to_encrypted() -> Result<(), Error> {
        let secret_sharer = test_secret_sharer::<Prime>();
        let value: NadaValue<Clear> = NadaValue::new_ecdsa_public_key(EcdsaPublicKeyArray([42; 33]));
        let party_jar = nada_value_clear_to_nada_value_encrypted(&value, &secret_sharer)?;
        for (_, value) in party_jar.into_elements() {
            let NadaValue::EcdsaPublicKey(ecdsa_public_key_primitive_type) = value else {
                panic!("not an ecdsa public key");
            };
            assert_eq!(ecdsa_public_key_primitive_type.0.len(), 33);
        }
        Ok(())
    }

    #[test]
    fn store_id_to_encrypted() -> Result<(), Error> {
        let secret_sharer = test_secret_sharer::<Prime>();
        let value: NadaValue<Clear> = NadaValue::new_store_id([42; 16]);
        let party_jar = nada_value_clear_to_nada_value_encrypted(&value, &secret_sharer)?;
        for (_, value) in party_jar.into_elements() {
            let NadaValue::StoreId(store_id_primitive_type) = value else {
                panic!("not a store id");
            };
            assert_eq!(store_id_primitive_type.len(), 16);
        }
        Ok(())
    }

    #[rstest]
    #[case::integer(NadaValue::new_secret_integer(1))]
    #[case::unsigned_integer(NadaValue::new_secret_unsigned_integer(1u32))]
    #[case::boolean(NadaValue::new_secret_boolean(true))]
    fn encrypted_to_clear_shares(#[case] clear_value: NadaValue<Clear>) -> Result<(), Error> {
        let secret_sharer = secret_sharer();
        let encrypted = nada_value_to_share(&clear_value, &secret_sharer)?.encode::<Prime>()?;
        let decrypted = nada_value_encrypted_to_nada_value_clear(encrypted, &secret_sharer)?;
        assert_eq!(clear_value, decrypted);
        Ok(())
    }

    #[test]
    fn encrypted_to_clear_ecdsa_key_shares() -> Result<(), Error> {
        let mut csprng = OsRng;

        // Create ecdsa private key
        let sk = SecretScalar::<Secp256k1>::random(&mut csprng);
        let ecdsa_sk = EcdsaPrivateKey::from_scalar(sk).unwrap();
        let clear_value_ecdsa_sk = NadaValue::new_ecdsa_private_key(ecdsa_sk);

        let secret_sharer = secret_sharer();
        let encrypted = nada_value_to_share(&clear_value_ecdsa_sk, &secret_sharer)?.encode::<Prime>()?;
        let decrypted = nada_value_encrypted_to_nada_value_clear(encrypted, &secret_sharer)?;
        assert_eq!(clear_value_ecdsa_sk, decrypted);
        Ok(())
    }

    #[test]
    fn encrypted_to_clear_ecdsa_signature_shares() -> Result<(), Error> {
        let mut csprng = OsRng;

        // Create ecdsa signature
        let r = NonZero::from_scalar(Scalar::<Secp256k1>::random(&mut csprng)).unwrap();
        let s = NonZero::from_scalar(Scalar::<Secp256k1>::random(&mut csprng)).unwrap();
        let ecdsa_sig = EcdsaSignature { r, s }.normalize_s();
        let clear_value_ecdsa_sig = NadaValue::new_ecdsa_signature(ecdsa_sig);

        let secret_sharer = secret_sharer();
        let encrypted = nada_value_to_share(&clear_value_ecdsa_sig, &secret_sharer)?.encode::<Prime>()?;
        let decrypted = nada_value_encrypted_to_nada_value_clear(encrypted, &secret_sharer)?;
        assert_eq!(clear_value_ecdsa_sig, decrypted);
        Ok(())
    }
}

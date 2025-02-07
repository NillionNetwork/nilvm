//! Value encoding utilities.

use math_lib::modular::{SafePrime, U128SafePrime, U256SafePrime, U64SafePrime};
use nada_value::{
    encoders::EncodableWithP,
    encrypted::{nada_values_clear_to_nada_values_encrypted, nada_values_encrypted_to_nada_values_clear},
};
use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, SecretSharerProperties, ShamirSecretSharer};
use std::{collections::HashMap, sync::Arc};

pub use basic_types::{
    jar::{DuplicatePartyShare, PartyJar},
    InvalidPartyId, PartyId,
};
pub use math_lib::modular::{EncodedModularNumber, EncodedModulo};
pub use nada_value::{
    classify::NadaValuesClassification,
    clear::Clear,
    encrypted::{BlobPrimitiveType, Encoded, Encrypted},
    errors::{ClearToEncryptedError, EncryptedToClearError},
    BigInt, BigUint, NadaType, NadaValue, TypeError,
};
pub use shamir_sharing::{protocol::ShamirError, secret_sharer::PartyShares};

/// A set of encrypted values.
pub type EncryptedValues = HashMap<String, NadaValue<Encrypted<Encoded>>>;

/// A set of values in cleartext.
pub type CleartextValues = HashMap<String, NadaValue<Clear>>;

/// Decode a set of values.
#[cfg(feature = "bincode-serde")]
pub fn decode_values(bincode_bytes: &[u8]) -> Result<EncryptedValues, encoding::codec::DecodeError> {
    encoding::codec::MessageCodec.decode(bincode_bytes)
}

/// Encode a set of values.
#[cfg(feature = "bincode-serde")]
pub fn encode_values(values: &EncryptedValues) -> Result<Vec<u8>, encoding::codec::EncodeError> {
    encoding::codec::MessageCodec.encode(values)
}

/// Compute the size of the encoded values.
#[cfg(feature = "bincode-serde")]
pub fn compute_values_size(values: &CleartextValues) -> Result<u64, encoding::codec::EncodeError> {
    encoding::codec::MessageCodec.encoded_size(values)
}

/// A secret masker.
///
/// This allows masking and unmasking secrets.
#[derive(Clone)]
pub struct SecretMasker {
    masker: Arc<dyn MaskUnmask>,
}

impl SecretMasker {
    /// Construct a new masker that uses a 64 bit safe prime under the hood.
    pub fn new_64_bit_safe_prime(polynomial_degree: u64, parties: Vec<PartyId>) -> Result<Self, ShamirError> {
        Self::new::<U64SafePrime>(polynomial_degree, parties)
    }

    /// Construct a new masker that uses a 128 bit safe prime under the hood.
    pub fn new_128_bit_safe_prime(polynomial_degree: u64, parties: Vec<PartyId>) -> Result<Self, ShamirError> {
        Self::new::<U128SafePrime>(polynomial_degree, parties)
    }

    /// Construct a new masker that uses a 256 bit safe prime under the hood.
    pub fn new_256_bit_safe_prime(polynomial_degree: u64, parties: Vec<PartyId>) -> Result<Self, ShamirError> {
        Self::new::<U256SafePrime>(polynomial_degree, parties)
    }

    fn new<T>(polynomial_degree: u64, parties: Vec<PartyId>) -> Result<Self, ShamirError>
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        let sharer = ShamirSecretSharer::<T>::new(PartyId::from(vec![]), polynomial_degree, parties)?;
        let masker: Arc<dyn MaskUnmask> = Arc::new(ShamirProxy { sharer });
        Ok(Self { masker })
    }

    /// Mask a set of values.
    pub fn mask(&self, values: CleartextValues) -> Result<PartyShares<EncryptedValues>, ClearToEncryptedError> {
        self.masker.mask(values)
    }

    /// Unmask a set of values.
    pub fn unmask(&self, jar: PartyJar<EncryptedValues>) -> Result<CleartextValues, EncryptedToClearError> {
        self.masker.unmask(jar)
    }

    /// Classify the given cleartext values.
    ///
    /// This allows getting the totals per value type which is a required parameter when storing values.
    pub fn classify_values(&self, values: &CleartextValues) -> NadaValuesClassification {
        self.masker.classify_values(values)
    }
}

struct ShamirProxy<T: SafePrime> {
    sharer: ShamirSecretSharer<T>,
}

trait MaskUnmask: Send + Sync + 'static {
    fn mask(&self, values: CleartextValues) -> Result<PartyShares<EncryptedValues>, ClearToEncryptedError>;

    fn unmask(
        &self,
        jar: PartyJar<EncryptedValues>,
    ) -> Result<HashMap<String, NadaValue<Clear>>, EncryptedToClearError>;

    fn classify_values(&self, values: &CleartextValues) -> NadaValuesClassification;
}

impl<T> MaskUnmask for ShamirProxy<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    fn mask(
        &self,
        values: CleartextValues,
    ) -> Result<PartyShares<HashMap<String, NadaValue<Encrypted<Encoded>>>>, ClearToEncryptedError> {
        // if we have no values, still return a map with each party as a key. otherwise the client
        // needs to handle this case in a special way
        if values.is_empty() {
            return Ok(self.sharer.parties().into_iter().map(|p| (p, Default::default())).collect());
        }

        let encrypted = nada_values_clear_to_nada_values_encrypted::<T>(values, &self.sharer)?;
        let mut output = PartyShares::default();
        for (party, values) in encrypted.into_elements() {
            let values = values.encode()?;
            output.insert(party, values);
        }
        Ok(output)
    }

    fn unmask(&self, jar: PartyJar<EncryptedValues>) -> Result<CleartextValues, EncryptedToClearError> {
        nada_values_encrypted_to_nada_values_clear(jar, &self.sharer)
    }

    fn classify_values(&self, values: &CleartextValues) -> NadaValuesClassification {
        NadaValuesClassification::new_from_clear::<T>(values)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ecdsa_keypair::privatekey::EcdsaPrivateKey;
    use generic_ec::{curves::Secp256k1, SecretScalar};
    use rand_chacha::rand_core::OsRng;

    fn make_masker() -> SecretMasker {
        SecretMasker::new_64_bit_safe_prime(
            1,
            vec![PartyId::from(vec![1]), PartyId::from(vec![2]), PartyId::from(vec![3])],
        )
        .expect("failed to build masker")
    }

    #[test]
    fn mask_unmask_shares() {
        let values = HashMap::from([
            ("a".into(), NadaValue::new_secret_integer(42)),
            ("b".into(), NadaValue::new_secret_blob(vec![1, 2, 3])),
            ("c".into(), NadaValue::new_secret_boolean(true)),
            ("d".into(), NadaValue::new_secret_unsigned_integer(1337u32)),
            (
                "e".into(),
                NadaValue::new_array(
                    NadaType::SecretInteger,
                    vec![NadaValue::new_secret_integer(100), NadaValue::new_secret_integer(200)],
                )
                .unwrap(),
            ),
        ]);
        let masker = make_masker();
        let masked_values = masker.mask(values.clone()).expect("failed to mask");
        let unmasked_values =
            masker.unmask(PartyJar::new_with_elements(masked_values).unwrap()).expect("failed to unmask");
        assert_eq!(unmasked_values, values);
    }

    #[test]
    fn mask_unmask_ecdsa_keys() {
        let mut values = HashMap::new();

        let mut csprng = OsRng;
        let sk = SecretScalar::<Secp256k1>::random(&mut csprng);
        let ecdsa_sk = EcdsaPrivateKey::from_scalar(sk).unwrap();

        values.insert(values.len().to_string(), NadaValue::<Clear>::new_ecdsa_private_key(ecdsa_sk));
        let masker = make_masker();
        let masked_values = masker.mask(values.clone()).expect("failed to mask");
        let unmasked_values =
            masker.unmask(PartyJar::new_with_elements(masked_values).unwrap()).expect("failed to unmask");
        assert_eq!(unmasked_values, values);
    }

    #[test]
    fn mask_unmask_ecdsa_signatures() {
        let mut values = HashMap::new();

        let mut csprng = OsRng;
        let sk = SecretScalar::<Secp256k1>::random(&mut csprng);
        let ecdsa_sk = EcdsaPrivateKey::from_scalar(sk).unwrap();

        values.insert(values.len().to_string(), NadaValue::<Clear>::new_ecdsa_private_key(ecdsa_sk));
        let masker = make_masker();
        let masked_values = masker.mask(values.clone()).expect("failed to mask");
        let unmasked_values =
            masker.unmask(PartyJar::new_with_elements(masked_values).unwrap()).expect("failed to unmask");
        assert_eq!(unmasked_values, values);
    }
}

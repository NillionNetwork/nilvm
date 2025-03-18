//! The threshold private key implementation.

use crate::{publickey::ThresholdPublicKey, PRIVATE_KEY_LENGTH};
use generic_ec::{errors::InvalidScalar, Curve, NonZero, Scalar, SecretScalar};
use key_share::{
    self,
    trusted_dealer::{self, TrustedDealerError},
    CoreKeyShare, ReconstructError,
};
use rand::rngs::OsRng;
use std::{cmp::PartialEq, fmt};
use subtle::ConstantTimeEq;
use thiserror::Error;

/// A struct representing a private key.
/// The private key is a non-zero scalar defined on an elliptic curve E.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(bound = ""))]
pub struct ThresholdPrivateKey<E: Curve>(NonZero<SecretScalar<E>>);

/// A struct representing an threshold private key share.
///
/// In the context of distributed key generation (DKG) or threshold signing,
/// the private key can be split into multiple shares, and each participant holds one share.
/// The shares of the private key are a non-zero scalar defined on the  elliptic curve.
#[derive(Clone)]
pub struct ThresholdPrivateKeyShare<E: Curve>(CoreKeyShare<E>);

impl<E: Curve> ThresholdPrivateKeyShare<E> {
    /// Public constructor from [`CoreKeyShare`].
    pub fn new(key_share: CoreKeyShare<E>) -> Self {
        Self(key_share)
    }

    /// Returns a reference to the inner `CoreKeyShare<E>` value.
    pub fn as_inner(&self) -> &CoreKeyShare<E> {
        &self.0
    }

    /// Consumes the `ThresholdPrivateKeyShare` and returns the inner `CoreKeyShare<E>`.
    pub fn into_inner(self) -> CoreKeyShare<E> {
        self.0
    }
}

impl<E: Curve> From<CoreKeyShare<E>> for ThresholdPrivateKeyShare<E> {
    fn from(key_share: CoreKeyShare<E>) -> Self {
        Self(key_share)
    }
}

#[cfg(feature = "serde")]
mod details {
    use super::ThresholdPrivateKeyShare;
    use generic_ec::Curve;
    use key_share::CoreKeyShare;
    use serde::{Deserialize, Serialize};
    use std::fmt;

    impl<E: Curve> Serialize for ThresholdPrivateKeyShare<E> {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            use serde::ser::SerializeSeq;
            let bson_byte_array =
                bson::to_vec(&self.0).map_err(|e| serde::ser::Error::custom(format!("Bson serialization: {e}")))?;
            let mut seq = serializer.serialize_seq(Some(bson_byte_array.len()))?;
            for e in &bson_byte_array {
                seq.serialize_element(e)?;
            }
            seq.end()
        }
    }

    struct ThresholdPrivateKeyShareDeserializeVisitor<E: Curve> {
        _marker: std::marker::PhantomData<E>,
    }

    impl<'de, E: Curve> serde::de::Visitor<'de> for ThresholdPrivateKeyShareDeserializeVisitor<E> {
        // The type that our Visitor is going to produce.
        type Value = ThresholdPrivateKeyShare<E>;

        // Format a message stating what data this Visitor expects to receive.
        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a byte array")
        }

        fn visit_seq<S>(self, mut access: S) -> Result<Self::Value, S::Error>
        where
            S: serde::de::SeqAccess<'de>,
        {
            let mut bson_byte_array = Vec::with_capacity(access.size_hint().unwrap_or(0));

            while let Some(value) = access.next_element()? {
                bson_byte_array.push(value);
            }
            let key: CoreKeyShare<E> = bson::from_slice(&bson_byte_array)
                .map_err(|e| serde::de::Error::custom(format!("Bson deserialization: {e}")))?;

            Ok(ThresholdPrivateKeyShare(key))
        }
    }

    impl<'de, E: Curve> Deserialize<'de> for ThresholdPrivateKeyShare<E> {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::de::Deserializer<'de>,
        {
            deserializer
                .deserialize_seq(ThresholdPrivateKeyShareDeserializeVisitor { _marker: std::marker::PhantomData })
        }
    }
}

impl<E: Curve> ThresholdPrivateKey<E> {
    /// Creates an [`ThresholdPrivateKey`] from a given [`SecretScalar`] of type E.
    ///
    /// # Arguments
    /// * `private_scalar` - A scalar value representing the private key.
    ///
    /// # Returns
    /// An `Option` containing `ThresholdPrivateKey` if the scalar is non-zero, otherwise `None`.
    pub fn from_scalar(private_scalar: SecretScalar<E>) -> Option<Self> {
        NonZero::from_secret_scalar(private_scalar).map(Self)
    }

    /// Attempts to create an [`ThresholdPrivateKey`] from a 32-byte array in big-endian order.
    ///
    /// The input bytes should be 32 bytes in length, representing the private scalar in big-endian.
    /// The ECDSA signatures interpretes bytes in big-endian.
    ///
    /// # Arguments
    /// * `bytes` - A byte slice expected to be of length 32.
    ///
    /// # Returns
    /// A `Result` containing [`ThresholdPrivateKey`] if successful, or an error of type [`ThresholdPrivateKeyError`].
    ///
    /// # Errors
    /// * [`ThresholdPrivateKeyError::InvalidLengthError`] - If the input byte array length is not 32 bytes.
    /// * [`ThresholdPrivateKeyError::ZeroScalarError`] - If the generated scalar is zero.
    /// * [`ThresholdPrivateKeyError::InvalidScalar`] - If the byte array does not represent a valid scalar.
    ///
    /// # Example
    /// ```rust
    /// use threshold_keypair::privatekey::ThresholdPrivateKey;
    /// use generic_ec::curves::Secp256k1;
    /// // bytes is 32 bytes long
    /// let bytes = &[84, 104, 105, 115, 32, 109, 101, 115, 115, 97, 103, 101, 32, 105, 115, 32, 101, 120, 97, 99, 116, 108, 121, 32, 51, 50, 32, 98, 121, 116, 101, 0];
    /// match ThresholdPrivateKey::<Secp256k1>::from_be_bytes(bytes) {
    ///     Ok(private_key) => println!("Private key created: {:?}", private_key),
    ///     Err(e) => println!("Error: {:?}", e),
    /// }
    /// ```
    pub fn from_be_bytes(bytes: &[u8]) -> Result<Self, ThresholdPrivateKeyError> {
        // Check if the length of the input bytes is exactly 32

        if bytes.len() != PRIVATE_KEY_LENGTH {
            return Err(ThresholdPrivateKeyError::InvalidLengthError);
        }

        // Two possible errors here: one from the from_le_bytes and the other from being zero
        let mut scalar = Scalar::<E>::from_be_bytes_mod_order(bytes);
        let private_scalar = SecretScalar::new(&mut scalar);
        Self::from_scalar(private_scalar).ok_or(ThresholdPrivateKeyError::ZeroScalarError)
    }

    /// Attempts to create an [`ThresholdPrivateKey`] from a 32-byte array in little-endian order.
    ///
    /// The input bytes should be 32 bytes in length, representing the private scalar in little-endian.
    /// The EdDSA signatures interpretes bytes in little-endian.
    ///
    /// # Arguments
    /// * `bytes` - A byte slice expected to be of length 32.
    ///
    /// # Returns
    /// A `Result` containing [`ThresholdPrivateKey`] if successful, or an error of type [`ThresholdPrivateKeyError`].
    ///
    /// # Errors
    /// * [`ThresholdPrivateKeyError::InvalidLengthError`] - If the input byte array length is not 32 bytes.
    /// * [`ThresholdPrivateKeyError::ZeroScalarError`] - If the generated scalar is zero.
    /// * [`ThresholdPrivateKeyError::InvalidScalar`] - If the byte array does not represent a valid scalar.
    ///
    /// # Example
    /// ```rust
    /// use threshold_keypair::privatekey::ThresholdPrivateKey;
    /// use generic_ec::curves::Ed25519;
    /// // bytes is 32 bytes long
    /// let bytes = &[84, 104, 105, 115, 32, 109, 101, 115, 115, 97, 103, 101, 32, 105, 115, 32, 101, 120, 97, 99, 116, 108, 121, 32, 51, 50, 32, 98, 121, 116, 101, 0];
    /// match ThresholdPrivateKey::<Ed25519>::from_le_bytes(bytes) {
    ///     Ok(private_key) => println!("Private key created: {:?}", private_key),
    ///     Err(e) => println!("Error: {:?}", e),
    /// }

    /// ```
    pub fn from_le_bytes(bytes: &[u8]) -> Result<Self, ThresholdPrivateKeyError> {
        // Check if the length of the input bytes is exactly 32

        if bytes.len() != PRIVATE_KEY_LENGTH {
            return Err(ThresholdPrivateKeyError::InvalidLengthError);
        }

        // Two possible errors here: one from the from_le_bytes and the other from being zero
        let mut scalar = Scalar::<E>::from_le_bytes_mod_order(bytes);
        let private_scalar = SecretScalar::new(&mut scalar);
        Self::from_scalar(private_scalar).ok_or(ThresholdPrivateKeyError::ZeroScalarError)
    }

    /// Converts the [`ThresholdPrivateKey`] to its byte representation in big-endian order.
    /// The ECDSA signatures interpretes bytes in big-endian.
    ///
    /// # Returns
    /// A `Vec<u8>` containing the 32-byte big-endian representation of the private scalar.
    ///
    /// # Example
    /// ```rust
    /// use threshold_keypair::privatekey::ThresholdPrivateKey;
    /// use generic_ec::curves::Secp256k1;
    /// use generic_ec::SecretScalar;
    /// use rand::rngs::OsRng;
    ///
    /// let mut csprng = OsRng;
    /// let key = ThresholdPrivateKey::from_scalar(SecretScalar::<Secp256k1>::random(&mut csprng)).unwrap();
    /// let key_bytes = key.to_be_bytes();
    /// println!("Key bytes: {:?}", key_bytes);
    /// ```
    pub fn to_be_bytes(self) -> Vec<u8> {
        let scalar = self.0.into_inner();
        let bytes = scalar.as_ref().to_be_bytes();
        bytes.to_vec()
    }

    /// Converts the [`ThresholdPrivateKey`] to its byte representation in little-endian order.
    /// The EdDSA signatures interpretes bytes in little-endian order.
    ///
    /// # Returns
    /// A `Vec<u8>` containing the 32-byte little-endian representation of the private scalar.
    ///
    /// # Example
    /// ```rust
    /// use threshold_keypair::privatekey::ThresholdPrivateKey;
    /// use generic_ec::curves::Ed25519;
    /// use generic_ec::SecretScalar;
    /// use rand::rngs::OsRng;
    ///
    /// let mut csprng = OsRng;
    /// let key = ThresholdPrivateKey::from_scalar(SecretScalar::<Ed25519>::random(&mut csprng)).unwrap();
    /// let key_bytes = key.to_le_bytes();
    /// println!("Key bytes: {:?}", key_bytes);
    /// ```
    pub fn to_le_bytes(self) -> Vec<u8> {
        let scalar = self.0.into_inner();
        let bytes = scalar.as_ref().to_le_bytes();
        bytes.to_vec()
    }

    /// Borrows the inner [`NonZero<SecretScalar>`] from this [`ThresholdPrivateKey`].
    ///
    /// # Returns
    /// A reference to the non-zero private scalar.
    ///
    /// # Example
    /// ```rust
    /// use threshold_keypair::privatekey::ThresholdPrivateKey;
    /// use generic_ec::curves::{Secp256k1,Ed25519};
    /// use generic_ec::SecretScalar;
    /// use rand::rngs::OsRng;
    ///
    /// let mut csprng = OsRng;
    /// let key = ThresholdPrivateKey::from_scalar(SecretScalar::<Secp256k1>::random(&mut csprng)).unwrap();
    /// let non_zero_scalar = key.as_non_zero_scalar();
    /// println!("Non-zero scalar: {:?}", non_zero_scalar);
    /// let key = ThresholdPrivateKey::from_scalar(SecretScalar::<Ed25519>::random(&mut csprng)).unwrap();
    /// let non_zero_scalar = key.as_non_zero_scalar();
    /// println!("Non-zero scalar: {:?}", non_zero_scalar);
    /// ```
    pub fn as_non_zero_scalar(&self) -> &NonZero<SecretScalar<E>> {
        &self.0
    }

    /// Derives the public key associated with this Elliptic private key.
    ///
    /// # Returns
    /// An instance of [`ThresholdPublicKey`] derived from the private key.
    ///
    /// # Example
    /// ```rust
    /// use threshold_keypair::privatekey::ThresholdPrivateKey;
    /// use generic_ec::curves::{Secp256k1,Ed25519};
    /// use generic_ec::SecretScalar;
    /// use rand::rngs::OsRng;
    ///
    /// let mut csprng = OsRng;
    /// let key = ThresholdPrivateKey::from_scalar(SecretScalar::<Secp256k1>::random(&mut csprng)).unwrap();
    /// let public_key = key.public_key();
    /// println!("Derived public key: {:?}", public_key);
    /// let key = ThresholdPrivateKey::from_scalar(SecretScalar::<Ed25519>::random(&mut csprng)).unwrap();
    /// let public_key = key.public_key();
    /// println!("Derived public key: {:?}", public_key);
    /// ```
    pub fn public_key(&self) -> ThresholdPublicKey<E> {
        ThresholdPublicKey::from_private_key(self)
    }

    /// Generate additive key shares of the provided Elliptic private key.
    ///
    /// This method generates `n` shares of the provided Threshold private key to be used along
    /// with the n-out-of-n CGGMP21 signing protocol <https://eprint.iacr.org/2021/060>.
    /// The key shares are based on the `Secp256k1` elliptic curve or on the Edwards Curve `Ed25519`.
    /// Both have security level of 128 bits.
    /// The underlying method used is [`key_share::trusted_dealer::TrustedDealerBuilder::generate_shares`].
    ///
    /// # Arguments
    ///
    /// * `n` - The number of key shares to generate.
    ///
    /// # Returns
    /// A `Result` containing a vector of [`ThresholdPrivateKeyShare`] if successful, or an error of type [`ThresholdPrivateKeyError`].
    ///
    /// # Example
    /// ```rust
    /// use threshold_keypair::privatekey::ThresholdPrivateKey;
    /// use generic_ec::curves::{Secp256k1,Ed25519};
    /// use generic_ec::SecretScalar;
    /// use rand::rngs::OsRng;
    ///
    /// let mut csprng = OsRng;
    /// let key = ThresholdPrivateKey::from_scalar(SecretScalar::<Secp256k1>::random(&mut csprng)).unwrap();
    /// let private_key_share = key.generate_shares(5);
    /// assert!(private_key_share.is_ok());
    /// let shares = private_key_share.unwrap();
    /// assert_eq!(shares.len(), 5);
    /// let key = ThresholdPrivateKey::from_scalar(SecretScalar::<Ed25519>::random(&mut csprng)).unwrap();
    /// let private_key_share = key.generate_shares(5);
    /// assert!(private_key_share.is_ok());
    /// let shares = private_key_share.unwrap();
    /// assert_eq!(shares.len(), 5);
    /// ```
    pub fn generate_shares(&self, n: u16) -> Result<Vec<ThresholdPrivateKeyShare<E>>, ThresholdPrivateKeyError> {
        let mut csprng = OsRng;
        let secret_key_to_be_imported = self.as_non_zero_scalar().to_owned();
        let core_shares = trusted_dealer::builder::<E>(n)
            .set_threshold(None)
            .set_shared_secret_key(secret_key_to_be_imported)
            .generate_shares(&mut csprng)?;
        // transform Vec<CoreKeyShare> into Vec<ThresholdPrivateKeyShare>
        let core_shares = core_shares.into_iter().map(ThresholdPrivateKeyShare).collect();
        Ok(core_shares)
    }

    /// Recover an Threshold private key from its shares.
    ///
    /// This method takes a vector of [`ThresholdPrivateKeyShare`] and attempts to reconstruct the original Threshold private key
    /// from a n-out-of-ne additive share.
    /// The reconstruction process is performed using [`key_share::reconstruct_secret_key`], which combines the individual key
    /// shares into a single scalar value, corresponding to the original threshold private key.
    ///
    /// # Arguments
    ///
    /// * `key_shares` - A vector containing [`ThresholdPrivateKeyShare`] instances that represent the distributed shares of the original threshold private key.
    ///
    /// # Returns
    /// A `Result` containing the reconstructed [`ThresholdPrivateKey`], or an error of type [`ThresholdPrivateKeyError`].
    pub fn recover(
        key_shares: Vec<ThresholdPrivateKeyShare<E>>,
    ) -> Result<ThresholdPrivateKey<E>, ThresholdPrivateKeyError> {
        let key_shares: Vec<CoreKeyShare<E>> = key_shares.into_iter().map(|key_share| key_share.into_inner()).collect();
        let scalar_reconstructed = key_share::reconstruct_secret_key(&key_shares)?;
        ThresholdPrivateKey::from_scalar(scalar_reconstructed).ok_or(ThresholdPrivateKeyError::ZeroKeyReconstructError)
    }
}

impl<E: Curve> PartialEq for ThresholdPrivateKey<E> {
    fn eq(&self, other: &Self) -> bool {
        let left = self.as_non_zero_scalar().clone();
        let right = other.as_non_zero_scalar().clone();
        left.ct_eq(&right).into()
    }
}

impl<E: Curve> PartialEq for ThresholdPrivateKeyShare<E> {
    fn eq(&self, other: &Self) -> bool {
        let left = &self.clone().0.x;
        let right = &other.clone().0.x;
        left.ct_eq(right).into()
    }
}

impl<E: Curve> From<ThresholdPrivateKeyShare<E>> for ThresholdPrivateKey<E> {
    fn from(encoded: ThresholdPrivateKeyShare<E>) -> Self {
        let secret_scalar = encoded.0.into_inner().x;
        ThresholdPrivateKey(secret_scalar)
    }
}

impl<E: Curve> fmt::Debug for ThresholdPrivateKeyShare<E> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("ThresholdPrivateKeyShare(Valid<DirtyCoreKeyShare<...>>")
    }
}

impl<E: Curve> fmt::Display for ThresholdPrivateKey<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.as_non_zero_scalar(),)
    }
}

/// Enum representing errors that can occur when handling Threshold private keys.
#[derive(Error, Debug)]
pub enum ThresholdPrivateKeyError {
    /// Error when creating a scalar from bytes.
    #[error("Invalid scalar for private key")]
    InvalidScalarError(InvalidScalar),

    /// Error when attempting to create a non-zero private scalar from a zero scalar.
    #[error("Zero scalar used")]
    ZeroScalarError,

    /// Error when the byte array used to create the private key is of an invalid size.
    #[error("Bytearray with invalid size")]
    InvalidLengthError,

    /// Error when generating the shares of a private key.
    #[error("Creating shares of an threshold private key failed")]
    InvalidShareGeneration(TrustedDealerError),

    /// Error when reconstructing a private key from its shares.
    #[error("Reconstructing an threshold private key failed")]
    ReconstructError(ReconstructError),

    /// Private key reconstruction yields zero private key.
    #[error("Private key reconstruction yields zero private key.")]
    ZeroKeyReconstructError,
}

// Implement the From trait to convert InvalidScalar into ThresholdPrivateKeyError
impl From<InvalidScalar> for ThresholdPrivateKeyError {
    fn from(error: InvalidScalar) -> Self {
        ThresholdPrivateKeyError::InvalidScalarError(error)
    }
}

// Implement the From trait to convert TrustedDealerError into ThresholdPrivateKeyError
impl From<TrustedDealerError> for ThresholdPrivateKeyError {
    fn from(error: TrustedDealerError) -> Self {
        ThresholdPrivateKeyError::InvalidShareGeneration(error)
    }
}

// Implement the From trait to convert ReconstructError into ThresholdPrivateKeyError
impl From<ReconstructError> for ThresholdPrivateKeyError {
    fn from(error: ReconstructError) -> Self {
        ThresholdPrivateKeyError::ReconstructError(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use generic_ec::{Curve, Point, Scalar};
    use subtle::ConstantTimeEq;

    fn test_new_from_valid_input<E: Curve>() {
        let mut csprng = OsRng;
        let scalar_zero = Scalar::<E>::zero();

        // from private scalar (with high probability it is non-zero)
        let private_scalar = SecretScalar::<E>::random(&mut csprng);
        // check if it is non-zero
        let is_non_zero = !private_scalar.as_ref().ct_eq(&scalar_zero);

        // recusive call until we find a non-zero element
        if is_non_zero.into() {
            let key = ThresholdPrivateKey::from_scalar(private_scalar).unwrap();
            println!("A random private scalar is well formed: {:?}", key.0);
        } else {
            test_new_from_valid_input::<E>()
        }
    }

    fn test_new_from_invalid_input<E: Curve>() {
        let mut scalar_zero = Scalar::<E>::zero();

        // from a zero private scalar
        let key = ThresholdPrivateKey::from_scalar(SecretScalar::<E>::new(&mut scalar_zero));
        match key {
            Some(_) => panic!("Threshold private key should not have a zero scalar."),
            None => (),
        }
    }
    fn test_from_be_bytes<E: Curve>() {
        let input_bytes = &[
            84, 104, 105, 115, 32, 109, 101, 115, 115, 97, 103, 101, 32, 105, 115, 32, 101, 120, 97, 99, 116, 108, 121,
            32, 51, 50, 32, 98, 121, 116, 101, 0,
        ];
        let key_result = ThresholdPrivateKey::<E>::from_be_bytes(input_bytes);
        match key_result {
            Err(e) => panic!("Input byte array with size different from 32 bytes: {e}"),
            Ok(key) => println!("From bytes key is well formed: {:?}", key.0),
        }
    }

    fn test_from_le_bytes<E: Curve>() {
        let input_bytes = &[
            84, 104, 105, 115, 32, 109, 101, 115, 115, 97, 103, 101, 32, 105, 115, 32, 101, 120, 97, 99, 116, 108, 121,
            32, 51, 50, 32, 98, 121, 116, 101, 0,
        ];
        let key_result = ThresholdPrivateKey::<E>::from_le_bytes(input_bytes);
        match key_result {
            Err(e) => panic!("Input byte array with size different from 32 bytes: {e}"),
            Ok(key) => println!("From bytes key is well formed: {:?}", key.0),
        }
    }
    fn test_fail_from_be_bytes<E: Curve>() {
        // from bytes will fail
        let input_bytes = b"some very big message bigger than 32 bytes that will cause the from_bytes to fail";
        let key_result = ThresholdPrivateKey::<E>::from_be_bytes(input_bytes);
        match key_result {
            Err(_) => (),
            Ok(key) => panic!("Input byte array too big. Key is not supposed to be created: {:?}", key.0),
        }
    }
    fn test_fail_from_le_bytes<E: Curve>() {
        // from bytes will fail
        let input_bytes = b"some very big message bigger than 32 bytes that will cause the from_bytes to fail";
        let key_result = ThresholdPrivateKey::<E>::from_le_bytes(input_bytes);
        match key_result {
            Err(_) => (),
            Ok(key) => panic!("Input byte array too big. Key is not supposed to be created: {:?}", key.0),
        }
    }

    fn test_valid_as_be_bytes<E: Curve>() {
        let input_bytes = &[
            84, 104, 105, 115, 32, 109, 101, 115, 115, 97, 103, 101, 32, 105, 115, 32, 101, 120, 97, 99, 116, 108, 121,
            32, 51, 50, 32, 98, 121, 116, 101, 0,
        ];
        let key_result = ThresholdPrivateKey::<E>::from_be_bytes(input_bytes);
        match key_result {
            Err(e) => panic!("Input byte array too big with error: {e}"),
            Ok(key) => {
                let bytes = key.to_be_bytes();
                assert_eq!(input_bytes.to_vec(), bytes);
            }
        }
    }

    fn test_valid_as_le_bytes<E: Curve>() {
        let input_bytes = &[
            84, 104, 105, 115, 32, 109, 101, 115, 115, 97, 103, 101, 32, 105, 115, 32, 101, 120, 97, 99, 116, 108, 121,
            32, 51, 50, 32, 98, 121, 116, 101, 0,
        ];
        let key_result = ThresholdPrivateKey::<E>::from_le_bytes(input_bytes);
        match key_result {
            Err(e) => panic!("Input byte array too big with error: {e}"),
            Ok(key) => {
                let bytes = key.to_le_bytes();
                assert_eq!(input_bytes.to_vec(), bytes);
            }
        }
    }

    fn test_valid_get_public_key<E: Curve>() {
        let mut csprng = OsRng;
        let sk = SecretScalar::<E>::random(&mut csprng);

        let pk = Point::<E>::generator().to_point() * &sk;
        let e_sk = ThresholdPrivateKey::from_scalar(sk).unwrap();
        let e_pk = ThresholdPrivateKey::public_key(&e_sk);
        assert_eq!(ThresholdPublicKey::from_point(pk).unwrap(), e_pk);
    }

    fn test_generate_shares_and_reconstruct<E: Curve>() {
        let mut csprng = OsRng;
        let n = 3;
        let sk = SecretScalar::<E>::random(&mut csprng);
        let e_sk = ThresholdPrivateKey::from_scalar(sk).unwrap();

        let sk_shares = e_sk.generate_shares(n).unwrap();
        for share in sk_shares.clone() {
            let i = share.as_inner().i;
            println!("The index is: {i}");
        }
        let e_reconstructed_sk = ThresholdPrivateKey::recover(sk_shares).unwrap();

        assert_eq!(e_reconstructed_sk, e_sk);
    }

    #[test]
    fn test_new_from_valid_input_256k1() {
        test_new_from_valid_input::<generic_ec::curves::Secp256k1>()
    }
    #[test]
    fn test_new_from_valid_input_25519() {
        test_new_from_valid_input::<generic_ec::curves::Ed25519>()
    }
    #[test]
    fn test_new_from_invalid_input_256k1() {
        test_new_from_invalid_input::<generic_ec::curves::Secp256k1>()
    }
    #[test]
    fn test_new_from_invalid_input_25519() {
        test_new_from_invalid_input::<generic_ec::curves::Ed25519>()
    }
    #[test]
    fn test_from_bytes_256k1() {
        test_from_be_bytes::<generic_ec::curves::Secp256k1>()
    }
    #[test]
    fn test_from_bytes_25519() {
        test_from_le_bytes::<generic_ec::curves::Ed25519>()
    }
    #[test]
    fn test_fail_from_bytes_256k1() {
        test_fail_from_be_bytes::<generic_ec::curves::Secp256k1>()
    }
    #[test]
    fn test_fail_from_bytes_25519() {
        test_fail_from_le_bytes::<generic_ec::curves::Ed25519>()
    }
    #[test]
    fn test_valid_as_bytes_256k1() {
        test_valid_as_be_bytes::<generic_ec::curves::Secp256k1>()
    }
    #[test]
    fn test_valid_as_bytes_25519() {
        test_valid_as_le_bytes::<generic_ec::curves::Ed25519>()
    }
    #[test]
    fn test_valid_get_public_key_256k1() {
        test_valid_get_public_key::<generic_ec::curves::Secp256k1>()
    }
    #[test]
    fn test_valid_get_public_key_25519() {
        test_valid_get_public_key::<generic_ec::curves::Ed25519>()
    }
    #[test]
    fn test_generate_shares_and_reconstruct_256k1() {
        test_generate_shares_and_reconstruct::<generic_ec::curves::Secp256k1>()
    }
    #[test]
    fn test_generate_shares_and_reconstruct_25519() {
        test_generate_shares_and_reconstruct::<generic_ec::curves::Ed25519>()
    }
}

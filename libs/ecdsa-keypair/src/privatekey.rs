//! The ecdsa private key implementation.

use crate::{publickey::EcdsaPublicKey, PRIVATE_KEY_LENGTH};
use generic_ec::{curves::Secp256k1, errors::InvalidScalar, NonZero, SecretScalar};
use key_share::{
    self,
    trusted_dealer::{self, TrustedDealerError},
    CoreKeyShare, ReconstructError,
};
use rand::rngs::OsRng;
use std::{cmp::PartialEq, fmt};
use subtle::ConstantTimeEq;
use thiserror::Error;

/// A struct representing an ECDSA private key.
/// The private key is a non-zero scalar defined on the secp256k1 elliptic curve.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EcdsaPrivateKey(NonZero<SecretScalar<Secp256k1>>);

/// A struct representing an ECDSA private key share.
///
/// In the context of distributed key generation (DKG) or threshold signing,
/// the private key can be split into multiple shares, and each participant holds one share.
/// The shares of the private key are a non-zero scalar defined on the secp256k1 elliptic curve.
#[derive(Clone)]
pub struct EcdsaPrivateKeyShare(CoreKeyShare<Secp256k1>);

impl EcdsaPrivateKeyShare {
    /// Public constructor from [`CoreKeyShare`].
    pub fn new(key_share: CoreKeyShare<Secp256k1>) -> Self {
        Self(key_share)
    }

    /// Returns a reference to the inner `CoreKeyShare<Secp256k1>` value.
    pub fn as_inner(&self) -> &CoreKeyShare<Secp256k1> {
        &self.0
    }

    /// Consumes the `EcdsaPrivateKeyShare` and returns the inner `CoreKeyShare<Secp256k1>`.
    pub fn into_inner(self) -> CoreKeyShare<Secp256k1> {
        self.0
    }
}

impl From<CoreKeyShare<Secp256k1>> for EcdsaPrivateKeyShare {
    fn from(key_share: CoreKeyShare<Secp256k1>) -> Self {
        Self(key_share)
    }
}

#[cfg(feature = "serde")]
mod details {
    use super::EcdsaPrivateKeyShare;
    use generic_ec::curves::Secp256k1;
    use key_share::CoreKeyShare;
    use serde::{Deserialize, Serialize};
    use std::fmt;

    impl Serialize for EcdsaPrivateKeyShare {
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

    struct EcdsaPrivateKeyShareDeserializeVisitor;

    impl<'de> serde::de::Visitor<'de> for EcdsaPrivateKeyShareDeserializeVisitor {
        // The type that our Visitor is going to produce.
        type Value = EcdsaPrivateKeyShare;

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
            let key: CoreKeyShare<Secp256k1> = bson::from_slice(&bson_byte_array)
                .map_err(|e| serde::de::Error::custom(format!("Bson deserialization: {e}")))?;

            Ok(EcdsaPrivateKeyShare(key))
        }
    }

    impl<'de> Deserialize<'de> for EcdsaPrivateKeyShare {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::de::Deserializer<'de>,
        {
            deserializer.deserialize_seq(EcdsaPrivateKeyShareDeserializeVisitor)
        }
    }
}

impl EcdsaPrivateKey {
    /// Creates an [`EcdsaPrivateKey`] from a given [`SecretScalar`] of type secp256k1.
    ///
    /// # Arguments
    /// * `private_scalar` - A scalar value representing the private key.
    ///
    /// # Returns
    /// An `Option` containing `EcdsaPrivateKey` if the scalar is non-zero, otherwise `None`.
    pub fn from_scalar(private_scalar: SecretScalar<Secp256k1>) -> Option<Self> {
        NonZero::from_secret_scalar(private_scalar).map(Self)
    }

    /// Attempts to create an [`EcdsaPrivateKey`] from a 32-byte array in big-endian order.
    ///
    /// The input bytes should be 32 bytes in length, representing the private scalar in big-endian.
    ///
    /// # Arguments
    /// * `bytes` - A byte slice expected to be of length 32.
    ///
    /// # Returns
    /// A `Result` containing [`EcdsaPrivateKey`] if successful, or an error of type [`EcdsaPrivateKeyError`].
    ///
    /// # Errors
    /// * [`EcdsaPrivateKeyError::InvalidLengthError`] - If the input byte array length is not 32 bytes.
    /// * [`EcdsaPrivateKeyError::ZeroScalarError`] - If the generated scalar is zero.
    /// * [`EcdsaPrivateKeyError::InvalidScalar`] - If the byte array does not represent a valid scalar.
    ///
    /// # Example
    /// ```rust
    /// use ecdsa_keypair::privatekey::EcdsaPrivateKey;
    /// let bytes = b"This message is exactly 32 bytes";
    /// match EcdsaPrivateKey::from_bytes(bytes) {
    ///     Ok(private_key) => println!("Private key created: {:?}", private_key),
    ///     Err(e) => println!("Error: {:?}", e),
    /// }
    /// ```
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, EcdsaPrivateKeyError> {
        // Check if the length of the input bytes is exactly 32
        if bytes.len() != PRIVATE_KEY_LENGTH {
            return Err(EcdsaPrivateKeyError::InvalidLengthError);
        }

        // Two possible errors here: one from the from_be_bytes and the other from being zero
        let private_scalar = SecretScalar::<Secp256k1>::from_be_bytes(bytes)?;
        Self::from_scalar(private_scalar).ok_or(EcdsaPrivateKeyError::ZeroScalarError)
    }

    /// Converts the [`EcdsaPrivateKey`] to its byte representation in big-endian order.
    ///
    /// # Returns
    /// A `Vec<u8>` containing the 32-byte big-endian representation of the private scalar.
    ///
    /// # Example
    /// ```rust
    /// use ecdsa_keypair::privatekey::EcdsaPrivateKey;
    /// use generic_ec::curves::Secp256k1;
    /// use generic_ec::SecretScalar;
    /// use rand::rngs::OsRng;
    ///
    /// let mut csprng = OsRng;
    /// let key = EcdsaPrivateKey::from_scalar(SecretScalar::<Secp256k1>::random(&mut csprng)).unwrap();
    /// let key_bytes = key.to_bytes();
    /// println!("Key bytes: {:?}", key_bytes);
    /// ```
    pub fn to_bytes(self) -> Vec<u8> {
        let scalar = self.0.into_inner();
        let bytes = scalar.as_ref().to_be_bytes();
        bytes.to_vec()
    }

    /// Borrows the inner [`NonZero<SecretScalar>`] from this [`EcdsaPrivateKey`].
    ///
    /// # Returns
    /// A reference to the non-zero private scalar.
    ///
    /// # Example
    /// ```rust
    /// use ecdsa_keypair::privatekey::EcdsaPrivateKey;
    /// use generic_ec::curves::Secp256k1;
    /// use generic_ec::SecretScalar;
    /// use rand::rngs::OsRng;
    ///
    /// let mut csprng = OsRng;
    /// let key = EcdsaPrivateKey::from_scalar(SecretScalar::<Secp256k1>::random(&mut csprng)).unwrap();
    /// let non_zero_scalar = key.as_non_zero_scalar();
    /// println!("Non-zero scalar: {:?}", non_zero_scalar);
    /// ```
    pub fn as_non_zero_scalar(&self) -> &NonZero<SecretScalar<Secp256k1>> {
        &self.0
    }

    /// Derives the public key associated with this ECDSA private key.
    ///
    /// # Returns
    /// An instance of [`EcdsaPublicKey`] derived from the private key.
    ///
    /// # Example
    /// ```rust
    /// use ecdsa_keypair::privatekey::EcdsaPrivateKey;
    /// use generic_ec::curves::Secp256k1;
    /// use generic_ec::SecretScalar;
    /// use rand::rngs::OsRng;
    ///
    /// let mut csprng = OsRng;
    /// let key = EcdsaPrivateKey::from_scalar(SecretScalar::<Secp256k1>::random(&mut csprng)).unwrap();
    /// let public_key = key.public_key();
    /// println!("Derived public key: {:?}", public_key);
    /// ```
    pub fn public_key(&self) -> EcdsaPublicKey {
        EcdsaPublicKey::from_private_key(self)
    }

    /// Generate additive key shares of the provided ECDSA private key.
    ///
    /// This method generates `n` shares of the provided ECSDA private key to be used along
    /// with the n-out-of-n CGGMP21 signing protocol <https://eprint.iacr.org/2021/060>.
    /// The key shares are based on the `Secp256k1` elliptic curve, have security level of 128 bits and
    /// the underlying method used is [`key_share::trusted_dealer::TrustedDealerBuilder::generate_shares`].
    ///
    /// # Arguments
    ///
    /// * `n` - The number of key shares to generate.
    ///
    /// # Returns
    /// A `Result` containing a vector of [`EcdsaPrivateKeyShare`] if successful, or an error of type [`EcdsaPrivateKeyError`].
    ///
    /// # Example
    /// ```rust
    /// use ecdsa_keypair::privatekey::EcdsaPrivateKey;
    /// use generic_ec::curves::Secp256k1;
    /// use generic_ec::SecretScalar;
    /// use rand::rngs::OsRng;
    ///
    /// let mut csprng = OsRng;
    /// let key = EcdsaPrivateKey::from_scalar(SecretScalar::<Secp256k1>::random(&mut csprng)).unwrap();
    /// let private_key_share = key.generate_shares(5);
    /// assert!(private_key_share.is_ok());
    /// let shares = private_key_share.unwrap();
    /// assert_eq!(shares.len(), 5);
    /// ```
    pub fn generate_shares(&self, n: u16) -> Result<Vec<EcdsaPrivateKeyShare>, EcdsaPrivateKeyError> {
        let mut csprng = OsRng;
        let secret_key_to_be_imported = self.as_non_zero_scalar().to_owned();
        let core_shares = trusted_dealer::builder::<Secp256k1>(n)
            .set_threshold(None)
            .set_shared_secret_key(secret_key_to_be_imported)
            .generate_shares(&mut csprng)?;
        // transform Vec<CoreKeyShare> into Vec<EcdsaPrivateKeyShare>
        let core_shares = core_shares.into_iter().map(EcdsaPrivateKeyShare).collect();
        Ok(core_shares)
    }

    /// Recover an ECDSA private key from its shares.
    ///
    /// This method takes a vector of [`EcdsaPrivateKeyShare`] and attempts to reconstruct the original ECDSA private key
    /// from a n-out-of-ne additive share.
    /// The reconstruction process is performed using [`key_share::reconstruct_secret_key`], which combines the individual key
    /// shares into a single scalar value, corresponding to the original ecdsa private key.
    ///
    /// # Arguments
    ///
    /// * `key_shares` - A vector containing [`EcdsaPrivateKeyShare`] instances that represent the distributed shares of the original ecdsa private key.
    ///
    /// # Returns
    /// A `Result` containing the reconstructed [`EcdsaPrivateKey`], or an error of type [`EcdsaPrivateKeyError`].
    pub fn recover(key_shares: Vec<EcdsaPrivateKeyShare>) -> Result<EcdsaPrivateKey, EcdsaPrivateKeyError> {
        let key_shares: Vec<CoreKeyShare<Secp256k1>> =
            key_shares.into_iter().map(|key_share| key_share.into_inner()).collect();
        let scalar_reconstructed = key_share::reconstruct_secret_key(&key_shares)?;
        EcdsaPrivateKey::from_scalar(scalar_reconstructed).ok_or(EcdsaPrivateKeyError::ZeroKeyReconstructError)
    }
}

impl PartialEq for EcdsaPrivateKey {
    fn eq(&self, other: &Self) -> bool {
        let left = self.as_non_zero_scalar().clone();
        let right = other.as_non_zero_scalar().clone();
        left.ct_eq(&right).into()
    }
}

impl PartialEq for EcdsaPrivateKeyShare {
    fn eq(&self, other: &Self) -> bool {
        let left = &self.clone().0.x;
        let right = &other.clone().0.x;
        left.ct_eq(right).into()
    }
}

impl From<EcdsaPrivateKeyShare> for EcdsaPrivateKey {
    fn from(encoded: EcdsaPrivateKeyShare) -> Self {
        let secret_scalar = encoded.0.into_inner().x;
        EcdsaPrivateKey(secret_scalar)
    }
}

impl fmt::Debug for EcdsaPrivateKeyShare {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("EcdsaPrivateKeyShare(Valid<DirtyCoreKeyShare<...>>")
    }
}

impl fmt::Display for EcdsaPrivateKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.as_non_zero_scalar(),)
    }
}

/// Enum representing errors that can occur when handling ECDSA private keys.
#[derive(Error, Debug)]
pub enum EcdsaPrivateKeyError {
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
    #[error("Creating shares of an ecdsa private key failed")]
    InvalidShareGeneration(TrustedDealerError),

    /// Error when reconstructing a private key from its shares.
    #[error("Reconstructing an ecdsa private key failed")]
    ReconstructError(ReconstructError),

    /// Private key reconstruction yields zero private key.
    #[error("Private key reconstruction yields zero private key.")]
    ZeroKeyReconstructError,
}

// Implement the From trait to convert InvalidScalar into EcdsaPrivateKeyError
impl From<InvalidScalar> for EcdsaPrivateKeyError {
    fn from(error: InvalidScalar) -> Self {
        EcdsaPrivateKeyError::InvalidScalarError(error)
    }
}

// Implement the From trait to convert TrustedDealerError into EcdsaPrivateKeyError
impl From<TrustedDealerError> for EcdsaPrivateKeyError {
    fn from(error: TrustedDealerError) -> Self {
        EcdsaPrivateKeyError::InvalidShareGeneration(error)
    }
}

// Implement the From trait to convert ReconstructError into EcdsaPrivateKeyError
impl From<ReconstructError> for EcdsaPrivateKeyError {
    fn from(error: ReconstructError) -> Self {
        EcdsaPrivateKeyError::ReconstructError(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use generic_ec::{Point, Scalar};
    use subtle::ConstantTimeEq;

    #[test]
    fn test_new_from_valid_input() {
        let mut csprng = OsRng;
        let scalar_zero = Scalar::<Secp256k1>::zero();

        // from private scalar (with high probability it is non-zero)
        let private_scalar = SecretScalar::<Secp256k1>::random(&mut csprng);
        // check if it is non-zero
        let is_non_zero = !private_scalar.as_ref().ct_eq(&scalar_zero);

        // recusive call until we find a non-zero element
        if is_non_zero.into() {
            let key = EcdsaPrivateKey::from_scalar(private_scalar).unwrap();
            println!("A random private scalar is well formed: {:?}", key.0);
        } else {
            test_new_from_valid_input()
        }
    }

    #[test]
    fn test_new_from_invalid_input() {
        let mut scalar_zero = Scalar::<Secp256k1>::zero();

        // from a zero private scalar
        let key = EcdsaPrivateKey::from_scalar(SecretScalar::<Secp256k1>::new(&mut scalar_zero));
        match key {
            Some(_) => panic!("ECDSA private key should not have a zero scalar."),
            None => (),
        }
    }

    #[test]
    fn test_from_bytes() {
        let input_bytes = b"This message is exactly 32 bytes";
        let key_result = EcdsaPrivateKey::from_bytes(input_bytes);
        match key_result {
            Err(e) => panic!("Input byte array with size different from 32 bytes: {e}"),
            Ok(key) => println!("From bytes key is well formed: {:?}", key.0),
        }
    }

    #[test]
    fn test_fail_from_bytes() {
        // from bytes will fail
        let input_bytes = b"some very big message bigger than 32 bytes that will cause the from_bytes to fail";
        let key_result = EcdsaPrivateKey::from_bytes(input_bytes);
        match key_result {
            Err(_) => (),
            Ok(key) => panic!("Input byte array too big. Key is not supposed to be created: {:?}", key.0),
        }
    }

    #[test]
    fn test_valid_as_bytes() {
        let input_bytes = b"This message is exactly 32 bytes";
        let key_result = EcdsaPrivateKey::from_bytes(input_bytes);
        match key_result {
            Err(e) => panic!("Input byte array too big with error: {e}"),
            Ok(key) => {
                let bytes = key.to_bytes();
                assert_eq!(input_bytes.to_vec(), bytes);
            }
        }
    }

    #[test]
    fn test_valid_get_public_key() {
        let mut csprng = OsRng;
        let sk = SecretScalar::<Secp256k1>::random(&mut csprng);

        let pk = Point::<Secp256k1>::generator().to_point() * &sk;
        let ecdsa_sk = EcdsaPrivateKey::from_scalar(sk).unwrap();
        let ecdsa_pk = EcdsaPrivateKey::public_key(&ecdsa_sk);
        assert_eq!(EcdsaPublicKey::from_point(pk).unwrap(), ecdsa_pk);
    }

    #[test]
    fn test_generate_shares_and_reconstruct() {
        let mut csprng = OsRng;
        let n = 3;
        let sk = SecretScalar::<Secp256k1>::random(&mut csprng);
        let ecdsa_sk = EcdsaPrivateKey::from_scalar(sk).unwrap();

        let sk_shares = ecdsa_sk.generate_shares(n).unwrap();
        for share in sk_shares.clone() {
            let i = share.as_inner().i;
            println!("The index is: {i}");
        }
        let ecdsa_reconstructed_sk = EcdsaPrivateKey::recover(sk_shares).unwrap();

        assert_eq!(ecdsa_reconstructed_sk, ecdsa_sk);
    }
}

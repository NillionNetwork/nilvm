//! Utilities for ecdsa keypairs.

#![deny(missing_docs)]
#![forbid(unsafe_code)]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::iterator_step_by_zero,
    clippy::invalid_regex,
    clippy::string_slice,
    clippy::unimplemented,
    clippy::todo
)]

use thiserror::Error;

/// The length of en ecdsa private, in bytes.
pub const PRIVATE_KEY_LENGTH: usize = 32;
/// The length of an uncompressed ecdsa public key, in bytes.
pub const UNCOMPRESSED_PUBLIC_KEY_LENGTH: usize = 65;
/// The length of a compressed ecdsa public key, in bytes.
pub const COMPRESSED_PUBLIC_KEY_LENGTH: usize = 33;

pub mod privatekey;
pub mod publickey;
pub mod signature;
pub use generic_ec;

use crate::{
    privatekey::{EcdsaPrivateKey, EcdsaPrivateKeyError},
    publickey::{EcdsaPublicKey, EcdsaPublicKeyError},
};

/// A structure representing an ECDSA keypair, consisting of a private key and the corresponding public key.
#[derive(Debug, Clone)]
pub struct EcdsaKeyPair {
    /// The private key used for signing.
    pub(crate) private_key: EcdsaPrivateKey,
    /// The public key corresponding to the private key, used for verification.
    pub(crate) public_key: EcdsaPublicKey,
}

impl EcdsaKeyPair {
    /// Attempts to create an ECDSA keypair from a byte slice.
    ///
    /// The provided byte slice must contain the private scalar (32 bytes) followed by
    /// the a un/compressed public key (65/33 bytes).
    ///
    /// # Arguments
    ///
    /// * `bytes` - A byte slice containing the private key and public key.
    ///
    /// # Returns
    ///
    /// * A result with a new [`EcdsaKeyPair`] or an error.
    ///
    /// # Errors
    ///
    /// * [`EcdsaKeyPairError::EcdsaKeyMalformed`] if the byte array is malformed
    ///   and does not include a complete private key.
    ///
    /// * [`EcdsaKeyPairError::MismatchedKeypair`] if the public key does not
    ///   correspond to the given private key.
    ///
    /// * [`EcdsaKeyPairError::EcdsaPrivateKeyError`] if the private key does not
    ///   correspond to a valid private key.
    ///
    /// * [`EcdsaKeyPairError::EcdsaPublicKeyError`] if the public key does not
    ///   correspond to a valid public key.
    ///
    /// # Example
    /// ```rust
    /// use ecdsa_keypair::EcdsaKeyPair;
    /// let sk_uncompressed_pk = [
    ///     38, 76, 141, 58, 15, 177, 125, 153, 63, 53, 154, 221, 93, 39, 171, 153,
    ///     87, 62, 228, 193, 107, 217, 224, 255, 156, 254, 109, 134, 132, 96, 179, 88,
    ///     4, 48, 203, 130, 1, 24, 29, 230, 219, 157, 213, 60, 168, 158, 25, 22, 205,
    ///     129, 249, 203, 133, 2, 210, 253, 204, 234, 141, 88, 235, 34, 103, 81, 173,
    ///     243, 160, 44, 48, 198, 9, 110, 166, 114, 0, 177, 205, 138, 75, 42, 220, 52,
    ///     210, 233, 81, 61, 222, 124, 74, 213, 88, 224, 36, 73, 79, 75, 62
    /// ];
    /// let keypair = EcdsaKeyPair::try_from_bytes(&sk_uncompressed_pk).unwrap();
    /// ```
    pub fn try_from_bytes(bytes: &[u8]) -> Result<Self, EcdsaKeyPairError> {
        let (private_key, public_key) =
            bytes.split_at_checked(PRIVATE_KEY_LENGTH).ok_or(EcdsaKeyPairError::EcdsaKeyMalformed)?;
        let private_key = EcdsaPrivateKey::from_bytes(private_key)?;
        let public_key = EcdsaPublicKey::from_bytes(public_key)?;

        if EcdsaPublicKey::from_private_key(&private_key) != public_key {
            return Err(EcdsaKeyPairError::MismatchedKeypair);
        }

        Ok(Self { private_key, public_key })
    }

    /// Returns the public key component of the keypair.
    ///
    /// # Returns
    ///
    /// * A clone of the public key.
    pub fn public_key(&self) -> EcdsaPublicKey {
        self.public_key.clone()
    }

    /// Returns the private key component of the keypair.
    ///
    /// # Returns
    ///
    /// * A clone of the private key.
    pub fn private_key(&self) -> EcdsaPrivateKey {
        self.private_key.clone()
    }
}

/// Errors while handling an ECDSA public-private keypair.
#[derive(Error, Debug)]
pub enum EcdsaKeyPairError {
    /// Error creating private key from bytes
    #[error("error reading key from bytes")]
    EcdsaKeyMalformed,

    /// Error creating private key from bytes
    #[error("error creating key from seed: {0}")]
    EcdsaPrivateKeyError(#[from] EcdsaPrivateKeyError),

    /// Error creating public key from bytes
    #[error("error creating key from seed: {0}")]
    EcdsaPublicKeyError(#[from] EcdsaPublicKeyError),

    /// Error due to a mismatch between the public key and the private key.
    #[error("error due to mismatch between public key and private key")]
    MismatchedKeypair,
}

#[cfg(test)]
mod tests {
    use super::*;
    use generic_ec::{curves::Secp256k1, SecretScalar};
    use rand::rngs::OsRng;
    use rstest::rstest;

    #[rstest]
    #[case(true)]
    #[case(false)]
    fn test_try_valid(#[case] compressed: bool) {
        let mut csprng = OsRng;
        let sk = EcdsaPrivateKey::from_scalar(SecretScalar::<Secp256k1>::random(&mut csprng)).unwrap();
        let pk = EcdsaPublicKey::from_private_key(&sk);

        let mut sk_pk_bytes = sk.to_bytes();
        let pk_bytes = pk.to_bytes(compressed);

        sk_pk_bytes.extend(pk_bytes);
        let sk_pk_bytes = &sk_pk_bytes;

        assert!(EcdsaKeyPair::try_from_bytes(sk_pk_bytes).is_ok());
    }

    #[test]
    fn test_try_from_fail() {
        let sk_pk_bytes = b"ThisIsNotAValidSecretAndPublicKeys..ButHasTheSameCompressedLength";
        assert!(EcdsaKeyPair::try_from_bytes(sk_pk_bytes).is_err());
    }
}

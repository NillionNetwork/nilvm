//! secp256k1 keys.

use crate::{InvalidKey, InvalidSignature, Signature};
use ed25519_dalek::Verifier;
use k256::ecdsa::{signature::Signer, SigningKey, VerifyingKey};
use sha2::{Digest, Sha256};
use std::sync::Arc;

/// Public/Private key authentication attributes.
#[derive(Debug, Clone)]
pub struct Secp256k1SigningKey {
    /// The signing key.
    pub signing_key: Arc<SigningKey>,
}

impl Secp256k1SigningKey {
    /// Construct a private key from a byte encoded version of it.
    pub fn try_from_bytes(bytes: &[u8; 32]) -> Result<Self, InvalidKey> {
        let signing_key = SigningKey::from_bytes(bytes.into()).map_err(|_| InvalidKey)?;
        Ok(Self { signing_key: Arc::new(signing_key) })
    }

    /// Generate a new public/private key from a seed.
    pub fn try_from_seed(seed: &str) -> Result<Self, InvalidKey> {
        let hash = Sha256::digest(seed);
        Self::try_from_bytes(hash.as_ref())
    }

    /// Generate a new random public/private key.
    /// Uses a cryptographically secure pseudo-random number generator.
    pub fn generate() -> Secp256k1SigningKey {
        let keypair = SigningKey::random(&mut rand::thread_rng());
        Secp256k1SigningKey { signing_key: Arc::new(keypair) }
    }

    /// Sign a message.
    pub fn sign(&self, data: &[u8]) -> Signature {
        let signature: k256::ecdsa::Signature = self.signing_key.sign(data);
        Signature(signature.to_vec())
    }

    /// Gets the public key.
    pub fn public_key(&self) -> Secp256k1PublicKey {
        Secp256k1PublicKey(*self.signing_key.verifying_key())
    }

    /// Gets the signing key's secret bytes.
    ///
    /// This exposes the secret key, use with care.
    pub fn as_bytes(&self) -> [u8; 32] {
        self.signing_key.to_bytes().into()
    }
}

impl TryFrom<&[u8]> for Secp256k1SigningKey {
    type Error = InvalidKey;

    fn try_from(bytes: &[u8]) -> Result<Self, InvalidKey> {
        let bytes = bytes.try_into().map_err(|_| InvalidKey)?;
        Self::try_from_bytes(bytes)
    }
}

/// A public key
#[derive(Clone, Debug)]
pub struct Secp256k1PublicKey(VerifyingKey);

impl Secp256k1PublicKey {
    /// Try to construct a key from the given bytes.
    pub fn from_bytes(bytes: &[u8; 33]) -> Result<Self, InvalidKey> {
        let key = VerifyingKey::from_sec1_bytes(bytes).map_err(|_| InvalidKey)?;
        Ok(Self(key))
    }

    /// Verify a signature.
    pub fn verify(&self, signature: &Signature, data: &[u8]) -> Result<(), InvalidSignature> {
        let signature = k256::ecdsa::Signature::from_slice(&signature.0).map_err(|_| InvalidSignature)?;
        self.0.verify(data, &signature).map_err(|_| InvalidSignature)?;
        Ok(())
    }

    /// Get the raw bytes in the underlying key.
    pub fn as_bytes(&self) -> Vec<u8> {
        self.0.to_sec1_bytes().into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_signature() -> (&'static [u8], Secp256k1SigningKey, Signature) {
        let data = b"hi mom";
        let key = Secp256k1SigningKey::generate();
        let signature = key.sign(data);
        (data, key, signature)
    }

    #[test]
    fn generate_from_seed() {
        let seed = "hi mom";
        let key1 = Secp256k1SigningKey::try_from_seed(seed).unwrap().public_key().as_bytes().to_vec();
        let key2 = Secp256k1SigningKey::try_from_seed(seed).unwrap().public_key().as_bytes().to_vec();
        assert_eq!(key1, key2);
        assert_ne!(key1, Secp256k1SigningKey::try_from_seed("other").unwrap().public_key().as_bytes());
    }

    #[test]
    fn signature_verification_ok() {
        let (payload, key, signature) = make_signature();
        key.public_key().verify(&signature, payload).expect("verification failed");
    }

    #[test]
    fn verify_different_payload_fails() {
        let (_, key, signature) = make_signature();
        key.public_key().verify(&signature, b"potato").expect_err("verification didn't fail");
    }

    #[test]
    fn verify_different_signature_fails() {
        let (payload, key, mut signature) = make_signature();
        signature.0[0] = signature.0[0].wrapping_add(1);
        key.public_key().verify(&signature, payload).expect_err("verification didn't fail");
    }

    #[test]
    fn verify_different_key_fails() {
        let (payload, _, signature) = make_signature();
        let key = Secp256k1SigningKey::generate();
        key.public_key().verify(&signature, payload).expect_err("verification didn't fail");
    }

    #[test]
    fn public_key_to_from_bytes() {
        let signing = Secp256k1SigningKey::generate();
        let public = signing.public_key();
        let bytes = public.as_bytes().as_slice().try_into().expect("not the right byte length");
        Secp256k1PublicKey::from_bytes(&bytes).expect("from_bytes failed");
    }

    #[test]
    fn signature_compatilibity() {
        let key = Secp256k1SigningKey::try_from_seed("test").unwrap();
        let signature = key.sign(&[1]);
        assert_eq!(
            signature.0,
            &[
                22, 156, 180, 160, 31, 25, 179, 99, 9, 52, 138, 203, 25, 173, 189, 253, 2, 163, 23, 137, 45, 20, 202,
                173, 171, 82, 198, 145, 245, 209, 138, 14, 52, 77, 242, 21, 52, 57, 9, 196, 178, 206, 66, 105, 97, 116,
                240, 199, 105, 179, 90, 222, 145, 26, 16, 71, 246, 49, 247, 208, 22, 19, 8, 84
            ]
        );
    }
}

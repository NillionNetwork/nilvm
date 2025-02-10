//! ed25519 keys.

use crate::{InvalidKey, InvalidSignature, Signature};
use ed25519_dalek::{ed25519::signature::Signer, Digest, SigningKey, Verifier};
use sha2::Sha256;
use std::sync::Arc;

/// Public/Private key authentication attributes.
#[derive(Debug, Clone)]
pub struct Ed25519SigningKey {
    /// The signing key.
    pub signing_key: Arc<SigningKey>,
}

impl Ed25519SigningKey {
    /// Construct a private key from a byte encoded version of it.
    pub fn from_bytes(bytes: &[u8; 32]) -> Self {
        let signing_key = SigningKey::from_bytes(bytes);
        Self { signing_key: Arc::new(signing_key) }
    }

    /// Generate a new random public/private key.
    /// Uses a cryptographically secure pseudo-random number generator.
    pub fn generate() -> Ed25519SigningKey {
        let keypair = SigningKey::generate(&mut rand::thread_rng());
        Ed25519SigningKey { signing_key: Arc::new(keypair) }
    }

    /// Generate a new public/private key from a seed.
    pub fn from_seed(seed: &str) -> Self {
        let hash = Sha256::digest(seed);
        Self::from_bytes(hash.as_ref())
    }

    /// Sign a message.
    pub fn sign(&self, data: &[u8]) -> Signature {
        Signature(self.signing_key.sign(data).to_vec())
    }

    /// Gets the public key.
    pub fn public_key(&self) -> Ed25519PublicKey {
        Ed25519PublicKey(self.signing_key.verifying_key())
    }

    /// Gets the signing key's secret bytes.
    ///
    /// This exposes the secret key, use with care.
    pub fn as_bytes(&self) -> [u8; 32] {
        *self.signing_key.as_bytes()
    }
}

impl TryFrom<&[u8]> for Ed25519SigningKey {
    type Error = InvalidKey;

    fn try_from(bytes: &[u8]) -> Result<Self, InvalidKey> {
        let bytes = bytes.try_into().map_err(|_| InvalidKey)?;
        Ok(Self::from_bytes(bytes))
    }
}

/// A public key
#[derive(Clone, Debug)]
pub struct Ed25519PublicKey(ed25519_dalek::VerifyingKey);

impl Ed25519PublicKey {
    /// Try to construct a key from the given bytes.
    pub fn from_bytes(bytes: &[u8; 32]) -> Result<Self, InvalidKey> {
        let key = ed25519_dalek::VerifyingKey::from_bytes(bytes).map_err(|_| InvalidKey)?;
        Ok(Self(key))
    }

    /// Verify a signature.
    pub fn verify(&self, signature: &Signature, data: &[u8]) -> Result<(), InvalidSignature> {
        let signature = ed25519_dalek::Signature::from_slice(&signature.0).map_err(|_| InvalidSignature)?;
        self.0.verify(data, &signature).map_err(|_| InvalidSignature)?;
        Ok(())
    }

    /// Get the raw bytes in the underlying key.
    pub fn as_bytes(&self) -> &[u8; 32] {
        self.0.as_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_signature() -> (&'static [u8], Ed25519SigningKey, Signature) {
        let data = b"hi mom";
        let key = Ed25519SigningKey::generate();
        let signature = key.sign(data);
        (data, key, signature)
    }

    #[test]
    fn generate_from_seed() {
        let seed = "hi mom";
        let key1 = Ed25519SigningKey::from_seed(seed).public_key().as_bytes().to_vec();
        let key2 = Ed25519SigningKey::from_seed(seed).public_key().as_bytes().to_vec();
        assert_eq!(key1, key2);
        assert_ne!(key1, Ed25519SigningKey::from_seed("other").public_key().as_bytes());
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
        let key = Ed25519SigningKey::generate();
        key.public_key().verify(&signature, payload).expect_err("verification didn't fail");
    }

    #[test]
    fn signature_compatilibity() {
        let key = Ed25519SigningKey::from_seed("test");
        let signature = key.sign(&[1]);
        assert_eq!(
            signature.0,
            &[
                12, 101, 115, 14, 187, 31, 66, 50, 107, 78, 139, 17, 70, 106, 146, 136, 233, 33, 233, 200, 141, 121,
                185, 35, 165, 112, 59, 178, 41, 234, 216, 253, 215, 94, 101, 234, 151, 121, 25, 68, 96, 125, 94, 37,
                130, 79, 94, 57, 144, 123, 221, 17, 164, 238, 217, 99, 84, 246, 21, 244, 217, 203, 139, 11
            ]
        );
    }
}

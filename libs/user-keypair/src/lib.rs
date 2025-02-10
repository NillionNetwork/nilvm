//! Utilities for User authentication keypairs.

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

use ed25519::{Ed25519PublicKey, Ed25519SigningKey};
use secp256k1::{Secp256k1PublicKey, Secp256k1SigningKey};

pub mod ed25519;
pub mod secp256k1;

/// A signature.
#[derive(Clone, PartialEq)]
pub struct Signature(Vec<u8>);

impl From<Signature> for Vec<u8> {
    fn from(signature: Signature) -> Self {
        signature.0
    }
}

impl From<Vec<u8>> for Signature {
    fn from(signature: Vec<u8>) -> Self {
        Self(signature)
    }
}

/// A signature was invalid.
#[derive(thiserror::Error, Debug)]
#[error("invalid signature")]
pub struct InvalidSignature;

/// A secret/public key was invalid.
#[derive(thiserror::Error, Debug)]
#[error("invalid key")]
pub struct InvalidKey;

/// A signing key.
#[derive(Debug, Clone)]
pub enum SigningKey {
    /// An ed25519 signing key.
    Ed25519(Ed25519SigningKey),

    /// A secp256k1 signing key.
    Secp256k1(Secp256k1SigningKey),
}

impl SigningKey {
    /// Generate an ed25519 signing key.
    pub fn generate_ed25519() -> Self {
        Ed25519SigningKey::generate().into()
    }

    /// Generate a secp256k1 signing key.
    pub fn generate_secp256k1() -> Self {
        Secp256k1SigningKey::generate().into()
    }

    /// Sign a message.
    pub fn sign(&self, data: &[u8]) -> Signature {
        match self {
            Self::Ed25519(key) => key.sign(data),
            Self::Secp256k1(key) => key.sign(data),
        }
    }

    /// Gets the public key.
    pub fn public_key(&self) -> PublicKey {
        match self {
            Self::Ed25519(key) => key.public_key().into(),
            Self::Secp256k1(key) => key.public_key().into(),
        }
    }

    /// Gets the signing key's secret bytes.
    ///
    /// This exposes the secret key, use with care.
    pub fn as_bytes(&self) -> Vec<u8> {
        match self {
            Self::Ed25519(key) => key.as_bytes().to_vec(),
            Self::Secp256k1(key) => key.as_bytes().to_vec(),
        }
    }
}

impl From<Ed25519SigningKey> for SigningKey {
    fn from(key: Ed25519SigningKey) -> Self {
        Self::Ed25519(key)
    }
}

impl From<Secp256k1SigningKey> for SigningKey {
    fn from(key: Secp256k1SigningKey) -> Self {
        Self::Secp256k1(key)
    }
}

/// A public key
#[derive(Clone, Debug)]
pub enum PublicKey {
    /// An ed25519 public key.
    Ed25519(Ed25519PublicKey),

    /// A secp256k1 public key.
    Secp256k1(Secp256k1PublicKey),
}

impl PublicKey {
    /// Verify a signature.
    pub fn verify(&self, signature: &Signature, data: &[u8]) -> Result<(), InvalidSignature> {
        match self {
            Self::Ed25519(key) => key.verify(signature, data),
            Self::Secp256k1(key) => key.verify(signature, data),
        }
    }

    /// Get the raw bytes in the underlying key.
    pub fn as_bytes(&self) -> Vec<u8> {
        match self {
            Self::Ed25519(key) => key.as_bytes().to_vec(),
            Self::Secp256k1(key) => key.as_bytes(),
        }
    }
}

impl From<Ed25519PublicKey> for PublicKey {
    fn from(key: Ed25519PublicKey) -> Self {
        Self::Ed25519(key)
    }
}

impl From<Secp256k1PublicKey> for PublicKey {
    fn from(key: Secp256k1PublicKey) -> Self {
        Self::Secp256k1(key)
    }
}

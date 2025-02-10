use anyhow::{anyhow, Result};
use cosmrs::{
    bip32::{DerivationPath, XPrv},
    crypto::secp256k1::SigningKey,
};
use sha2::{Digest, Sha256};
use std::{fmt, str::FromStr, sync::Arc};

const DEFAULT_ADDRESS_PREFIX: &str = "nillion";
// Standard derivation path for Cosmos
const DEFAULT_KEY_DERIVATION_PATH: &str = "m/44'/118'/0'/0/0";

/// A nillion-chain private key.
#[derive(Clone)]
pub struct NillionChainPrivateKey {
    pub(crate) key: Arc<SigningKey>,
    raw_key: Vec<u8>,
    pub address: NillionChainAddress,
}

impl NillionChainPrivateKey {
    /// Construct a private key from a seed.
    pub fn from_seed(seed: &str) -> Result<Self> {
        let derivation_path =
            DerivationPath::from_str(DEFAULT_KEY_DERIVATION_PATH).map_err(|e| anyhow!("deriving path: {e}"))?;

        // Hash the seed using SHA-256 to ensure it's 256 bits
        let mut hasher = Sha256::new();
        hasher.update(seed.as_bytes());
        let seed_hash = hasher.finalize();

        // Create the master key from the seed hash
        let xprv = XPrv::new(seed_hash).map_err(|e| anyhow!("creating private key: {e}"))?;

        // Derive the private key using the hashed seed and derivation path
        let mut derived_xprv = xprv;
        for child_number in derivation_path.into_iter() {
            derived_xprv = derived_xprv.derive_child(child_number).map_err(|e| anyhow!("deriving child key: {e}"))?;
        }

        // Convert the derived private key to a SecretKey
        let signing_key_bytes = derived_xprv.private_key().to_bytes();
        Self::from_bytes(&signing_key_bytes)
    }

    /// Construct a key from raw bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let signing_key = SigningKey::from_slice(bytes).map_err(|e| anyhow!("error in signing key creation: {e}"))?;
        let public_key = signing_key.public_key();
        let address =
            public_key.account_id(DEFAULT_ADDRESS_PREFIX).map_err(|e| anyhow!("Error in signing key creation: {e}"))?;
        Ok(Self { key: signing_key.into(), address: NillionChainAddress(address.to_string()), raw_key: bytes.to_vec() })
    }

    pub fn from_hex(hex: &str) -> Result<Self> {
        let bytes = hex::decode(hex).map_err(|e| anyhow!("decoding hex: {e}"))?;
        Self::from_bytes(&bytes)
    }

    pub fn as_hex(&self) -> String {
        hex::encode(&self.raw_key)
    }
}

/// A nillion chain address.
#[derive(Clone, Debug)]
pub struct NillionChainAddress(pub String);

impl fmt::Display for NillionChainAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_private_key() {
        let private_key = NillionChainPrivateKey::from_seed("test").expect("could not create private key");
        assert_eq!(private_key.address.0, "nillion1mp04skwnrpt7v2y3n4hd7ejjm59p4zp3d39hde");
    }
}

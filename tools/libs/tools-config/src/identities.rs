//! Utilities for handling identities and identities configuration.
use crate::{path::config_directory, ToolConfig};
use serde::{Deserialize, Serialize};
use std::{fmt, path::PathBuf, str::FromStr};

/// The Identity
///
/// Represents the key required for the client to access the Nillion network
#[derive(Serialize, Deserialize, Debug)]
pub struct Identity {
    /// The user private key
    #[serde(serialize_with = "hex::serde::serialize", deserialize_with = "hex::serde::deserialize")]
    pub private_key: Vec<u8>,

    /// The key type used.
    #[serde(default)]
    pub kind: Kind,
}

/// A key type.
#[derive(Clone, Serialize, Deserialize, Debug, Default)]
pub enum Kind {
    /// An ed25519 key.
    #[default]
    Ed25519,

    /// A secp256k1 key.
    Secp256k1,
}

impl FromStr for Kind {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "ed25519" => Ok(Self::Ed25519),
            "secp256k1" => Ok(Self::Secp256k1),
            _ => Err("invalid curve"),
        }
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Kind::Ed25519 => write!(f, "ed25519"),
            Kind::Secp256k1 => write!(f, "secp256k1"),
        }
    }
}

impl ToolConfig for Identity {
    fn root_config_path() -> PathBuf {
        config_directory().map(|dir| dir.join("identities")).unwrap_or_else(|| PathBuf::from("./"))
    }
}

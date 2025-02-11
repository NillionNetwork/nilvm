//! Outputs for the ECDSA DGK protocol.
pub use ecdsa_keypair::privatekey::EcdsaPrivateKeyShare;
use std::fmt::Display;

/// The ECDSA DGK output.
#[derive(Clone)]
pub enum KeyGenOutput<T> {
    /// The protocol was successful.
    Success {
        /// The output.
        element: T,
    },

    /// This or a subprotocol aborted by chance.
    Abort {
        /// The reason why it aborted
        reason: String,
    },
}

impl<T> Display for KeyGenOutput<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Success { .. } => write!(f, "Success"),
            Self::Abort { .. } => write!(f, "Abort"),
        }
    }
}

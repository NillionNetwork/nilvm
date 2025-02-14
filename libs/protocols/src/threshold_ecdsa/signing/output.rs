//! Outputs for the ECDSA signing protocol.
use cggmp21::signing::SigningError;
use std::fmt::Display;
use threshold_keypair::signature::EcdsaSignatureShare;

/// The ECDSA signing output.
pub enum EcdsaSignatureShareOutput {
    /// The protocol was successful.
    Success {
        /// The output elements.
        element: EcdsaSignatureShare,
    },

    /// This or a subprotocol aborted by chance.
    Abort {
        /// The reason why it aborted
        reason: SigningError,
    },
}

impl Display for EcdsaSignatureShareOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Success { .. } => write!(f, "Success"),
            Self::Abort { .. } => write!(f, "Abort"),
        }
    }
}

//! Outputs for the EdDSA signing protocol.
use givre::signing::round2::SigningError;
use std::fmt::Display;
use threshold_keypair::signature::EddsaSignature;

/// The EdDSA signing output.
pub enum EddsaSignatureOutput {
    /// The protocol was successful.
    Success {
        /// The output elements.
        element: EddsaSignature,
    },

    /// This or a subprotocol aborted by chance.
    Abort {
        /// The reason why it aborted
        reason: SigningError,
    },
}

impl Display for EddsaSignatureOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Success { .. } => write!(f, "Success"),
            Self::Abort { .. } => write!(f, "Abort"),
        }
    }
}

//! Outputs for the PREP-MODULO protocol.
use cggmp21::{
    key_refresh::KeyRefreshError,
    key_share::{DirtyAuxInfo, Valid},
};
use serde::{Deserialize, Serialize};
use std::fmt::{self, Debug, Display};

/// The shares produced on a successful PREP-MODULO run.
#[derive(Clone, Serialize, Deserialize)]
pub struct EcdsaAuxInfo {
    /// The auxiliary informatin
    pub aux_info: Valid<DirtyAuxInfo>,
}

impl Debug for EcdsaAuxInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EcdsaAuxInfo").finish()
    }
}

/// The ECDSA aux info output.
pub enum EcdsaAuxInfoOutput<T> {
    /// The protocol was successful.
    Success {
        /// The output elements.
        element: T,
    },

    /// This or a subprotocol aborted by chance.
    Abort {
        /// The reason why it aborted
        reason: KeyRefreshError,
    },
}

impl<T> EcdsaAuxInfoOutput<T> {
    /// Try to convert this output into its inner element.
    pub fn try_into_element(self) -> Result<T, KeyRefreshError> {
        match self {
            EcdsaAuxInfoOutput::Success { element } => Ok(element),
            EcdsaAuxInfoOutput::Abort { reason } => Err(reason),
        }
    }
}

impl<T> Display for EcdsaAuxInfoOutput<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Success { .. } => write!(f, "Success"),
            Self::Abort { .. } => write!(f, "Abort"),
        }
    }
}

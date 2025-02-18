//! Nillion node API.

pub mod auth;
pub mod compute;
pub mod leader_queries;
pub mod membership;
pub mod payments;
pub mod permissions;
pub mod preprocessing;
pub mod programs;
pub(crate) mod proto;
pub mod values;

#[cfg(feature = "rust-types")]
pub mod conversions;

#[cfg(feature = "rust-types")]
pub use conversions::*;

#[cfg(feature = "rust-types")]
pub use prost::Message;

#[cfg(feature = "rust-types")]
pub use chrono::{DateTime, Utc};

#[cfg(feature = "rust-types")]
pub mod strum {
    pub use ::strum::IntoEnumIterator;
}

#[cfg(feature = "rust-types")]
pub use tonic::{Code, Result, Status};

#[cfg(feature = "rust-types")]
pub mod errors {
    pub use tonic_types::{ErrorDetails, PreconditionViolation, QuotaFailure, QuotaViolation, RetryInfo, StatusExt};

    /// An error parsing an identifier from hex.
    #[derive(Debug, thiserror::Error)]
    pub enum InvalidHexId {
        /// The hex encoding was malformed.
        #[error("invalid hex encoding")]
        HexEncoding,

        /// The length of the identifier was wrong.
        #[error("invalid length")]
        InvalidLength,
    }
}

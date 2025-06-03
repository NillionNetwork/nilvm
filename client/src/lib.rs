//! The Nillion client.

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

pub mod builder;
pub mod grpc;
pub mod operation;
pub mod payments;
pub(crate) mod retry;
pub mod vm;

pub use nilchain_client::transactions::TokenAmount;
pub use nillion_client_core::values::{Clear, NadaType, NadaValue};
pub use node_api::auth::rust::UserId;
pub use tonic::async_trait;
pub use user_keypair::{ed25519::*, secp256k1::*, SigningKey};
pub use uuid::Uuid;

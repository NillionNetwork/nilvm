//! Utilities for threshold keypairs.

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

/// The length of en threshold private, in bytes.
pub const PRIVATE_KEY_LENGTH: usize = 32;
/// The length of an uncompressed ecdsa public key, in bytes.
pub const UNCOMPRESSED_ECDSA_PUBLIC_KEY_LENGTH: usize = 65;
/// The length of a compressed ecdsa public key, in bytes.
pub const COMPRESSED_ECDSA_PUBLIC_KEY_LENGTH: usize = 33;
/// The length of a eddsa public key, in bytes.
pub const EDDSA_PUBLIC_KEY_LENGTH: usize = 32;

pub mod privatekey;
pub mod publickey;
pub mod signature;
pub use generic_ec::{self, Curve};

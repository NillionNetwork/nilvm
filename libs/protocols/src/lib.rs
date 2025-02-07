//! Cryptographic Protocols

#![deny(missing_docs)]
#![forbid(unsafe_code)]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::iterator_step_by_zero,
    clippy::invalid_regex,
    clippy::string_slice,
    clippy::unimplemented,
    clippy::todo
)]

pub mod bit_operations;
pub mod conditionals;
pub mod distributed_key_generation;
pub mod division;
pub mod multiplication;
pub mod random;
pub mod reveal;
pub mod threshold_ecdsa;

#[cfg(any(test, feature = "validation"))]
pub mod simulator;

//! Basic types.

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
#![feature(exhaustive_patterns)]

pub mod batches;
pub mod errors;
pub mod jar;
pub mod party;

pub use batches::Batches;
pub use party::{InvalidPartyId, PartyId, PartyMessage};

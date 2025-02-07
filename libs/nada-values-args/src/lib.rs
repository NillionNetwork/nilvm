//! Helpers to use entities in the `values` crate in `clap` arguments.

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

pub mod args;
pub mod file;
pub mod named;
pub(crate) mod parse;

pub use args::NadaValueArgs;

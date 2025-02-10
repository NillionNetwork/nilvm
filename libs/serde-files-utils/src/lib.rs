//! Input/Output library
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
#![allow(clippy::module_inception)]

#[cfg(feature = "binary")]
pub mod binary;
#[cfg(feature = "json")]
pub mod json;
#[cfg(feature = "text")]
pub mod string;
#[cfg(feature = "yaml")]
pub mod yaml;

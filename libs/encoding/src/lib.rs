//! This crate contains everything related to the encoding of messages.

#![deny(missing_docs)]
#![forbid(unsafe_code)]
#![deny(
clippy::unwrap_used,
clippy::expect_used,
clippy::panic,
clippy::indexing_slicing,
//clippy::arithmetic_side_effects, //TODO: this triggers false positives with ModularNumber
clippy::iterator_step_by_zero,
clippy::invalid_regex,
clippy::string_slice,
clippy::unimplemented,
clippy::todo
)]
#![allow(clippy::module_inception)]

pub mod codec;

//! Math library for Big Integers, Modular Big Integers, and Polynomial operations
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
#![feature(trivial_bounds)]
#![allow(trivial_bounds)]

pub mod _tutorial;
pub mod conversions;
pub mod decoders;
pub mod errors;
pub mod fields;
pub mod galois;
pub mod matrix;
pub mod modular;
pub mod polynomial;
pub mod ring;
pub mod serde;
#[cfg(any(test, feature = "bench"))]
pub mod test_macros;

//! Nada value crate it contains NadaValue<T> and NadaType, pull all the possible T for NadaValue.
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
#![feature(never_type)]
#![feature(exhaustive_patterns)]
extern crate core;

pub mod classify;
pub mod clear;
pub mod clear_modular;
pub mod encoders;
pub mod encrypted;
pub mod errors;
#[cfg(feature = "json")]
pub mod json;
#[cfg(feature = "protobuf-serde")]
pub mod protobuf;
pub mod validation;
pub(crate) mod value;

pub use nada_type::{
    NadaPrimitiveType, NadaType, NadaTypeKind, NadaTypeMetadata, NeverPrimitiveType, PrimitiveTypes, Shape, TypeError,
};
pub use num_bigint::{BigInt, BigUint};
pub use value::{NadaInt, NadaUint, NadaValue};

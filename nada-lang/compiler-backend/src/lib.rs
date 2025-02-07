//! Compiler backend library for Nada lang
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
#![cfg_attr(test, feature(box_patterns))]
#![feature(never_type)]

pub mod literal_value;
pub mod preprocess;
pub mod program_contract;
pub mod validators;

pub mod mir {
    //! Reexport of mir with the 'mir::' namespace
    pub use mir_model::{proto, *};
}

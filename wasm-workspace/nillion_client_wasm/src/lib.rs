//! Nillion Javascript Client to interact with Nillion Network

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

mod errors;
mod program;
mod values;

pub use program::ProgramMetadata;
pub use values::{NadaValue, NadaValues};

/// Better logging for javascript errors
pub fn set_panic_hook() {
    console_error_panic_hook::set_once();
}

//! Execution engine virtual machine logic.

#![forbid(unsafe_code)]
#![deny(
    missing_docs,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::iterator_step_by_zero,
    clippy::invalid_regex,
    clippy::string_slice,
    clippy::unimplemented
)]
#![allow(clippy::module_inception)]

extern crate core;

pub mod metrics;
#[cfg(any(test, feature = "simulator"))]
pub mod simulator;
pub mod vm;

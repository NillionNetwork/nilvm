//! PREP-DIV-INT-SECRET protocol.
//!
//! This protocol produces shares of elements that can then be used to run the DIV-INT-SECRET protocol.

use anyhow::anyhow;

pub mod output;
pub mod state;

#[cfg(any(test, feature = "bench"))]
pub mod protocol;

#[cfg(test)]
pub(crate) mod test;

#[cfg(any(test, feature = "validation"))]
pub mod validation;

use self::state::PrepDivisionIntegerSecretState;
pub use output::{
    EncodedPrepDivisionIntegerSecretShares, PrepDivisionIntegerSecretShares, PrepDivisionIntegerSecretStateOutput,
};

state_machine_macros::define_encoded_dyn_state_machine!(
    PrepDivisionIntegerSecretState,
    PrepDivisionIntegerSecretStateOutput<EncodedPrepDivisionIntegerSecretShares>
);

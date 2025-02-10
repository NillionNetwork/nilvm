//! PREP-COMPARE protocol.
//!
//! This protocol produces shares of elements that can then be used to run the COMPARE protocol.

use anyhow::anyhow;

pub mod output;
pub mod state;

#[cfg(any(test, feature = "bench"))]
pub mod protocol;

#[cfg(any(test, feature = "validation"))]
pub mod validation;

#[cfg(test)]
pub(crate) mod test;

use self::state::PrepCompareState;
pub use output::{EncodedPrepCompareShares, PrepCompareShares, PrepCompareStateOutput};

state_machine_macros::define_encoded_dyn_state_machine!(
    PrepCompareState,
    PrepCompareStateOutput<EncodedPrepCompareShares>
);

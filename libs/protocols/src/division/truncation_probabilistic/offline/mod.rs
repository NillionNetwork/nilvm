//! PREP-TRUNCPR protocol.
//!
//! This protocol produces shares of elements that can then be used to run the PREP-TRUNCPR protocol.

use anyhow::anyhow;

pub mod output;
pub mod state;

#[cfg(test)]
pub(crate) mod test;

#[cfg(any(test, feature = "validation"))]
pub mod validation;

use self::state::PrepTruncPrState;
pub use output::{EncodedPrepTruncPrShares, PrepTruncPrShares, PrepTruncPrStateOutput};

state_machine_macros::define_encoded_dyn_state_machine!(
    PrepTruncPrState,
    PrepTruncPrStateOutput<EncodedPrepTruncPrShares>
);

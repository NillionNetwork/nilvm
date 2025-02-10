//! PREP-PUBLIC-OUTPUT-EQUALITY protocol.
//!
//! This protocol produces shares of elements that can then be used
//! to run the PUBLIC-OUTPUT-EQUALITY protocol.

use anyhow::anyhow;

pub mod output;
pub mod state;

#[cfg(test)]
pub(crate) mod test;

#[cfg(any(test, feature = "validation"))]
pub mod validation;

use self::state::PrepPublicOutputEqualityState;
pub use output::{
    EncodedPrepPublicOutputEqualityShares, PrepPublicOutputEqualityShares, PrepPublicOutputEqualityStateOutput,
};

state_machine_macros::define_encoded_dyn_state_machine!(
    PrepPublicOutputEqualityState,
    PrepPublicOutputEqualityStateOutput<EncodedPrepPublicOutputEqualityShares>
);

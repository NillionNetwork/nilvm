//! PREP-MOD2M protocol.
//!
//! This protocol produces shares of elements that can then be used to run the MOD2M protocol.

use anyhow::anyhow;

pub mod output;
pub mod state;

#[cfg(test)]
pub(crate) mod test;

#[cfg(any(test, feature = "validation", feature = "testing"))]
pub mod validation;

use self::state::PrepModulo2mState;
pub use output::{EncodedPrepModulo2mShares, PrepModulo2mShares, PrepModulo2mStateOutput};

state_machine_macros::define_encoded_dyn_state_machine!(
    PrepModulo2mState,
    PrepModulo2mStateOutput<EncodedPrepModulo2mShares>
);

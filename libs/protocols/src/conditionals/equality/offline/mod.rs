//! PREP PRIVATE OUTPUT EQUALITY protocol.
//!
//! This protocol produces the preprocessing elements required to run the PRIVATE OUTPUT EQUALITY protocol.
//! The protocol is used to privately evaluate whether two shares are equal and produce a shared output.

use anyhow::anyhow;
pub mod output;

pub mod state;

#[cfg(test)]
pub mod protocol;

#[cfg(any(test, feature = "validation"))]
pub mod validation;

#[cfg(test)]
pub(crate) mod test;

pub use state::*;
use state_machine::StateMachine;

/// The PREP PRIVATE OUTPUT EQUALITY protocol state machine.
pub type PrepPrivateOutputEqualityStateMachine<T> = StateMachine<PrepPrivateOutputEqualityState<T>>;

pub use output::{EncodedPrepPrivateOutputEqualityShares, PrepPrivateOutputEqualityStateOutput};

state_machine_macros::define_encoded_dyn_state_machine!(
    PrepPrivateOutputEqualityState,
    PrepPrivateOutputEqualityStateOutput<EncodedPrepPrivateOutputEqualityShares>
);

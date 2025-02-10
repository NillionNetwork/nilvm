//! PRIVATE OUTPUT EQUALITY protocol.
//!
//! The protocol is used to privately evaluate whether two shares are equal and produce a shared output.

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

/// The PRIVATE OUTPUT EQUALITY protocol state machine.
pub type PrivateOutputEqualityStateMachine<T> = StateMachine<PrivateOutputEqualityState<T>>;

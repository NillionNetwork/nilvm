//! POLY EVAL protocol.
//!
//! The protocol is used to privately evaluate a polynomial in the online phase with limited communication rounds.

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

/// The POLY EVAL protocol state machine.
pub type PolyEvalStateMachine<T> = StateMachine<PolyEvalState<T>>;

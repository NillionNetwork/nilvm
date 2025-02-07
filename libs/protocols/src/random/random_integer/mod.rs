//! Random protocol.

use state_machine::StateMachine;

pub mod state;
pub use state::*;

#[cfg(any(test, feature = "validation"))]
pub mod validation;

/// The Random Integer protocol state machine.
pub type RandomIntegerStateMachine<T> = StateMachine<RandomIntegerState<T>>;

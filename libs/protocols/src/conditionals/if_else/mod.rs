//! If Else protocol.

use state_machine::StateMachine;

pub mod state;
pub use state::*;

/// The IF-ELSE protocol state machine.
pub type IfElseStateMachine<T> = StateMachine<IfElseState<T>>;

#[cfg(test)]
mod test;

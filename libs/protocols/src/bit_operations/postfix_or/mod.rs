//! The POSTFIX-OR protocol.

pub mod state;

pub use state::*;

#[cfg(test)]
mod test;

use state_machine::StateMachine;

/// The POSTFIX-OR state machine.
pub type PostfixOrStateMachine<T> = StateMachine<PostfixOrState<T>>;

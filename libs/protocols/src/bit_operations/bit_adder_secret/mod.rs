//! The SECRET-BIT-ADDER protocol.

pub mod state;

pub use state::*;

#[cfg(test)]
mod test;

use state_machine::StateMachine;

/// The SECRET-BIT-ADDER state machine.
pub type SecretBitAdderStateMachine<T> = StateMachine<SecretBitAdderState<T>>;

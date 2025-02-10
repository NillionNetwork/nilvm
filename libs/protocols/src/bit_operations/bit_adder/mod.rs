//! The BIT-ADDER protocol.

pub mod state;

pub use state::*;

#[cfg(test)]
mod test;

use state_machine::StateMachine;

/// The BIT-ADDER state machine.
pub type BitAdderStateMachine<T> = StateMachine<BitAdderState<T>>;

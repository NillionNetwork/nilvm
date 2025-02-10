//! The MIXED-BIT-ADDER protocol.

pub mod state;

pub use state::*;

#[cfg(test)]
mod test;

use state_machine::StateMachine;

/// The MIXED-BIT-ADDER state machine.
pub type MixedBitAdderStateMachine<T> = StateMachine<MixedBitAdderState<T>>;

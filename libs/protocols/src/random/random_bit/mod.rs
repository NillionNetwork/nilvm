//! The RAN-BIT protocol.

pub mod state;

pub use state::*;

pub mod output;

#[cfg(test)]
mod test;
pub use output::*;

use state_machine::StateMachine;

#[cfg(any(test, feature = "validation"))]
pub mod validation;

/// The Random Bit (Random Boolean) state machine.
pub type RanBitStateMachine<T> = StateMachine<RandomBitState<T>>;

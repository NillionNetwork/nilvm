//! The SCALE protocol.

pub mod state;

pub use state::*;

#[cfg(test)]
mod test;

use state_machine::StateMachine;

/// The SCALE state machine.
pub type ScaleStateMachine<T> = StateMachine<ScaleState<T>>;

//! The BIT-DECOMPOSE protocol.

pub mod state;

pub use state::*;

#[cfg(test)]
mod test;

use state_machine::StateMachine;

/// The BIT-DECOMPOSE state machine.
pub type BitDecomposeStateMachine<T> = StateMachine<BitDecomposeState<T>>;

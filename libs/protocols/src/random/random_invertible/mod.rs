//! Random Invertible protocol.

use state_machine::StateMachine;

pub mod state;
pub use state::*;

#[cfg(test)]
mod test;

/// The INV-RAN protocol state machine.
pub type InvRanStateMachine<T> = StateMachine<InvRanState<T>>;

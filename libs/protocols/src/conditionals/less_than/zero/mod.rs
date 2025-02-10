//! The LESS-THAN-ZERO protocol.

pub mod state;

pub use state::*;

#[cfg(any(test, feature = "bench"))]
pub mod protocol;

#[cfg(test)]
mod test;

use state_machine::StateMachine;

/// The LESS-THAN-ZERO protocol state machine.
pub type LessThanZeroStateMachine<T> = StateMachine<LessThanZeroState<T>>;

//! The QUATERNARY-LESS-THAN protocol.

pub mod state;

pub use state::*;

#[cfg(any(test, feature = "bench"))]
pub mod protocol;

#[cfg(test)]
mod test;

use state_machine::StateMachine;

/// The QUATERNARY-LESS-THAN protocol state machine.
pub type QuatLessStateMachine<T> = StateMachine<QuatLessState<T>>;

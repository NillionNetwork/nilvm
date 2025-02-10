//! The BIT-LESS-THAN protocol.

pub mod state;

pub use state::*;

#[cfg(test)]
mod test;

use state_machine::StateMachine;

/// The BIT-LESS-THAN protocol state machine.
pub type BitLessThanStateMachine<T> = StateMachine<BitLessThanState<T>>;

//! The DIVISION protocol.

use state_machine::StateMachine;

/// The Integer division by public divisor protocol state machine.
pub type DivisonIntegerPublicDivisorStateMachine<T> = StateMachine<DivisionIntegerPublicDivisorState<T>>;

pub mod state;
pub use state::*;

#[cfg(test)]
pub mod protocol;

#[cfg(test)]
mod test;

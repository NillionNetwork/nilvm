//! DIVISION protocol.

use state_machine::StateMachine;

pub mod online;
pub use online::state::*;

pub mod offline;

/// The Integer division by secret divisor protocol state machine.
pub type DivisonIntegerSecretDivisorStateMachine<T> = StateMachine<DivisionIntegerSecretDivisorState<T>>;

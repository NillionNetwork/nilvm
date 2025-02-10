//! COMPARE protocol.

use offline::state::PrepCompareState;
use state_machine::StateMachine;

pub mod online;
pub use online::state::*;

pub mod offline;
pub mod quaternary;
pub mod zero;

/// The COMPARE protocol state machine.
pub type CompareStateMachine<T> = StateMachine<CompareState<T>>;

/// The PREP-COMPARE protocol state machine.
pub type PrepCompareStateMachine<T> = StateMachine<PrepCompareState<T>>;

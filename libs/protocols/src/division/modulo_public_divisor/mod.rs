//! COMPARE protocol.

use offline::state::PrepModuloState;
use state_machine::StateMachine;

pub mod offline;
pub mod online;
pub use online::state::*;

/// The MODULO protocol state machine.
pub type ModuloStateMachine<T> = StateMachine<ModuloState<T>>;

/// The PREP-MODULO protocol state machine.
pub type PrepModuloStateMachine<T> = StateMachine<PrepModuloState<T>>;

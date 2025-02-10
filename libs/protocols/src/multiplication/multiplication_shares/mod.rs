//! Share multiplication protocol.

pub mod state;
pub use state::*;

use state_machine::StateMachine;

/// The MULT protocol state machine.
pub type MultStateMachine<T> = StateMachine<MultState<T>>;

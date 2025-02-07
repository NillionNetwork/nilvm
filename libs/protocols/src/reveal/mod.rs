//! Reveal protocol.

use state_machine::StateMachine;

pub mod state;
pub use state::*;

/// The REVEAL protocol state machine.
pub type RevealStateMachine<F, S> = StateMachine<RevealState<F, S>>;

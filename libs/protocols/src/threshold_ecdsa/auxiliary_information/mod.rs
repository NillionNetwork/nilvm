//! The ECDSA auxiliary information protocol.

pub mod fake;
pub mod output;
pub mod state;

pub use state::*;

#[cfg(test)]
mod test;

use state_machine::StateMachine;

/// The Ecdsa Aux Info state machine.
pub type EcdsaAuxInfoStateMachine = StateMachine<EcdsaAuxInfoState>;

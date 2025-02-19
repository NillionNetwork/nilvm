//! Threshold EdDSA protocol
pub mod output;
pub mod state;

pub use state::*;
#[cfg(test)]
pub mod test;

use state_machine::StateMachine;

/// The Eddsa Signing state machine.
pub type EddsaSignStateMachine = StateMachine<EddsaSignState>;

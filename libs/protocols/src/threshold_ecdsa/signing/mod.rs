//! The ECDSA signing protocol.

pub mod output;
pub mod state;

pub use state::*;

#[cfg(test)]
mod test;

use state_machine::StateMachine;

/// The Ecdsa Signing state machine.
pub type EcdsaSignStateMachine = StateMachine<EcdsaSignState>;

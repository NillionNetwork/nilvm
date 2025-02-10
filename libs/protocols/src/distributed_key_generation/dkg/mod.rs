//! The ECDSA DKG protocol.

pub mod output;
pub mod state;

pub use state::*;

#[cfg(test)]
mod test;

use state_machine::StateMachine;

/// The Ecdsa DKG state machine.
pub type EcdsaKeyGenStateMachine = StateMachine<EcdsaKeyGenState>;

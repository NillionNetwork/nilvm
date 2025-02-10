//! The ECDSA DKG protocol.

pub mod output;
pub mod state;

pub use state::*;

pub use cggmp21::generic_ec::Curve;

#[cfg(test)]
mod test;

use state_machine::StateMachine;

/// The DKG state machine.
pub type KeyGenStateMachine<C> = StateMachine<KeyGenState<C>>;

//! The Modulo with secret divisor protocol.

use state_machine::StateMachine;

/// The Modulo division by secret divisor protocol state machine.
pub type ModuloIntegerSecretDivisorStateMachine<T> = StateMachine<ModuloIntegerSecretDivisorState<T>>;

pub mod state;
pub use state::*;

#[cfg(test)]
pub mod protocol;

#[cfg(test)]
mod test;

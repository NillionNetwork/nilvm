//! The RANDOM-BITWISE protocol.
//!
//! This protocol generates N random modular numbers where each of them is either 0 or 1.

pub mod state;
pub use state::*;

pub mod bitwise_shares;
#[cfg(test)]
mod test;
pub use bitwise_shares::*;

use state_machine::StateMachine;

/// The RAN-BITWISE state machine.
pub type RanBitwiseStateMachine<T> = StateMachine<RanBitwiseState<T>>;

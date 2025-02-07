//! The RAN-QUATERNARY protocol.

pub mod state;

pub use state::*;

#[cfg(any(test, feature = "bench"))]
pub mod protocol;

pub mod quaternary_shares;
#[cfg(test)]
mod test;
pub use quaternary_shares::*;

use state_machine::StateMachine;

/// The RAN-QUATERNARY protocol state machine.
pub type RanQuaternaryStateMachine<T> = StateMachine<RanQuaternaryState<T>>;

//! Share multiplication protocol.

pub mod state;

pub use state::MultTruncShares;
use state::MultTruncState;
use state_machine::StateMachine;

/// The MULTIPLICATION-AND-TRUNCATION protocol state machine.
pub type MultTruncStateMachine<T> = StateMachine<MultTruncState<T>>;

#[cfg(any(test, feature = "bench", feature = "validation"))]
pub mod protocol;

#[cfg(test)]
mod test;

//! Share multiplication and reveal protocol.

pub mod state;

pub use state::PubOperandShares;

use state::PubMultState;
use state_machine::StateMachine;

/// The PUB-MULT protocol state machine.
pub type PubMultStateMachine<T> = StateMachine<PubMultState<T>>;

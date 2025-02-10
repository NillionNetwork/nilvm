//! MOD2M protocol.

use offline::state::PrepTruncPrState;
use state_machine::StateMachine;

pub mod offline;
pub mod online;
pub use online::state::*;

/// The TRUNCPR protocol state machine.
pub type TruncPrStateMachine<T> = StateMachine<TruncPrState<T>>;

/// The PREP-TRUNCPR protocol state machine.
pub type PrepTruncPrStateMachine<T> = StateMachine<PrepTruncPrState<T>>;

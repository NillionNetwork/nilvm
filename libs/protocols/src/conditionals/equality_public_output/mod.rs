//! PublicOutputEquality public output protocol.

use state_machine::StateMachine;

pub mod online;
pub use online::state::*;

pub mod offline;
use offline::state::PrepPublicOutputEqualityState;

/// The PUBLIC-OUTPUT-EQUALITY protocol state machine.
pub type PublicOutputEqualityStateMachine<T> = StateMachine<PublicOutputEqualityState<T>>;

/// The PREP-PUBLIC-OUTPUT-EQUALITY protocol state machine.
pub type PrepPublicOutputEqualityStateMachine<T> = StateMachine<PrepPublicOutputEqualityState<T>>;

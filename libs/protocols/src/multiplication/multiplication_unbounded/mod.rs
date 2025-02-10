//! Unbounded multiplication protocol families state machines.

use prefix::PrepPrefixMultState;
use state_machine::StateMachine;

pub mod offline;
pub mod online;
pub use online::state::*;
pub mod prefix;

/// The PREP-PREFIX-MULT protocol state machine.
pub type PrepPrefixMultStateMachine<T> = StateMachine<PrepPrefixMultState<T>>;

/// The UNBOUNDED-MULT protocol state machine.
pub type UnboundedMultStateMachine<T> = StateMachine<UnboundedMultState<T>>;

//! MOD2M protocol.

use offline::state::PrepModulo2mState;
use state_machine::StateMachine;

pub mod offline;
pub mod online;
pub use online::state::*;

/// The MODULO2M protocol state machine.
pub type Modulo2mStateMachine<T> = StateMachine<Modulo2mState<T>>;

/// The PREP-MODULO2M protocol state machine.
pub type PrepModulo2mStateMachine<T> = StateMachine<PrepModulo2mState<T>>;

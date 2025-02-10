//! State machine abstractions.
//!
//! The main type you want to look at if you're **defining** a state machine is the
//! [StateMachineState][crate::StateMachineState] trait.
//!
//! If instead you are trying to **use** a state machine, then you probably are looking for
//! [StateMachine][crate::StateMachine].

#![deny(missing_docs)]
#![forbid(unsafe_code)]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::iterator_step_by_zero,
    clippy::invalid_regex,
    clippy::string_slice,
    clippy::unimplemented,
    clippy::todo
)]

pub mod errors;
pub mod sm;
pub mod state;
#[cfg(test)]
mod test;

pub use sm::{StateMachine, StateMachineOutput};
pub use state::{StateMachineState, StateMachineStateExt, StateMachineStateOutput, StateMachineStateResult};

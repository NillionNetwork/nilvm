//! Implementation of errors that occurs during the execution of a state machine.

use anyhow::anyhow;
use thiserror::Error;

/// Errors that occur during the execution of a state machine.
#[derive(Error, Debug)]
pub enum StateMachineError {
    /// This error occurs when try to get next and the current state is a final state.
    #[error("final state")]
    Finished,

    /// This error occurs when try to get next state and the current state is not completed.
    #[error("state is not completed")]
    StateIsNotCompleted,

    /// This error occurs when any unexpected error is caught
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),

    /// The state machine channel dropped
    #[error("Channel dropped")]
    ChannelDropped(String),
}

impl From<InvalidStateError> for StateMachineError {
    fn from(_: InvalidStateError) -> Self {
        StateMachineError::UnexpectedError(anyhow!("invalid state has been reached"))
    }
}

impl From<StateUnavailableError> for StateMachineError {
    fn from(error: StateUnavailableError) -> Self {
        StateMachineError::UnexpectedError(anyhow!("{}", error))
    }
}

/// Error used when an invalid state transition occurs.
#[derive(Debug, Error)]
#[error("invalid state")]
pub struct InvalidStateError;

/// A state machine's state is unavailable.
///
/// This can be triggered by different reasons such as:
/// * An internal bug used the state but didn't put it back in its place.
/// * The state machine reached a terminal state and therefore the state is gone.
#[derive(Debug, Error)]
#[error("state unavailable: {0}")]
pub struct StateUnavailableError(pub &'static str);

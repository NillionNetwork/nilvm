//! State machine definitions.

use crate::{
    errors::{InvalidStateError, StateMachineError, StateUnavailableError},
    state::{RecipientMessage, StateMachineMessage, StateMachineState, StateMachineStateOutput},
};
use std::fmt::Formatter;

// A thin wrapper of the state. This lets us have visibility into why the state was taken to
// provide better error messages.
enum StateMachineInner<S> {
    Uninitialized,
    Taken,
    State(S),
    Finalized,
}

impl<S> StateMachineInner<S> {
    fn state(&self) -> Result<&S, StateUnavailableError> {
        if let Self::State(state) = self { Ok(state) } else { Err(self.as_error()) }
    }

    fn state_mut(&mut self) -> Result<&mut S, StateUnavailableError> {
        if let Self::State(state) = self { Ok(state) } else { Err(self.as_error()) }
    }

    fn into_state(self) -> Result<S, StateUnavailableError> {
        if let Self::State(state) = self { Ok(state) } else { Err(self.as_error()) }
    }

    fn take_state(&mut self) -> Result<S, StateUnavailableError> {
        let state = std::mem::replace(self, StateMachineInner::Taken);
        if let Self::State(state) = state { Ok(state) } else { Err(state.as_error()) }
    }

    fn as_error(&self) -> StateUnavailableError {
        let detail = match self {
            Self::Uninitialized => "state is uninitialized",
            Self::Taken => "state is taken",
            Self::Finalized => "state machine reached terminal state",
            // This shouldn't happen but we don't want to make this fallible for this dummy error.
            Self::State(_) => "internal error",
        };
        StateUnavailableError(detail)
    }
}

/// Implementation of a state machine.
///
/// This is a simple wrapper over a [StateMachineState] that allows using it without having to deal with all of the
/// functions in that trait that take `self` by value.
pub struct StateMachine<S: StateMachineState> {
    inner: StateMachineInner<S>,
    out_of_order_messages: Vec<S::InputMessage>,
}

impl<S: StateMachineState> StateMachine<S> {
    /// Create a new state machine.
    pub fn new(initial_state: S) -> Self {
        StateMachine { inner: StateMachineInner::State(initial_state), out_of_order_messages: Vec::new() }
    }

    /// Create a new state machine having an empty initial state.
    pub fn new_empty() -> Self {
        StateMachine { inner: StateMachineInner::Uninitialized, out_of_order_messages: Vec::new() }
    }

    /// Try to get an immutable reference to the current state.
    ///
    /// This will return an error if the state machine was previously consumed during a state transition. This can
    /// happen if either the state machine reached a terminal state or an unrecoverable error occurred during the
    /// state transition.
    pub fn state(&self) -> Result<&S, StateUnavailableError> {
        self.inner.state()
    }

    /// Try to get a mutable reference to the current state. See [state][StateMachine::state].
    pub fn state_mut(&mut self) -> Result<&mut S, StateUnavailableError> {
        self.inner.state_mut()
    }

    /// Consumes the state machine and returns the underlying state.
    pub fn into_state(self) -> Result<S, StateUnavailableError> {
        self.inner.into_state()
    }

    /// Checks whether the current state in this state machine is completed.
    pub fn is_state_completed(&self) -> bool {
        match self.inner.state() {
            Ok(state) => state.is_completed(),
            // If the state is consumed, we're completed. Any attempts to use the underlying state will
            // return an error anyway. AKA an empty state is always automatically completed.
            Err(_) => true,
        }
    }

    /// Checks whether the state machine is finished.
    pub fn is_finished(&self) -> bool {
        matches!(&self.inner, StateMachineInner::Finalized)
    }

    /// Let the underlying state handle the provided message, returning whatever output it produced.
    ///
    /// This returns a [StateMachineOutput], which is very similar to a [StateMachineStateOutput], except it doesn't
    /// have the [StateMachineState] as part of it.
    pub fn handle_message(&mut self, message: S::InputMessage) -> Result<HandleOutput<S>, StateMachineError> {
        // This is behind a feature flag as it's otherwise very CPU intensive.
        #[cfg(feature = "log-transitions")]
        let current_state_str = self.to_string();

        let state = self.inner.take_state()?;
        let output = state.handle_message(message)?;

        let output = self.apply_state_output(output);

        #[cfg(feature = "log-transitions")]
        {
            let new_state_str = self.to_string();
            if current_state_str != new_state_str {
                tracing::debug!("State transition: {current_state_str} -> {new_state_str}");
            }
        }

        // If there was a state transition, try to apply out of order messages. This is assuming state changes always
        // emit `Messages`, which is currently the case.
        if let StateMachineOutput::Messages(output_messages) = output {
            self.apply_out_of_order_messages(output_messages)
        } else {
            Ok(output)
        }
    }

    fn apply_state_output(&mut self, output: StateMachineStateOutput<S>) -> HandleOutput<S> {
        match output {
            StateMachineStateOutput::Empty(state) => {
                self.inner = StateMachineInner::State(state);
                StateMachineOutput::Empty
            }
            StateMachineStateOutput::Messages(state, messages) => {
                self.inner = StateMachineInner::State(state);
                StateMachineOutput::Messages(messages)
            }
            StateMachineStateOutput::OutOfOrder(state, message) => {
                self.inner = StateMachineInner::State(state);
                // Save it for later.
                self.out_of_order_messages.push(message);
                StateMachineOutput::Empty
            }
            StateMachineStateOutput::Final(output) => {
                self.inner = StateMachineInner::Finalized;
                StateMachineOutput::Final(output)
            }
        }
    }

    // Applies any out of order messages and accumulates new messages into the provided `Vec`.
    //
    // Collecting messages is necessary in case we manage to perform more than one state transition after receiving
    // a single message. This could theoretically happen if we received all messages for state N before we received
    // the last one for state N - 1. In this case, after receiving that last one, we would jump to state N and then
    // immediately to state N + 1 given we already have all of the ones for state N.
    fn apply_out_of_order_messages(
        &mut self,
        mut output_messages: Vec<StateMachineMessage<S>>,
    ) -> Result<HandleOutput<S>, StateMachineError> {
        let pending_messages = std::mem::take(&mut self.out_of_order_messages).into_iter();
        for message in pending_messages {
            match self.handle_message(message)? {
                StateMachineOutput::Messages(messages) => output_messages.extend(messages),
                // Note: if at this point `output_messages.len() > 0` then that would mean our messages are meaningless
                // to both us and the rest of the parties since we managed to get to the final state without them,
                // and so did they.
                StateMachineOutput::Final(output) => {
                    self.inner = StateMachineInner::Finalized;
                    return Ok(StateMachineOutput::Final(output));
                }
                StateMachineOutput::Empty => (),
            };
        }
        Ok(StateMachineOutput::Messages(output_messages))
    }
}

impl<S: StateMachineState> std::fmt::Display for StateMachine<S> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "StateMachine(")?;
        match &self.inner {
            StateMachineInner::Uninitialized => write!(f, "Uninitialized")?,
            StateMachineInner::Taken => write!(f, "Taken")?,
            StateMachineInner::State(state) => write!(f, "{}", state)?,
            StateMachineInner::Finalized => write!(f, "Finalized")?,
        }
        write!(f, ")")
    }
}

/// The output of a state machine. See the documentation on [StateMachineStateOutput] as these are basically
/// the same enum variant except it doesn't contain the state machine state itself.
#[derive(Debug)]
pub enum StateMachineOutput<R, O, F> {
    /// A state machine's output messages, typically something that needs to be communicated
    /// to other participants' state machines.
    Messages(Vec<RecipientMessage<R, O>>),

    /// The final output of a state machine.
    Final(F),

    /// No output was produced.
    Empty,
}

impl<R, O, F> StateMachineOutput<R, O, F> {
    /// Convert into a final output, error otherwise.
    pub fn into_final(self) -> Result<F, InvalidStateError> {
        match self {
            Self::Final(output) => Ok(output),
            _ => Err(InvalidStateError),
        }
    }

    /// Convert into output messages, error otherwise.
    pub fn into_messages(self) -> Result<Vec<RecipientMessage<R, O>>, InvalidStateError> {
        match self {
            Self::Messages(messages) => Ok(messages),
            _ => Err(InvalidStateError),
        }
    }

    /// Convert into an empty output, error otherwise.
    pub fn into_empty(self) -> Result<(), InvalidStateError> {
        match self {
            Self::Empty => Ok(()),
            _ => Err(InvalidStateError),
        }
    }
}

/// An alias for `StateMachineOutput` based on a `StateMachineState`.
#[allow(type_alias_bounds)]
pub type HandleOutput<S: StateMachineState> = StateMachineOutput<S::RecipientId, S::OutputMessage, S::FinalResult>;

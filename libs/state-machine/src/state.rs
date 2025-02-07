//! A state machine's state.

use crate::{
    errors::{InvalidStateError, StateMachineError},
    sm::StateMachineOutput,
};
use serde::{de::DeserializeOwned, Serialize};

/// Implementation a the state machine's state.
///
/// This trait should be implemented for an enum that expects to be used as a state within a
/// [StateMachine][crate::StateMachine] and allows defining various things like:
///
/// * Checking whether the current state is completed via [is_completed][StateMachineState::is_completed].
/// * Defining state transitions for this state via [try_next][StateMachineState::try_next].
/// * Defining the types used in this state machine to represent messages and how to handle them
///   via [handle_message][StateMachineState::handle_message].
///
/// It is recommended to use the `state-machine-derive` crate to make it easier to define this trait as it
/// can involve a good amount of boilerplate and the macro exposed in that crate automatically generates it for you.
pub trait StateMachineState
where
    Self: Sized + std::fmt::Display,
{
    /// The type that this state machine uses to address recipients in its output messages.
    ///
    /// This can be anything that the client is expected to use to represent the recipient of a message, but it will
    /// typically either be a string, a uuid, or some other type that's used to represent nodes in the network.
    /// Ultimately there will be some router component somewhere that will know how to address a node based on the
    /// contents of this type.
    type RecipientId;

    /// The input message for this state machine.
    ///
    /// This provides an abstraction over what clients should use to communicate with this state machine. A message
    /// will typically be an enum where each variant will talk to a specific state in this state machine.
    ///
    /// For example, for a state machine that expects 2 numbers in one state, and a bool in another we may define:
    ///
    /// ```ignore
    /// #[derive(Serialize, Deserialize, Clone)]
    /// pub enum MyStateMachineMessage {
    ///     // Will be handled in the state that is waiting for these numbers.
    ///     SetNumbers{numbers: Vec<u32>},
    ///
    ///     // Will be handled in the state that is waiting for the bool.
    ///     SetBool{the_bool: bool},
    /// }
    /// ```
    ///
    /// See documentation on [handle_message][StateMachineState::handle_message] for more information.
    type InputMessage: Serialize + DeserializeOwned + Clone + Send;

    /// The output message this state machine produces.
    ///
    /// Every input message that a state machine handles may generate 0+ output messages. These messages are typically
    /// used to communicate with other nodes' state machines during a state transition.
    ///
    /// For example, a state machine that runs Shamir will likely want to send its shares of the secret to every
    /// other node that's running the same state machine. In this case, the output message would likely be the same
    /// as the input one:
    ///
    /// ```ignore
    /// #[derive(Serialize, Deserialize, Clone)]
    /// pub enum ShamirMessage {
    ///     SetPartyShare{party_id: PartyId, share: Share},
    /// }
    /// ```
    ///
    /// The Shamir state machine would then emit N messages, one for every node in the network/shard, containing its
    /// own party id and the share that belongs to each particular node the message is being addressed to.
    type OutputMessage: Serialize + DeserializeOwned + Clone + Send;

    /// The type that represents the final output in this state machine.
    ///
    /// This is a single type that represents whatever can come out of this state machine once it's completed.
    /// This can include:
    ///
    /// * The output if it's successful. For example, the reconstructed secret in a _REVEAL_ operation.
    /// * The output if the protocol was aborted. For example, _RAN_ failing because some node lied.
    ///
    /// Because it's possible for this type to represent multiple outputs, this will typically be an enum
    /// covering all of them.
    type FinalResult: Send;

    /// Check if the current state of the state machine is completed.
    ///
    /// In this context, a state is completed if it has received all of the information it needs for it to
    /// transition into the next state. For example, a state that needs 10 numbers should only return true once
    /// 10 numbers have been set in its internal state.
    ///
    /// This will called during a call to [StateMachine::try_next][crate::StateMachine::try_next] and
    /// [StateMachineStateExt::advance_if_completed] to ensure the current state is completed before advancing
    /// the state machine.
    fn is_completed(&self) -> bool;

    /// Try to advance the state machine.
    ///
    /// This takes the current state machine by value, which allows taking any members in the current state and
    /// forwarding them to the next one, if any, or to the [StateMachineState::FinalResult] if the state transition
    /// causes the state machine state to finish.
    fn try_next(self) -> StateMachineStateResult<Self>;

    /// Handle a message and return an output.
    ///
    /// This is where most of the logic will be, and deals with handling the messages that this state machine
    /// understands to enrich the current state, possibly advancing the state machine, and optionally returning
    /// some output.
    ///
    /// See [StateMachineStateOutput] for more information on what this function's output represents.
    fn handle_message(self, message: Self::InputMessage) -> StateMachineStateResult<Self>;
}

/// Represents the types of outputs a state machine's message handling can produce.
///
/// Because [StateMachineState::handle_message] takes the state machine by value, this method will always return
/// the state machine (unless the output is [Final][StateMachineStateOutput::Final]) along with optionally more
/// information.
///
/// Since [StateMachineState] will always be used inside a [StateMachine][crate::StateMachine] (e.g.
/// `StateMachine<MyStateMachineState>`), `StateMachine` will deal with processing this output and
/// "splitting" the returned state machine state from the rest of the information, keeping the state locally and
/// returning the information to the caller.
pub enum StateMachineStateOutput<S: StateMachineState> {
    /// The action updated the underlying states and it either didn't cause a state transition or the state that was
    /// transitioned to didn't have any output.
    Empty(S),

    /// Some underlying state transitioned and produced some output messages which should be forwarded to the
    /// message recipients.
    Messages(S, Vec<StateMachineMessage<S>>),

    /// The message received was meant for a different state than the one the state machine is. This variant
    /// returns both the state machine and the out of order input message that was originally sent.
    OutOfOrder(S, S::InputMessage),

    /// The state machine finished and yielded this output. This is the only state that doesn't return a state
    /// machine state because that state was consumed during the state transition and no longer exists.
    Final(S::FinalResult),
}

impl<S: StateMachineState> StateMachineStateOutput<S> {
    /// Consume this output and keep only the state, returning an error if there's no state.
    pub fn into_state(self) -> Result<S, InvalidStateError> {
        use StateMachineStateOutput::*;
        match self {
            Empty(state) | Messages(state, _) => Ok(state),
            Final(_) | OutOfOrder(..) => Err(InvalidStateError),
        }
    }

    /// Consume this output and keep only the final output, returning an error if this is not a `Final`.
    pub fn into_final(self) -> Result<S::FinalResult, InvalidStateError> {
        use StateMachineStateOutput::*;
        match self {
            Final(output) => Ok(output),
            Empty(_) | Messages(..) | OutOfOrder(..) => Err(InvalidStateError),
        }
    }

    /// Consume this output and keep the inner state and messages, returning an error if this is not a `Messages`.
    pub fn into_messages(self) -> Result<(S, Vec<StateMachineMessage<S>>), InvalidStateError> {
        use StateMachineStateOutput::*;
        match self {
            Messages(state, messages) => Ok((state, messages)),
            Empty(_) | Final(_) | OutOfOrder(..) => Err(InvalidStateError),
        }
    }
}

impl<S: StateMachineState> From<S> for StateMachineStateOutput<S> {
    fn from(state: S) -> Self {
        Self::Empty(state)
    }
}

/// An alias for what `handle_message` returns to simplify user code.
pub type StateMachineStateResult<S> = Result<StateMachineStateOutput<S>, StateMachineError>;

/// A recipient for a message.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Recipient<T> {
    /// A single recipient.
    Single(T),

    /// Multiple recipients.
    Multiple(Vec<T>),
}

/// A message for a state machine. This is a simple wrapper over both:
///
/// * An output message that was produced by a state machine during the handling of an input message.
/// * A recipient that the message is addressed to. A router component will know how to map a recipient to
///   a node in the network.
#[derive(Clone, Debug)]
pub struct RecipientMessage<I, O> {
    recipient: Recipient<I>,
    contents: O,
}

impl<I, O> RecipientMessage<I, O> {
    /// Construct a new state machine message.
    pub fn new(recipient: Recipient<I>, contents: O) -> Self {
        Self { recipient, contents }
    }

    /// The recipient of this message.
    pub fn recipient(&self) -> &Recipient<I> {
        &self.recipient
    }

    /// The contents of this message, AKA the message itself.
    pub fn contents(&self) -> &O {
        &self.contents
    }

    /// Consumes this message and returns the contents of it.
    pub fn into_contents(self) -> O {
        self.contents
    }

    /// Consumes this message and returns the recipient and contents.
    pub fn into_parts(self) -> (Recipient<I>, O) {
        (self.recipient, self.contents)
    }

    /// Wraps the contents of this message. This is a convenient function that just wraps the contents of the
    /// message with something else.
    ///
    /// This is heavily used by state machines which are compositions of other state machines. In these ones, a message
    /// produced by an underlying state machine will need to be forwarded to another node. However, because the
    /// underlying state machine returns a different type of message than the one built on top of it, we need to wrap
    /// them in the state machine's message type to be able to forward them to the caller. For example:
    ///
    /// ```ignore
    /// // The underlying state machine's messages.
    /// enum FooStateMessage {
    ///     SetSomething{something: u32},
    ///     // ...
    /// }
    ///
    /// // The state machine that's built on top of `Foo`.
    /// enum BarStateMessage {
    ///     // A wrapper for a message for `Foo`.
    ///     Foo(FooStateMessage),
    ///
    ///     // ...
    /// }
    ///
    /// impl StateMachineState for Bar {
    /// // ...
    ///
    /// fn handle_message(self, message: BarStateMessage) -> StateMachineStateResult<Bar> {
    ///     match message {
    ///         // Forward this message to our Foo state machine
    ///         Foo(message) => match get_foo_state_machine()?.handle_message(message)? {
    ///             // `messages` here is a `Vec<FooStateMessage>`, we need to wrap them to return them.
    ///             StateMachineOutput::Messages(messages) => {
    ///                 // Wrap them in our own `BarStateMessage`.
    ///                 let messages = messages.into_iter().map(|message| message.wrap(&BarStateMessage::Foo)).collect();
    ///
    ///                 // Return them along.
    ///                 Ok(StateMachineStateOutput::Messages(self, messages))
    ///             }
    ///             // ...
    ///         }
    ///     }
    /// }
    /// }
    /// ```
    ///
    /// This also makes it trivial for a message to find its way in a very nested state machine: each state machine
    /// understands how to route its messages down, and each output message is wrapped on its own message enum, making
    /// the input and output the same.
    pub fn wrap<F, O2>(self, wrapper: &F) -> RecipientMessage<I, O2>
    where
        F: Fn(O) -> O2,
    {
        let contents = wrapper(self.contents);
        RecipientMessage::new(self.recipient, contents)
    }
}

/// An alias that allows deriving the recipient and output message out of a state machine state.
#[allow(type_alias_bounds)]
pub type StateMachineMessage<S: StateMachineState> = RecipientMessage<S::RecipientId, S::OutputMessage>;

/// An extension trait that adds some helper functions on top of a state machine state. This is automatically
/// defined for any type that implements [StateMachineState].
pub trait StateMachineStateExt: StateMachineState {
    /// Transitions the state if it is completed. This is basically a wrapper over checking whether
    /// [StateMachineState::is_completed] is true and calling [StateMachineState::try_next] if that's the case,
    /// or returning `self` otherwise.
    fn advance_if_completed(self) -> StateMachineStateResult<Self>;

    /// Wraps the messages, if any, of the provided `StateMachineOutput`. If the output is final or an intermediate
    /// output, it returns an error.
    ///
    /// See [StateMachineMessage::wrap] as this is a thin wrapper over that.
    fn wrap_message<O, F, W>(
        self,
        output: StateMachineOutput<Self::RecipientId, O, F>,
        wrapper: W,
    ) -> StateMachineStateResult<Self>
    where
        W: Fn(O) -> Self::OutputMessage;
}

impl<T: StateMachineState> StateMachineStateExt for T {
    fn advance_if_completed(self) -> StateMachineStateResult<Self> {
        if self.is_completed() { self.try_next() } else { Ok(StateMachineStateOutput::Empty(self)) }
    }

    fn wrap_message<O, F, W>(
        self,
        output: StateMachineOutput<Self::RecipientId, O, F>,
        wrapper: W,
    ) -> StateMachineStateResult<Self>
    where
        W: Fn(O) -> Self::OutputMessage,
    {
        match output {
            StateMachineOutput::Messages(outputs) => {
                let outputs = outputs.into_iter().map(|message| message.wrap(&wrapper)).collect();
                Ok(StateMachineStateOutput::Messages(self, outputs))
            }
            StateMachineOutput::Final(_) => Err(InvalidStateError.into()),
            StateMachineOutput::Empty => Ok(StateMachineStateOutput::Empty(self)),
        }
    }
}

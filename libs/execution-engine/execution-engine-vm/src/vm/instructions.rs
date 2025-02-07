//! Protocol state machine router implementation

use crate::vm::{memory::MemoryValue, sm::ExecutionContext};
use anyhow::{anyhow, Error};
use basic_types::{PartyId, PartyMessage};
use jit_compiler::models::protocols::{memory::ProtocolAddress, Protocol};
use math_lib::modular::{AsBits, ModularNumber, SafePrime};
use nada_value::{encrypted::Encrypted, NadaType, NadaValue};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer};
use state_machine::{state::RecipientMessage, StateMachine, StateMachineOutput, StateMachineState};
use std::{fmt::Debug, marker::PhantomData};

/// The statistic security parameter kappa. Used by several protocols (MODULO, MOD2M, TRUNC, DIVISION...).
/// Determines the probability of breaking the protocol. It is always set to 40.
/// Meaning there is 1 over 2^40 chances of breaking the protocol. This is a standard value.
pub const STATISTIC_KAPPA: usize = 40;

/// Get statistic k
/// Calculate the maximum value of statistic k from the size of the field and kappa.
/// Statistic k defines the maximum number of bits for numbers used as divisor in MODULO and DIVISION and other protocols.
#[allow(clippy::arithmetic_side_effects)]
pub fn get_statistic_k<T: SafePrime>() -> usize {
    (T::MODULO.bits() - STATISTIC_KAPPA - 1) / 2
}

/// Instruction result
pub enum InstructionResult<S, T>
where
    S: InstructionStateMachine<T>,
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// The instruction has finished and it could return a value
    Value {
        /// Result of the execution
        value: NadaValue<Encrypted<T>>,
    },
    /// The instruction returns nothing.
    Empty,
    /// The instruction needs communication with the other nodes. This represents the state machine
    /// will be executed and the messages that will be sent the other nodes.
    StateMachine {
        /// State of the state machine
        state_machine: S,
        /// Messages to send
        messages: Vec<RecipientMessage<PartyId, S::Message>>,
    },
    /// The instruction needs communication with the other nodes. This represents the messages that
    /// will be sent.
    InstructionMessage {
        /// Messages to send
        messages: Vec<RecipientMessage<PartyId, S::Message>>,
    },
}

/// A program instruction.
///
/// Abstracts the unit of execution for a program in the Nillion VM.
pub trait Instruction<T>: Protocol
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Type of preprocessing element that the instruction will use during the execution
    type PreprocessingElement: Default + Debug;

    /// Instruction router that knows how to delegate the messages to the corresponding
    /// ['InstructionStateMachine']
    type Router: InstructionRouter<T, Message = Self::Message>;

    /// Type of instruction messages
    type Message: Serialize + DeserializeOwned + Send + Clone + Debug;

    /// Runs the instruction
    fn run<I>(
        self,
        context: &mut ExecutionContext<I, T>,
        share_elements: Self::PreprocessingElement,
    ) -> Result<InstructionResult<Self::Router, T>, Error>
    where
        I: Instruction<T>;
}

/// Available messages are sent to the VM to execute a required protocol
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct InstructionMessage<M>
where
    M: Clone + Debug,
{
    /// Address of the protocol is being executed
    pub(crate) address: ProtocolAddress,
    /// Protocol message
    pub(crate) message: M,
}

/// Implements the instruction router. It is used by the vm to delegate the message to the
/// corresponding instruction when it receives an ['InstructionMessage']
pub trait InstructionRouter<T>: InstructionStateMachine<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
}

/// Implements the state machine of an instruction
pub trait InstructionStateMachine<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Allow message by the instruction
    type Message: Clone + Debug;

    /// Check if the state machine is finished
    fn is_finished(&self) -> bool;

    /// Delegates the handling of the instruction message to the state machine
    fn handle_message<P>(
        &mut self,
        context: &mut ExecutionContext<P, T>,
        message: PartyMessage<P::Message>,
    ) -> Result<InstructionResult<P::Router, T>, Error>
    where
        P: Instruction<T, Message = Self::Message>;
}

/// This provides a default implementation of ['InstructionStateMachine'] for the instructions that
/// return a collection of values as result
pub struct DefaultInstructionStateMachine<M, S>
where
    S: StateMachineState,
{
    /// State machine
    pub sm: StateMachine<S>,
    /// Return type
    pub return_type: NadaType,
    _unused: PhantomData<M>,
}

impl<M, S: StateMachineState> DefaultInstructionStateMachine<M, S> {
    /// Creates a new ['DefaultInstructionStateMachine']
    pub fn new(initial_state: S, return_type: NadaType) -> Self {
        Self { sm: StateMachine::new(initial_state), return_type, _unused: PhantomData }
    }
}

impl<M, S, I, F, T> InstructionStateMachine<T> for DefaultInstructionStateMachine<M, S>
where
    M: TryInto<I, Error = Error> + From<S::OutputMessage> + Clone + Debug,
    S: StateMachineState<RecipientId = PartyId, InputMessage = PartyMessage<I>, FinalResult = F>,
    F: Into<Vec<ModularNumber<T>>>,
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    type Message = M;

    fn is_finished(&self) -> bool {
        self.sm.is_finished()
    }

    fn handle_message<P>(
        &mut self,
        _context: &mut ExecutionContext<P, T>,
        message: PartyMessage<P::Message>,
    ) -> Result<InstructionResult<P::Router, T>, Error>
    where
        P: Instruction<T, Message = Self::Message>,
    {
        let (party_id, message) = message.into_parts();
        // Check if the message is understood by the instruction.
        let msg = message.try_into()?;
        // Delegates the message to the instruction state machine
        match self.sm.handle_message(PartyMessage::new(party_id, msg))? {
            StateMachineOutput::Final(values) => {
                // Manage the resulting share from the instruction state machine
                let return_type = self.return_type.clone();
                let mut values: Vec<ModularNumber<T>> = values.into();
                let value = values.pop().ok_or_else(|| anyhow!("{} result is empty", stringify!($message)))?;
                let value = NadaValue::new_memory_value(return_type, value)?;
                Ok(InstructionResult::Value { value })
            }
            StateMachineOutput::Empty => Ok(InstructionResult::Empty),
            StateMachineOutput::Messages(messages) => {
                Ok(InstructionResult::InstructionMessage { messages: into_instruction_messages(messages) })
            }
        }
    }
}

/// Transforms the resulting messages from the instruction execution into ['InstructionMessage'].
pub fn into_instruction_messages<I, T, M>(messages: I) -> Vec<RecipientMessage<PartyId, M>>
where
    I: IntoIterator<Item = RecipientMessage<PartyId, T>>,
    M: From<T>,
{
    messages.into_iter().map(|m| m.wrap(&M::from)).collect()
}

#[cfg(test)]
mod tests {
    use math_lib::modular::{U128SafePrime, U256SafePrime, U64SafePrime};

    use crate::vm::instructions::get_statistic_k;

    #[test]
    fn test_get_statistic_k() {
        assert_eq!(11, get_statistic_k::<U64SafePrime>());
        assert_eq!(43, get_statistic_k::<U128SafePrime>());
        assert_eq!(107, get_statistic_k::<U256SafePrime>());
    }
}

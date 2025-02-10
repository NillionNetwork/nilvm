//! The VM state machine.

use super::plan::{ExecutionPlan, ExecutionStep};
use crate::{
    metrics::{ExecutionMetrics, ExecutionMetricsConfig},
    vm::{
        errors::EvaluationError,
        instructions::{
            Instruction, InstructionMessage, InstructionResult, InstructionRouter, InstructionStateMachine,
        },
        memory::{RuntimeMemory, RuntimeMemoryError},
        sm::states::{Executing, WaitingBootstrap},
    },
};
use anyhow::{anyhow, Error};
use basic_types::PartyMessage;
use jit_compiler::models::protocols::{memory::ProtocolAddress, OutputMemoryAllocation};
use math_lib::modular::SafePrime;
use nada_value::{encrypted::Encrypted, NadaValue};
use serde::{Deserialize, Serialize};
use shamir_sharing::{
    party::PartyId,
    secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer},
};
use state_machine::{
    state::{Recipient, RecipientMessage},
    StateMachine, StateMachineState, StateMachineStateExt, StateMachineStateOutput, StateMachineStateResult,
};
use state_machine_derive::StateMachineState;
use std::{
    collections::{BTreeMap, HashMap},
    fmt::Debug,
    sync::Arc,
};
use uuid::Uuid;

pub(crate) mod states {
    use super::ExecutionContext;
    use crate::vm::instructions::{Instruction, InstructionMessage};
    use jit_compiler::models::protocols::memory::ProtocolAddress;
    use math_lib::modular::SafePrime;
    use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer};
    use std::collections::BTreeMap;

    /// The initial state, where no instructions have been executed.
    pub struct WaitingBootstrap<I, T>
    where
        I: Instruction<T>,
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        /// The program's execution context.
        pub(super) context: ExecutionContext<I, T>,
    }

    /// We have received a message and the VM is executing protocols.
    pub struct Executing<I, T>
    where
        I: Instruction<T>,
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        /// The protocol state machine.
        pub(crate) router: BTreeMap<ProtocolAddress, I::Router>,

        /// The state machine's execution context.
        pub(crate) context: ExecutionContext<I, T>,

        pub(crate) out_of_order_messages: Vec<InstructionMessage<I::Message>>,
    }
}

/// The VM state.
#[derive(StateMachineState)]
#[state_machine(
    recipient_id = "PartyId",
    input_message = "PartyMessage<VmStateMessage<I::Message>>",
    output_message = "VmStateMessage<I::Message>",
    final_result = "(HashMap<String, NadaValue<Encrypted<T>>>, ExecutionMetrics)",
    handle_message_fn = "Self::handle_message"
)]
pub enum VmState<I, T>
where
    I: Instruction<T>,
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// We are waiting to be bootstrapped.
    #[state_machine(completed = "true", transition_fn = "Self::transition_waiting_bootstrap")]
    WaitingBootstrap(WaitingBootstrap<I, T>),

    /// We are waiting for the protocol.
    #[state_machine(
        completed = "state.router.values().all(|router| router.is_finished())",
        transition_fn = "Self::transition_executing"
    )]
    Executing(Executing<I, T>),
}

/// Intermediate struct to simplify the API that build the protocol messages during the online
/// execution line.
struct OnlineInstructionRegistry<R, T>
where
    R: InstructionRouter<T>,
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Keeps a link from the protocol address to the protocol state machine
    router: BTreeMap<ProtocolAddress, R>,
    /// Protocol messages that will be sent to the parties. The messages are grouped into chunks,
    /// to avoid sent a message to large.
    messages: HashMap<PartyId, Vec<Vec<InstructionMessage<R::Message>>>>,
}

impl<I, T> VmState<I, T>
where
    I: Instruction<T>,
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Construct a new VM.
    pub(crate) fn new(context: ExecutionContext<I, T>) -> Self {
        VmState::WaitingBootstrap(WaitingBootstrap { context })
    }

    // Runs until we hit a synchronization point (e.g. an online step) or we complete the
    // execution.
    fn run(mut context: ExecutionContext<I, T>) -> StateMachineStateResult<Self> {
        context.execution_metrics.execution_started();
        while let Some(ExecutionStep { local, online, .. }) = context.plan.next_step() {
            context.execution_metrics.add_new_step();
            Self::run_local_protocols(&mut context, local)?;
            let OnlineInstructionRegistry { router, messages } =
                Self::register_online_instruction(&mut context, online)?;
            if !router.is_empty() {
                context.execution_metrics.update_compute_duration();
                let state = VmState::Executing(Executing { router, context, out_of_order_messages: vec![] });
                return state.wrap_communication_rounds(messages);
            }
        }
        let outputs = context.rebuild_outputs().map_err(|e| anyhow!("{e}"))?;
        context.execution_metrics.update_compute_duration();
        Ok(StateMachineStateOutput::Final((outputs, context.execution_metrics)))
    }

    /// Run the local protocols
    fn run_local_protocols(context: &mut ExecutionContext<I, T>, instructions: Vec<I>) -> Result<(), Error> {
        for instruction in instructions {
            let address = instruction.address();

            context.execution_metrics.local_protocol_started(&instruction);

            let protocol_result = instruction.run(context, I::PreprocessingElement::default())?;

            context.execution_metrics.local_protocol_ended(&address);

            match protocol_result {
                InstructionResult::Value { value } => context.store(address, value)?,
                InstructionResult::Empty => {}
                _ => return Err(anyhow!("malformed instruction: a local protocol has not finished"))?,
            }
        }
        Ok(())
    }

    /// Builds the online instruction messages that will be sent the nodes.
    fn register_online_instruction(
        context: &mut ExecutionContext<I, T>,
        instructions: Vec<(I, I::PreprocessingElement)>,
    ) -> Result<OnlineInstructionRegistry<I::Router, T>, Error> {
        let mut router = BTreeMap::new();
        let mut output_messages = HashMap::new();
        for (instruction, preprocessing_elements) in instructions {
            let address = instruction.address();
            context.execution_metrics.online_protocol_started(&instruction);
            match instruction.run(context, preprocessing_elements)? {
                // Some online protocols have optimizations based on runtime values. In these cases,
                // communication is not required, but we can't know it in compilation time.
                InstructionResult::Value { value } => {
                    context.execution_metrics.online_protocol_ended(&address);
                    context.store(address, value)?;
                }
                InstructionResult::Empty => {
                    context.execution_metrics.online_protocol_ended(&address);
                }
                InstructionResult::StateMachine { state_machine, messages } => {
                    context.execution_metrics.online_protocol_paused(&address);
                    router.insert(address, state_machine);
                    extend_communication_round(context, &mut output_messages, address, messages);
                }
                InstructionResult::InstructionMessage { .. } => {
                    return Err(anyhow!("unexpected instruction message found"));
                }
            }
        }
        Ok(OnlineInstructionRegistry { router, messages: output_messages })
    }

    fn transition_waiting_bootstrap(state: WaitingBootstrap<I, T>) -> StateMachineStateResult<Self> {
        Self::run(state.context)
    }

    fn transition_executing(state: Executing<I, T>) -> StateMachineStateResult<Self> {
        Self::run(state.context)
    }

    fn handle_message(state: Self, message: PartyMessage<VmStateMessage<I::Message>>) -> StateMachineStateResult<Self> {
        use VmStateMessage::*;
        let (party_id, message) = message.into_parts();
        match (message, state) {
            // Bootstrapping is just a means to manually trigger the first execution.
            (Bootstrap, state @ VmState::WaitingBootstrap(_)) => state.try_next(),
            (ProtocolMessages(messages), state @ VmState::Executing(_)) => state.dispatch_messages(party_id, messages),
            (message, state) => Ok(StateMachineStateOutput::OutOfOrder(state, PartyMessage::new(party_id, message))),
        }
    }

    /// Dispatches a batch of messages
    fn dispatch_messages(
        mut self,
        party_id: PartyId,
        messages: Vec<InstructionMessage<I::Message>>,
    ) -> StateMachineStateResult<Self> {
        match &mut self {
            VmState::Executing(inner) => {
                let mut output_messages = HashMap::new();
                // Dispatches every message
                for message in messages {
                    let address = message.address;
                    inner.context.execution_metrics.protocol_resumed(&address);
                    if let Some(router) = inner.router.get_mut(&address) {
                        let protocol_message = PartyMessage::new(party_id.clone(), message.message);
                        // Delegates the handling of the protocol message to the protocol state machine
                        let protocol_result = router.handle_message(&mut inner.context, protocol_message)?;
                        match protocol_result {
                            InstructionResult::Value { value } => {
                                inner.context.execution_metrics.online_protocol_ended(&address);
                                inner.context.store(address, value).map_err(|e| anyhow!("{e}"))?;
                            }
                            InstructionResult::InstructionMessage { messages } => {
                                inner.context.execution_metrics.online_protocol_paused(&address);
                                extend_communication_round(&mut inner.context, &mut output_messages, address, messages);
                            }
                            InstructionResult::Empty => {
                                inner.context.execution_metrics.online_protocol_paused(&address);
                            }
                            InstructionResult::StateMachine { .. } => {
                                Err(anyhow!("unexpected instruction state machine register"))?
                            }
                        }
                    } else {
                        inner.context.execution_metrics.online_protocol_paused(&address);
                        inner.out_of_order_messages.push(message);
                    }
                }
                if !output_messages.is_empty() {
                    self.wrap_communication_rounds(output_messages)
                } else if !inner.out_of_order_messages.is_empty() {
                    let out_of_order_messages = std::mem::take(&mut inner.out_of_order_messages);
                    let party_messages =
                        PartyMessage::new(party_id, VmStateMessage::ProtocolMessages(out_of_order_messages));
                    Ok(StateMachineStateOutput::OutOfOrder(self, party_messages))
                } else {
                    self.advance_if_completed()
                }
            }
            // If the state is not `Executing`, we could receive messages that will be processed later.
            _ => {
                let message = PartyMessage::new(party_id, VmStateMessage::ProtocolMessages(messages));
                Ok(StateMachineStateOutput::OutOfOrder(self, message))
            }
        }
    }

    /// This creates the communication rounds with all protocol messages.
    /// If the total of messages is larger than the nodes accept (max_protocol_message_count),
    /// they are already organized into different communication rounds.
    fn wrap_communication_rounds(
        self,
        rounds: HashMap<PartyId, Vec<Vec<InstructionMessage<I::Message>>>>,
    ) -> StateMachineStateResult<Self> {
        let mut wrapped_rounds = vec![];
        for (party_id, party_rounds) in rounds {
            let recipient = Recipient::Single(party_id);
            let party_round: Vec<_> = party_rounds
                .into_iter()
                .map(|r| RecipientMessage::new(recipient.clone(), VmStateMessage::ProtocolMessages(r)))
                .collect();
            wrapped_rounds.extend(party_round);
        }

        Ok(StateMachineStateOutput::Messages(self, wrapped_rounds))
    }
}

/// Accumulates the messages resulting from protocols according to their destination party.
/// If the number of protocol messages in a communication round exceeds the limit that is accepted
/// by the nodes, the messages are organized in more than one communication round.
pub(crate) fn extend_communication_round<I, T>(
    context: &mut ExecutionContext<I, T>,
    all_party_rounds: &mut HashMap<PartyId, Vec<Vec<InstructionMessage<I::Message>>>>,
    address: ProtocolAddress,
    protocol_messages: Vec<RecipientMessage<PartyId, I::Message>>,
) where
    I: Instruction<T>,
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    let msg_wrapper = |message| InstructionMessage { address, message };
    let mut protocol_messages_content = Vec::with_capacity(protocol_messages.len());
    let mut is_new_round_required = true;
    for recipient_message in protocol_messages {
        let (recipient, message) = recipient_message.into_parts();
        let message = msg_wrapper(message);
        let parties = match recipient {
            Recipient::Single(party_id) => vec![party_id],
            Recipient::Multiple(parties) => parties,
        };
        for party in parties {
            // We accumulate the protocol messages, but they are split into chunks to avoid sending
            // a message to large.
            let party_rounds = all_party_rounds.entry(party).or_default();
            match party_rounds.last_mut() {
                // The communication round does not accept more messages.
                Some(last) if last.len() == context.max_protocol_messages_count => {
                    party_rounds.push(vec![message.clone()])
                }
                // The messages are pushed into the communication round.
                Some(last) => {
                    is_new_round_required = false;
                    last.push(message.clone())
                }
                // Create the first communication round
                None => party_rounds.push(vec![message.clone()]),
            }
        }
        protocol_messages_content.push(message);
    }
    if is_new_round_required {
        context.execution_metrics.add_step_round();
    }
    context.execution_metrics.add_protocol_round(&address, &protocol_messages_content);
}

pub(crate) type VmStateMachine<I, T> = StateMachine<VmState<I, T>>;

/// A message for the VM state machine.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[repr(u8)]
pub enum VmStateMessage<M: Clone + Debug> {
    /// Bootstrap the execution of the program.
    Bootstrap = 0,

    /// A message for a protocol state machine.
    ProtocolMessages(Vec<InstructionMessage<M>>) = 1,
}

/// An error returned during the VM creation.
#[derive(Debug, thiserror::Error)]
pub enum VmCreateError {
    /// There are no instructions in the execution of the program.
    #[error("program has no instructions")]
    NoInstructions,
}

/// Execution context implementation.
///
/// The execution context is shared with all the protocols during execution (invoking `run()`)
pub struct ExecutionContext<I, T>
where
    I: Instruction<T>,
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// The execution plan contains the protocols in the order they will be executed.
    plan: ExecutionPlan<I, T>,
    /// Shamir secret sharer
    secret_sharer: Arc<ShamirSecretSharer<T>>,
    /// Runtime memory that contains the values during the program execution
    pub memory: RuntimeMemory<T>,
    /// Max number of protocol messages by communication round
    max_protocol_messages_count: usize,
    /// Metrics on the execution
    pub execution_metrics: ExecutionMetrics,
    /// The compute action identifier.
    pub compute_id: Uuid,
}

impl<I, T> ExecutionContext<I, T>
where
    I: Instruction<T>,
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    pub(crate) fn new(
        plan: ExecutionPlan<I, T>,
        secret_sharer: Arc<ShamirSecretSharer<T>>,
        memory: RuntimeMemory<T>,
        max_protocol_messages_count: usize,
        metrics_config: ExecutionMetricsConfig,
        compute_id: Uuid,
    ) -> Self {
        Self {
            plan: plan.reverse(),
            secret_sharer,
            memory,
            max_protocol_messages_count,
            execution_metrics: ExecutionMetrics::new(metrics_config),
            compute_id,
        }
    }

    /// Returns the secret sharer for the execution
    pub fn secret_sharer(&self) -> Arc<ShamirSecretSharer<T>> {
        self.secret_sharer.clone()
    }

    pub(crate) fn store(
        &mut self,
        address: ProtocolAddress,
        value: NadaValue<Encrypted<T>>,
    ) -> Result<(), RuntimeMemoryError> {
        self.memory.store(address, value)
    }

    /// Read the result of a protocol
    pub fn read(&mut self, address: ProtocolAddress) -> Result<NadaValue<Encrypted<T>>, RuntimeMemoryError> {
        self.memory.read_value(address)
    }

    /// This method rebuild the outputs from the runtime memory. For this purpose, the output_memory_scheme is used.
    pub(crate) fn rebuild_outputs(&mut self) -> Result<HashMap<String, NadaValue<Encrypted<T>>>, EvaluationError> {
        let mut outputs: HashMap<String, NadaValue<Encrypted<T>>> = HashMap::new();
        for (output_name, OutputMemoryAllocation { ty, address }) in self.memory.output_memory_scheme.clone() {
            let output = self
                .memory
                .read_value(address)
                .map_err(|e| EvaluationError::OutputRetrieveError(output_name.clone(), e.to_string()))?;
            let output_type = output.to_type();
            if ty != output_type {
                return Err(EvaluationError::OutputRetrieveError(
                    output_name,
                    format!("types do not match: expected {ty}, found {output_type}"),
                ));
            }
            outputs.insert(output_name, output);
        }
        Ok(outputs)
    }
}

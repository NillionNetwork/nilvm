//! Symmetric protocol simulator.
//!
//! A protocol is considered to be symmetric if all nodes in the network assume the same role within the protocol. In
//! other words, there is a single role a node can take, which means all nodes are running the exact same steps
//! to progress the protocol.
//!
//! An example of a symmetric protocol is any protocol involved in the pre-processing phase. In those, all nodes
//! are doing the same: the protocol advances step by step and produces the same output (or shares of it) in all
//! of them once it's completed.
//!
//! An example of a protocol that is not symmetric is the one to store a secret, as the node that submits the share
//! has a very different role than the rest of them.

use anyhow::{anyhow, Error};
use basic_types::{PartyId, PartyMessage};
use rayon::prelude::*;
use state_machine::{
    sm::StateMachineOutput,
    state::{Recipient, StateMachineMessage},
    StateMachine, StateMachineState,
};
use std::{collections::HashMap, time::Instant};
use uuid::Uuid;

/// A symmetric protocol simulator.
///
/// This simulator takes some basic configurations as input and can run any type that implements
/// [`Protocol`][Protocol] until its completion.
///
/// Note that there is no networking involved in the execution of this simulator. It instead simply acts as a basic
/// router that takes output messages from the protocol being run and forwards them to the target party. The goal
/// of this is to allow complex protocols to be easily tested by simply providing a way to instantiate it.
///
/// This type implements [`Default`], providing some default prime number and generator which should satisfy any
/// common protocol test.
#[derive(Clone)]
pub struct SymmetricProtocolSimulator {
    max_rounds: usize,
    network_size: usize,
    diagnostics: bool,
}

impl SymmetricProtocolSimulator {
    /// Construct a new simulator.
    ///
    /// # Arguments
    /// - `network_size` - The number of nodes in the simulated network.
    /// - `max_rounds` - The maximum number of rounds to perform before the protocol is assumed to be stuck in a loop.
    pub fn new(network_size: usize, max_rounds: usize) -> Self {
        Self { max_rounds, network_size, diagnostics: true }
    }

    /// Enable/disable diagnostics during the protocol execution.
    ///
    /// If diagnostics are disabled, no print statements will be emitted by the simulator. This is
    /// used during benchmarks so as not to cause slowdowns/spam stdout.
    pub fn with_diagnostics(mut self, diagnostics: bool) -> Self {
        self.diagnostics = diagnostics;
        self
    }

    /// Runs the given protocol and returns its output.
    ///
    /// # Arguments
    /// - `protocol` - The protocol to be run.
    /// - `network_size` - The size of the network to use.
    pub fn run_protocol<P: Protocol, M>(&self, protocol: &P) -> Result<Vec<PartyOutput<P::State>>, Error>
    where
        P::State: StateMachineState<InputMessage = PartyMessage<M>, OutputMessage = M> + Send + Sync,
        <P::State as StateMachineState>::InputMessage: Sync + Send,
        M: Clone + Send,
    {
        let context = self.initialize_protocol(protocol)?;
        let start_time = Instant::now();
        let result = self.run_until_completion(context);
        let elapsed = start_time.elapsed();
        if self.diagnostics {
            println!("Protocol execution took {}ms", elapsed.as_millis());
        }
        result
    }

    fn run_until_completion<S, M>(&self, context: ProtocolContext<S>) -> Result<Vec<PartyOutput<S>>, Error>
    where
        S: StateMachineState<RecipientId = PartyId, InputMessage = PartyMessage<M>, OutputMessage = M> + Send + Sync,
        S::InputMessage: Sync + Send,
        M: Clone + Send,
    {
        let mut party_states = context.party_states;
        let mut next_round_messages = context.initial_messages;
        let mut round_id = 0;
        let mut outputs = Vec::new();
        let expected_outputs = party_states.party_count();
        loop {
            // Take this round's messages so we can collect the next round's messages in `messages`.
            let round_messages = std::mem::take(&mut next_round_messages);
            if round_messages.is_empty() {
                return Err(anyhow!("started round {round_id} without any messages"));
            }
            if self.diagnostics {
                println!("Running round {round_id} using {} messages", round_messages.len());
            }
            for message in round_messages {
                let (sender_party_id, message) = message.into_parts();
                let (recipients, message) = message.into_parts();
                match recipients {
                    Recipient::Single(party_id) => {
                        party_states.add_party_message(party_id, PartyMessage::new(sender_party_id, message))?
                    }
                    Recipient::Multiple(party_ids) => {
                        for party_id in party_ids {
                            party_states.add_party_message(
                                party_id,
                                PartyMessage::new(sender_party_id.clone(), message.clone()),
                            )?;
                        }
                    }
                };
            }

            // Apply the messages for every party in parallel and collect the results.
            let round_results: Vec<_> =
                party_states.states.par_iter_mut().map(|(_, party_state)| party_state.apply_messages()).collect();
            for result in round_results {
                match result? {
                    PartyRoundOutput::Completed(output) => {
                        outputs.push(output);
                        if outputs.len() == expected_outputs {
                            return Ok(outputs);
                        }
                    }
                    PartyRoundOutput::Messages(messages) => next_round_messages.extend(messages),
                };
            }

            round_id += 1;
            if round_id >= self.max_rounds {
                return Err(anyhow!("exceeded maximum number of rounds without completing protocol"));
            }
        }
    }

    fn initialize_protocol<P: Protocol>(&self, protocol: &P) -> Result<ProtocolContext<P::State>, Error> {
        let mut parties = Vec::new();
        for _ in 0..self.network_size {
            parties.push(PartyId::from(Uuid::new_v4()));
        }
        let prepare = protocol.prepare(&parties)?;

        let mut context = ProtocolContext::default();
        for party_id in &parties {
            let InitializedProtocol { state, initial_messages } = protocol
                .initialize(party_id.clone(), &prepare)
                .map_err(|e| anyhow!("failed to initialize protocol: {e}"))?;
            context.party_states.add_party(party_id.clone(), state);
            let initial_messages =
                initial_messages.into_iter().map(|message| PartyMessage::new(party_id.clone(), message));
            context.initial_messages.extend(initial_messages);
        }
        Ok(context)
    }
}

enum PartyRoundOutput<S: StateMachineState> {
    Completed(PartyOutput<S>),
    Messages(Vec<PartyMessage<StateMachineMessage<S>>>),
}

struct PartyState<S: StateMachineState> {
    party_id: PartyId,
    state_machine: StateMachine<S>,
    input_messages: Vec<S::InputMessage>,
}

impl<S: StateMachineState> PartyState<S> {
    fn new(party_id: PartyId, state: S) -> Self {
        Self { party_id, state_machine: StateMachine::new(state), input_messages: Vec::new() }
    }

    fn apply_messages(&mut self) -> Result<PartyRoundOutput<S>, Error> {
        let mut next_round_messages = Vec::new();
        for message in std::mem::take(&mut self.input_messages) {
            match self.state_machine.handle_message(message) {
                Ok(StateMachineOutput::Final(output)) => {
                    return Ok(PartyRoundOutput::Completed(PartyOutput::new(self.party_id.clone(), output)));
                }
                Ok(StateMachineOutput::Messages(messages)) => {
                    let messages =
                        messages.into_iter().map(|message| PartyMessage::new(self.party_id.clone(), message));
                    next_round_messages.extend(messages)
                }
                Ok(StateMachineOutput::Empty) => (),
                Err(e) => return Err(anyhow!("failed to handle message: {e}")),
            }
        }
        Ok(PartyRoundOutput::Messages(next_round_messages))
    }
}

struct PartyStates<S: StateMachineState> {
    states: HashMap<PartyId, PartyState<S>>,
}

impl<S: StateMachineState> PartyStates<S> {
    fn add_party(&mut self, party_id: PartyId, state: S) {
        self.states.insert(party_id.clone(), PartyState::new(party_id, state));
    }

    fn add_party_message(&mut self, party_id: PartyId, message: S::InputMessage) -> Result<(), Error> {
        let party_state =
            self.states.get_mut(&party_id).ok_or_else(|| anyhow!("state for party {party_id:?} not found"))?;
        party_state.input_messages.push(message);
        Ok(())
    }

    fn party_count(&self) -> usize {
        self.states.len()
    }
}

struct ProtocolContext<S: StateMachineState> {
    party_states: PartyStates<S>,
    initial_messages: Vec<PartyMessage<StateMachineMessage<S>>>,
}

impl<S: StateMachineState> Default for ProtocolContext<S> {
    fn default() -> Self {
        Self { party_states: PartyStates { states: HashMap::new() }, initial_messages: Vec::new() }
    }
}

/// The final output for the instance of the protocol being run by a particular party.
pub struct PartyOutput<S: StateMachineState> {
    /// The party id.
    pub party_id: PartyId,

    /// The output itself.
    pub output: S::FinalResult,
}

impl<S: StateMachineState> PartyOutput<S> {
    /// Construct a new `PartyOutput`.
    pub fn new(party_id: PartyId, output: S::FinalResult) -> Self {
        Self { party_id, output }
    }
}

/// A protocol abstraction.
///
/// The main concept being abstracted is the initialization of the protocol. Once a protocol is initialized and
/// we have the initial set of messages that it generates, it should be able to run on its own by feeding messages
/// into the instance of the protocol being run by each party until its completion.
pub trait Protocol {
    /// The protocol state to be instantiated.
    type State: StateMachineState<RecipientId = PartyId>;

    /// The output of the prepare initialization step.
    ///
    /// This is a customization point for protocols that require its initialization to be tied to the network
    /// configuration or prime numbers to be used. For example, some protocols may require the set of party ids to
    /// generate a set of shares and then feed each share to the right instance of each protocol. In this example,
    /// the set of shares would be set as output of the prepare phase and the initialization of the protocol itself
    /// would perform the latter using that output.
    type PrepareOutput;

    /// Prepare the execution of the protocol.
    ///
    /// This should perform any configuration specific initializations that will be needed later on during the
    /// party specific initialization. The output of this function will be fed to the
    /// [`initialize`][Protocol::initialize] function.
    fn prepare(&self, parties: &[PartyId]) -> Result<Self::PrepareOutput, Error>;

    /// Initialize a protocol for a particular party.
    ///
    /// This should instantiate the state machine for this protocol and include all of the initialization
    /// messages that it generates as part of it.
    ///
    /// # Arguments.
    /// - `local_party_id` - The id of the party that's this instantiation is for.
    /// - `prepare` - The output of the call to [`prepare`][Protocol::prepare].
    fn initialize(
        &self,
        local_party_id: PartyId,
        prepare_output: &Self::PrepareOutput,
    ) -> Result<InitializedProtocol<Self::State>, Error>;
}

/// An initialized protocol, along with the messages it produced during initialization.
pub struct InitializedProtocol<S: StateMachineState> {
    /// The protocol's state.
    pub state: S,

    /// The initial set of messages the protocol generated.
    pub initial_messages: Vec<StateMachineMessage<S>>,
}

impl<S: StateMachineState> InitializedProtocol<S> {
    /// Constructs a new protocol.
    pub fn new(state: S, initial_messages: Vec<StateMachineMessage<S>>) -> Self {
        Self { state, initial_messages }
    }
}

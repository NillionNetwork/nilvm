//! The ECDSA-AUX-INFO protocol state machine.
//!
//! This state machine generates the Auxiliary information required by the Threshold ECDSA Signatures protocol.
use crate::threshold_ecdsa::util::SortedParties;
use anyhow::anyhow;
use basic_types::{PartyId, PartyMessage};
use cggmp21::{
    key_refresh::msg::aux_only::Msg,
    key_share::{AuxInfo, DirtyAuxInfo, Valid},
    round_based::state_machine::{ProceedResult, StateMachine},
    rug::Integer,
    security_level::SecurityLevel128,
    ExecutionId, KeyRefreshError, PregeneratedPrimes,
};
use rand::RngCore;
use round_based::{Incoming, MessageDestination, MessageType, Outgoing};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use state_machine::{
    errors::StateMachineError,
    state::{Recipient, RecipientMessage, StateMachineMessage},
    StateMachineState, StateMachineStateOutput, StateMachineStateResult,
};
use std::{
    fmt::Display,
    sync::{
        mpsc::{Receiver, Sender},
        Mutex,
    },
    thread,
};

use super::output::{EcdsaAuxInfo, EcdsaAuxInfoOutput};

type EcdsaAuxInfoIncomingMessage = Incoming<Msg<Sha256, SecurityLevel128>>;
type EcdsaAuxInfoOutgoingMessage = ProceedResult<Result<AuxInfo, KeyRefreshError>, Msg<Sha256, SecurityLevel128>>;
type AuxInfoResult = Result<Valid<DirtyAuxInfo>, KeyRefreshError>;
type ProceedResultType = ProceedResult<AuxInfoResult, Msg<Sha256, SecurityLevel128>>;

/// The pregenerated prime mode to use.
#[derive(Clone, Debug, Default)]
pub enum PregeneratedPrimesMode {
    /// Use random integers.
    ///
    /// This should be used in production.
    #[default]
    Random,

    /// Use a fixed pair of integers.
    ///
    /// This should only be used when testing.
    Fixed {
        /// The p prime.
        p: Integer,

        /// The q prime.
        q: Integer,
    },
}

/// Proxy for the message types in Aux Info state machine.
#[derive(Clone, Serialize, Deserialize)]
enum AuxInfoStateMessageType {
    /// Broadcast message type
    Broadcast,
    /// Peer to Peer message type
    P2P,
}

/// Represents the messages sent between internal rounds of the Auxiliary Info protocol.
#[derive(Clone, Serialize, Deserialize)]
pub struct RoundStateMessage {
    msg: Msg<Sha256, SecurityLevel128>,
    msg_type: AuxInfoStateMessageType,
}

/// Threshold ECDSA Auxiliary Info state machine.
pub struct EcdsaAuxInfoState {
    sm_join_handle: thread::JoinHandle<Result<(), StateMachineError>>,
    pub(crate) sender: Sender<EcdsaAuxInfoIncomingMessage>,
    pub(crate) receiver: Mutex<Receiver<EcdsaAuxInfoOutgoingMessage>>,
    sorted_parties: SortedParties,
}

impl StateMachineState for EcdsaAuxInfoState {
    type RecipientId = PartyId;
    type InputMessage = PartyMessage<EcdsaAuxInfoStateMessage>;
    type OutputMessage = EcdsaAuxInfoStateMessage;
    type FinalResult = EcdsaAuxInfoOutput<EcdsaAuxInfo>;

    fn is_completed(&self) -> bool {
        false
    }

    fn try_next(self) -> StateMachineStateResult<Self> {
        Err(StateMachineError::UnexpectedError(anyhow!(
            "we never call this internally, and should never be called externally"
        )))
    }

    fn handle_message(self, message: Self::InputMessage) -> StateMachineStateResult<Self> {
        // Send receive message to state machine
        let message = self.to_cggmp21_sm_messages(message)?;
        self.sender.send(message).map_err(|e| StateMachineError::ChannelDropped(e.to_string()))?;

        // collect messages from their state machine to be sent to other parties
        let mut outgoing_messages = vec![];
        {
            let receiver = self.receiver.lock().map_err(|_| {
                StateMachineError::ChannelDropped("unexpected error when accessing the receiver".to_string())
            })?;
            loop {
                let result = receiver.recv().map_err(|e| StateMachineError::ChannelDropped(e.to_string()))?;
                match result {
                    EcdsaAuxInfoOutgoingMessage::SendMsg(msg) => {
                        outgoing_messages.push(msg);
                    }
                    EcdsaAuxInfoOutgoingMessage::NeedsOneMoreMessage => break,
                    EcdsaAuxInfoOutgoingMessage::Output(output) => {
                        (self.sm_join_handle.join().map_err(|e| {
                            StateMachineError::UnexpectedError(anyhow!("error in cggmp21 state machine thread: {e:?}"))
                        })?)?;
                        return match output {
                            Ok(aux_info) => {
                                let aux_info_element = EcdsaAuxInfo { aux_info };
                                Ok(StateMachineStateOutput::Final(EcdsaAuxInfoOutput::Success {
                                    element: aux_info_element,
                                }))
                            }
                            Err(error) => {
                                Ok(StateMachineStateOutput::Final(EcdsaAuxInfoOutput::Abort { reason: error }))
                            }
                        };
                    }
                    EcdsaAuxInfoOutgoingMessage::Yielded => continue,
                    EcdsaAuxInfoOutgoingMessage::Error(e) => {
                        return Err(StateMachineError::UnexpectedError(e.into()));
                    }
                };
            }
        }

        // Build our state machine messages
        let messages = self.to_nillion_sm_messages(outgoing_messages)?;
        if messages.is_empty() {
            Ok(StateMachineStateOutput::Empty(self))
        } else {
            Ok(StateMachineStateOutput::Messages(self, messages))
        }
    }
}

impl Display for EcdsaAuxInfoState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "EcdsaAuxInfoState")
    }
}

impl EcdsaAuxInfoState {
    /// Construct a new ECDSA-AUX-INFO state.
    pub fn new(
        eid: Vec<u8>,
        parties: Vec<PartyId>,
        party: PartyId,
        mode: PregeneratedPrimesMode,
    ) -> Result<(Self, Vec<StateMachineMessage<Self>>), EcdsaAuxInfoCreateError> {
        // Create channels for our state machine to communicate with their state machine
        let (sender_to_aux_info, receiver_from_nillion_sm) = std::sync::mpsc::channel();
        let (sender_to_nillion_sm, receiver_from_aux_info) = std::sync::mpsc::channel();

        // compute input elements required for their state machine
        let sorted_parties = SortedParties::new(parties);
        let party_index = sorted_parties.index(party).map_err(|e| EcdsaAuxInfoCreateError::Unexpected(e.into()))?;
        let parties_len = sorted_parties.len();

        // Spawn cggmp21 StateMachine in a separate thread.
        let join_handle = thread::spawn(move || -> Result<(), StateMachineError> {
            EcdsaAuxInfoState::run_cggmp21_aux_info_sm(
                eid,
                party_index,
                parties_len,
                sender_to_nillion_sm,
                receiver_from_nillion_sm,
                mode,
            )
            .map_err(|_| StateMachineError::UnexpectedError(anyhow!("unexpected error inside their state machine")))?;
            Ok(())
        });

        // Get initial round of messages from their state machine
        let state = EcdsaAuxInfoState {
            sm_join_handle: join_handle,
            sender: sender_to_aux_info,
            receiver: Mutex::new(receiver_from_aux_info),
            sorted_parties,
        };
        let outgoing_messages = state.collect_initial_messages()?;

        // Transform their message into our messages
        let messages =
            state.to_nillion_sm_messages(outgoing_messages).map_err(|_| EcdsaAuxInfoCreateError::PartyNotFound)?;

        Ok((state, messages))
    }

    fn run_cggmp21_aux_info_sm(
        eid: Vec<u8>,
        party_index: u16,
        parties_len: u16,
        sender_to_nillion_sm: Sender<ProceedResultType>,
        receiver_from_nillion_sm: Receiver<Incoming<Msg<Sha256, SecurityLevel128>>>,
        mode: PregeneratedPrimesMode,
    ) -> Result<(), EcdsaAuxInfoCreateError> {
        // state machine initialization
        let mut rng = rand::thread_rng();
        let eid = ExecutionId::new(&eid);
        let pregenerated_primes = match mode {
            PregeneratedPrimesMode::Random => PregeneratedPrimes::<SecurityLevel128>::generate(&mut rand::thread_rng()),
            PregeneratedPrimesMode::Fixed { p, q } => PregeneratedPrimes::<SecurityLevel128>::new(p, q)
                .ok_or(EcdsaAuxInfoCreateError::PregeneratedPrimesTooSmall)?,
        };
        let mut aux_info_sm =
            cggmp21::aux_info_gen(eid, party_index, parties_len, pregenerated_primes).into_state_machine(&mut rng);

        // run state machine
        loop {
            let result = aux_info_sm.proceed();
            if matches!(result, ProceedResult::Output(_)) {
                sender_to_nillion_sm
                    .send(result)
                    .map_err(|e| EcdsaAuxInfoCreateError::ChannelDropped(format!("sender result error: {:?}", e)))?;
                break;
            } else if matches!(result, ProceedResult::NeedsOneMoreMessage) {
                sender_to_nillion_sm
                    .send(result)
                    .map_err(|e| EcdsaAuxInfoCreateError::ChannelDropped(format!("sender result error: {:?}", e)))?;
                let received = receiver_from_nillion_sm
                    .recv()
                    .map_err(|e| EcdsaAuxInfoCreateError::ChannelDropped(format!("receiver result error: {:?}", e)))?;
                let _ = aux_info_sm.received_msg(received);
            } else {
                sender_to_nillion_sm
                    .send(result)
                    .map_err(|e| EcdsaAuxInfoCreateError::ChannelDropped(format!("send result error: {:?}", e)))?;
            }
        }
        Ok(())
    }

    // Transforms nillion (our) messages into cggmp21 (their) state machine messages
    fn to_cggmp21_sm_messages(
        &self,
        incoming_message: <crate::threshold_ecdsa::auxiliary_information::state::EcdsaAuxInfoState as StateMachineState>::InputMessage,
    ) -> Result<Incoming<Msg<Sha256, SecurityLevel128>>, StateMachineError> {
        let sender = incoming_message.sender;
        let EcdsaAuxInfoStateMessage::Message(input_message) = incoming_message.message;
        let message = Incoming {
            id: rand::thread_rng().next_u64(),
            sender: self.sorted_parties.index(sender)?,
            msg_type: match input_message.msg_type {
                AuxInfoStateMessageType::Broadcast => MessageType::Broadcast,
                AuxInfoStateMessageType::P2P => MessageType::P2P,
            },
            msg: input_message.msg,
        };
        Ok(message)
    }

    // Transforms cggmp21 (their) messages into nillion (our) state machine messages
    fn to_nillion_sm_messages(
        &self,
        outgoing_messages: Vec<Outgoing<Msg<Sha256, SecurityLevel128>>>,
    ) -> Result<Vec<RecipientMessage<PartyId, EcdsaAuxInfoStateMessage>>, StateMachineError> {
        let mut messages = vec![];
        for message in outgoing_messages {
            let recipient_message = match message.recipient {
                MessageDestination::AllParties => {
                    let msg = RoundStateMessage { msg: message.msg, msg_type: AuxInfoStateMessageType::Broadcast };
                    RecipientMessage::new(
                        Recipient::Multiple(self.sorted_parties.parties()),
                        EcdsaAuxInfoStateMessage::Message(msg),
                    )
                }
                MessageDestination::OneParty(party_index) => {
                    let party = self.sorted_parties.party(party_index)?;
                    let msg = RoundStateMessage { msg: message.msg, msg_type: AuxInfoStateMessageType::P2P };
                    RecipientMessage::new(Recipient::Single(party), EcdsaAuxInfoStateMessage::Message(msg))
                }
            };
            messages.push(recipient_message);
        }
        Ok(messages)
    }

    // Collects initial set of messages from their state machine to be sent to other parties
    fn collect_initial_messages(
        &self,
    ) -> Result<Vec<Outgoing<Msg<Sha256, SecurityLevel128>>>, EcdsaAuxInfoCreateError> {
        let mut outgoing_messages = vec![];

        // Lock receiver and handle errors
        let receiver = self.receiver.lock().map_err(|_| {
            EcdsaAuxInfoCreateError::Unexpected(anyhow!("unexpected error when accessing the receiver"))
        })?;

        loop {
            let result = receiver.recv().map_err(|e| EcdsaAuxInfoCreateError::Unexpected(e.into()))?;
            match result {
                EcdsaAuxInfoOutgoingMessage::SendMsg(msg) => {
                    outgoing_messages.push(msg);
                }
                EcdsaAuxInfoOutgoingMessage::Yielded => continue,
                EcdsaAuxInfoOutgoingMessage::Error(e) => {
                    return Err(EcdsaAuxInfoCreateError::Unexpected(e.into()));
                }
                EcdsaAuxInfoOutgoingMessage::NeedsOneMoreMessage => break,
                _ => return Err(EcdsaAuxInfoCreateError::Unexpected(anyhow!("unexpected state when received"))),
            };
        }

        Ok(outgoing_messages)
    }
}

/// A message for the ECDSA-AUX-INFO protocol.
#[derive(Clone, Serialize, Deserialize)]
#[repr(u8)]
pub enum EcdsaAuxInfoStateMessage {
    /// A message for the ECDSA-AUX-INFO state machine.
    Message(RoundStateMessage) = 0,
}

/// An error during the ECDSA-AUX-INFO state creation.
#[derive(Debug, thiserror::Error)]
pub enum EcdsaAuxInfoCreateError {
    /// PartyId not found in the set of computing parties.
    #[error("PartyId not found in the set of computing parties.")]
    PartyNotFound,

    /// Unexpected error
    #[error("unexpected error: {0}")]
    Unexpected(anyhow::Error),

    /// Unexpected error inside cggmp21 state machine thread
    #[error("channel dropped in cggmp21 state machine: {0}")]
    ChannelDropped(String),

    /// Pregenerated primes are too small.
    #[error("pre-generated primes are too small")]
    PregeneratedPrimesTooSmall,
}

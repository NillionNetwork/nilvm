//! The DKG protocol state machine.
//!
//! This state machine generates the shares of the ECDSA private key. It uses the CGGMP21 DKG protocol.
use crate::{distributed_key_generation::dkg::output::EcdsaKeyGenOutput, threshold_ecdsa::util::SortedParties};
use anyhow::anyhow;
use basic_types::{PartyId, PartyMessage};
use cggmp21::{
    keygen::msg::non_threshold::Msg,
    round_based::state_machine::{ProceedResult, StateMachine},
    security_level::SecurityLevel128,
    supported_curves::Secp256k1,
    ExecutionId, KeygenError,
};
use key_share::{DirtyCoreKeyShare, Valid};
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
    fmt::{self, Display},
    sync::{
        mpsc::{Receiver, Sender},
        Mutex,
    },
    thread,
};

use ecdsa_keypair::privatekey::EcdsaPrivateKeyShare;

type EcdsaKeyGenIncomingMessage = Incoming<Msg<Secp256k1, SecurityLevel128, Sha256>>;
type EcdsaKeyGenOutgoingMessage =
    ProceedResult<Result<Valid<DirtyCoreKeyShare<Secp256k1>>, KeygenError>, Msg<Secp256k1, SecurityLevel128, Sha256>>;
type KeyGenResult = Result<Valid<DirtyCoreKeyShare<Secp256k1>>, KeygenError>;
type ProceedResultType = ProceedResult<KeyGenResult, Msg<Secp256k1, SecurityLevel128, Sha256>>;
type EcdsaKeyGenMessage = Msg<Secp256k1, SecurityLevel128, Sha256>;
type OutgoingEcdsaKeyGenMessage = Outgoing<EcdsaKeyGenMessage>;
type CollectMessagesResult = Result<Vec<OutgoingEcdsaKeyGenMessage>, EcdsaKeyGenError>;

/// Proxy for the message types in DKG state machine.
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub enum KeyGenStateMessageType {
    /// Broadcast message type
    Broadcast,
    /// Peer to Peer message type
    P2P,
}

/// Represents the messages sent between internal rounds of the DKG protocol.
#[derive(Clone, Serialize, Deserialize)]
pub struct RoundStateMessage {
    /// Message exchanged between internal rounds of the DKG protocol.
    /// This field will be None only if a decoding error occurs in
    /// `StateMachineMessage<EcdsaKeyGenStateMessage>::encoded_bytes_as_output_message`
    /// when attempting to deserialize an EcdsaKeyGenStateMessage.
    pub msg: Option<Msg<Secp256k1, SecurityLevel128, Sha256>>,
    /// The type of message sent between internal rounds of the DKG protocol.
    pub msg_type: KeyGenStateMessageType,
}

impl fmt::Debug for RoundStateMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Print msg_type first
        write!(f, "RoundStateMessage {{ msg_type: {:?}, ", self.msg_type)?;

        // Match on the msg enum to print details based on the current round
        match &self.msg {
            Some(Msg::Round1(_)) => write!(f, "round: Round1")?,
            Some(Msg::Round2(_)) => write!(f, "round: Round2")?,
            Some(Msg::Round3(_)) => write!(f, "round: Round3")?,
            Some(Msg::ReliabilityCheck(_)) => write!(f, "round: ReliabilityCheck")?,
            None => write!(f, "round: None")?,
        }

        write!(f, " }}")
    }
}

impl PartialEq for RoundStateMessage {
    fn eq(&self, other: &Self) -> bool {
        // First, check if the msg_type is equal
        if self.msg_type != other.msg_type {
            return false;
        }

        // Next, match on the `msg` enum to compare the variant and the associated data
        match (&self.msg, &other.msg) {
            // If both are in the same round variant, return true
            (Some(Msg::Round1(_)), Some(Msg::Round1(_))) => true,
            (Some(Msg::Round2(_)), Some(Msg::Round2(_))) => true,
            (Some(Msg::Round3(_)), Some(Msg::Round3(_))) => true,
            (Some(Msg::ReliabilityCheck(_)), Some(Msg::ReliabilityCheck(_))) => true,
            (None, None) => true,

            // If the variants are different, return false
            _ => false,
        }
    }
}

/// Distributed Key Generation state machine.
pub struct EcdsaKeyGenState {
    sm_join_handle: thread::JoinHandle<Result<(), StateMachineError>>,
    pub(crate) sender: Sender<EcdsaKeyGenIncomingMessage>,
    pub(crate) receiver: Mutex<Receiver<EcdsaKeyGenOutgoingMessage>>,
    sorted_parties: SortedParties,
}

impl StateMachineState for EcdsaKeyGenState {
    type RecipientId = PartyId;
    type InputMessage = PartyMessage<EcdsaKeyGenStateMessage>;
    type OutputMessage = EcdsaKeyGenStateMessage;
    type FinalResult = EcdsaKeyGenOutput;

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
                    EcdsaKeyGenOutgoingMessage::SendMsg(msg) => {
                        outgoing_messages.push(msg);
                    }
                    EcdsaKeyGenOutgoingMessage::NeedsOneMoreMessage => break,
                    EcdsaKeyGenOutgoingMessage::Output(output) => {
                        (self.sm_join_handle.join().map_err(|e| {
                            StateMachineError::UnexpectedError(anyhow!("error in cggmp21 state machine thread: {e:?}"))
                        })?)?;
                        return match output {
                            Ok(private_key_share) => Ok(StateMachineStateOutput::Final(EcdsaKeyGenOutput::Success {
                                element: EcdsaPrivateKeyShare::new(private_key_share),
                            })),
                            Err(error) => Ok(StateMachineStateOutput::Final(EcdsaKeyGenOutput::Abort {
                                reason: error.to_string(),
                            })),
                        };
                    }
                    EcdsaKeyGenOutgoingMessage::Yielded => continue,
                    EcdsaKeyGenOutgoingMessage::Error(e) => {
                        return Err(StateMachineError::UnexpectedError(e.into()));
                    }
                };
            }
        }

        // Build our state machine messages
        let messages = self.to_nillion_sm_messages(outgoing_messages)?;

        Ok(StateMachineStateOutput::Messages(self, messages))
    }
}

impl Display for EcdsaKeyGenState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "EcdsaKeyGenState")
    }
}

impl EcdsaKeyGenState {
    /// Construct a new ECDSA-DKG state.
    pub fn new(
        eid: Vec<u8>,
        parties: Vec<PartyId>,
        party: PartyId,
    ) -> Result<(Self, Vec<StateMachineMessage<Self>>), EcdsaKeyGenError> {
        // Create channels for our state machine to communicate with their state machine
        let (sender_to_keygen, receiver_from_nillion_sm) = std::sync::mpsc::channel();
        let (sender_to_nillion_sm, receiver_from_keygen) = std::sync::mpsc::channel();

        // compute input elements required for their state machine
        let sorted_parties = SortedParties::new(parties);
        let party_index = sorted_parties.index(party).map_err(|e| EcdsaKeyGenError::Unexpected(e.into()))?;
        let parties_len = sorted_parties.len();

        // Spawn cggmp21 StateMachine in a separate thread.
        let join_handle = thread::spawn(move || -> Result<(), StateMachineError> {
            EcdsaKeyGenState::run_distributed_key_generation_sm(
                eid,
                party_index,
                parties_len,
                sender_to_nillion_sm,
                receiver_from_nillion_sm,
            )
            .map_err(|_| {
                StateMachineError::UnexpectedError(anyhow!("unexpected error inside CGGMP21 state machine"))
            })?;
            Ok(())
        });

        // Get initial round of messages from their state machine
        let state = EcdsaKeyGenState {
            sm_join_handle: join_handle,
            sender: sender_to_keygen,
            receiver: Mutex::new(receiver_from_keygen),
            sorted_parties,
        };
        let outgoing_messages = state.collect_initial_messages()?;

        // Transform their message into our messages
        let messages = state.to_nillion_sm_messages(outgoing_messages).map_err(|_| EcdsaKeyGenError::PartyNotFound)?;

        Ok((state, messages))
    }

    fn run_distributed_key_generation_sm(
        eid: Vec<u8>,
        party_index: u16,
        parties_len: u16,
        sender_to_nillion_sm: Sender<ProceedResultType>,
        receiver_from_nillion_sm: Receiver<Incoming<Msg<Secp256k1, SecurityLevel128, Sha256>>>,
    ) -> Result<(), EcdsaKeyGenError> {
        // state machine initialization
        let mut rng = rand::thread_rng();
        let eid = ExecutionId::new(&eid);
        let mut keygen_sm = cggmp21::keygen::<Secp256k1>(eid, party_index, parties_len).into_state_machine(&mut rng);

        // run state machine
        loop {
            let result = keygen_sm.proceed();
            if matches!(result, ProceedResult::Output(_)) {
                sender_to_nillion_sm
                    .send(result)
                    .map_err(|e| EcdsaKeyGenError::ChannelDropped(format!("sender result error: {:?}", e)))?;
                break;
            } else if matches!(result, ProceedResult::NeedsOneMoreMessage) {
                sender_to_nillion_sm
                    .send(result)
                    .map_err(|e| EcdsaKeyGenError::ChannelDropped(format!("sender result error: {:?}", e)))?;
                let received = receiver_from_nillion_sm
                    .recv()
                    .map_err(|e| EcdsaKeyGenError::ChannelDropped(format!("receiver result error: {:?}", e)))?;
                let _ = keygen_sm.received_msg(received);
            } else {
                sender_to_nillion_sm
                    .send(result)
                    .map_err(|e| EcdsaKeyGenError::ChannelDropped(format!("send result error: {:?}", e)))?;
            }
        }
        Ok(())
    }

    // Transforms nillion (our) messages into cggmp21 (their) state machine messages
    fn to_cggmp21_sm_messages(
        &self,
        incoming_message: <crate::distributed_key_generation::dkg::state::EcdsaKeyGenState as StateMachineState>::InputMessage,
    ) -> Result<Incoming<Msg<Secp256k1, SecurityLevel128, Sha256>>, StateMachineError> {
        let sender = incoming_message.sender;
        let EcdsaKeyGenStateMessage::Message(input_message) = incoming_message.message;
        let message = Incoming {
            id: rand::thread_rng().next_u64(),
            sender: self.sorted_parties.index(sender)?,
            msg_type: match input_message.msg_type {
                KeyGenStateMessageType::Broadcast => MessageType::Broadcast,
                KeyGenStateMessageType::P2P => MessageType::P2P,
            },
            msg: input_message.msg.ok_or_else(|| StateMachineError::UnexpectedError(anyhow!(
                "Message was None. This indicates a decoding error occurred in \
                `impl StateMachineMessage<EcdsaKeyGenStateMessage> for EcdsaKeyGenStateMessage::encoded_bytes_as_output_message`. \
                Check that the message bytes can be properly decoded into an EcdsaKeyGenStateMessage."
            )))?,
        };
        Ok(message)
    }

    // Transforms cggmp21 (their) messages into nillion (our) state machine messages
    fn to_nillion_sm_messages(
        &self,
        outgoing_messages: Vec<Outgoing<Msg<Secp256k1, SecurityLevel128, Sha256>>>,
    ) -> Result<Vec<RecipientMessage<PartyId, EcdsaKeyGenStateMessage>>, StateMachineError> {
        let mut messages = vec![];
        for message in outgoing_messages {
            let recipient_message = match message.recipient {
                MessageDestination::AllParties => {
                    let msg = RoundStateMessage { msg: Some(message.msg), msg_type: KeyGenStateMessageType::Broadcast };
                    RecipientMessage::new(
                        Recipient::Multiple(self.sorted_parties.parties()),
                        EcdsaKeyGenStateMessage::Message(msg),
                    )
                }
                MessageDestination::OneParty(party_index) => {
                    let party = self.sorted_parties.party(party_index)?;
                    let msg = RoundStateMessage { msg: Some(message.msg), msg_type: KeyGenStateMessageType::P2P };
                    RecipientMessage::new(Recipient::Single(party), EcdsaKeyGenStateMessage::Message(msg))
                }
            };
            messages.push(recipient_message);
        }
        Ok(messages)
    }

    // Collects initial set of messages from their state machine to be sent to other parties
    fn collect_initial_messages(&self) -> CollectMessagesResult {
        let mut outgoing_messages = vec![];

        // Lock receiver and handle errors
        let receiver = self
            .receiver
            .lock()
            .map_err(|_| EcdsaKeyGenError::Unexpected(anyhow!("unexpected error when accessing the receiver")))?;

        loop {
            let result = receiver.recv().map_err(|e| EcdsaKeyGenError::Unexpected(e.into()))?;
            match result {
                EcdsaKeyGenOutgoingMessage::SendMsg(msg) => {
                    outgoing_messages.push(msg);
                }
                EcdsaKeyGenOutgoingMessage::Yielded => continue,
                EcdsaKeyGenOutgoingMessage::Error(e) => {
                    return Err(EcdsaKeyGenError::Unexpected(e.into()));
                }
                EcdsaKeyGenOutgoingMessage::NeedsOneMoreMessage => break,
                _ => return Err(EcdsaKeyGenError::Unexpected(anyhow!("unexpected state when received"))),
            };
        }

        Ok(outgoing_messages)
    }
}

/// A message for the ECDSA-DKG protocol.
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
#[repr(u8)]
pub enum EcdsaKeyGenStateMessage {
    /// A message for the ECDSA-DKG state machine.
    Message(RoundStateMessage) = 0,
}

/// An error during the ECDSA-DKG state creation.
#[derive(Debug, thiserror::Error)]
pub enum EcdsaKeyGenError {
    /// PartyId not found in the set of computing parties.
    #[error("partyId not found in the set of computing parties.")]
    PartyNotFound,

    /// joining key share and auxiliary information failed.
    #[error("joining key share and auxiliary information failed")]
    ValidateError,

    /// Key share provided is not valid.
    #[error("key share provided is not valid.")]
    InvalidKeyShare,

    /// Unexpected error
    #[error("unexpected error: {0}")]
    Unexpected(anyhow::Error),

    /// Unexpected error inside cggmp21 state machine thread
    #[error("channel dropped in cggmp21 state machine: {0}")]
    ChannelDropped(String),
}

//! The DKG protocol state machine.
//!
//! This state machine generates the shares of the ECDSA private key. It uses the CGGMP21 DKG protocol.
use crate::{distributed_key_generation::dkg::output::KeyGenOutput, threshold_ecdsa::util::SortedParties};
use anyhow::anyhow;
use basic_types::{PartyId, PartyMessage};
use cggmp21::{
    generic_ec::{curves::Ed25519, Curve},
    keygen::{msg::non_threshold::Msg, GenericKeygenBuilder, NonThreshold},
    round_based::state_machine::{ProceedResult, StateMachine},
    security_level::SecurityLevel128,
    supported_curves::Secp256k1,
    ExecutionId, KeygenError,
};
use key_share::{CoreKeyShare, DirtyCoreKeyShare, Valid};
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
    mem,
    sync::{
        mpsc::{Receiver, Sender},
        Mutex,
    },
    thread,
};
use threshold_keypair::privatekey::ThresholdPrivateKeyShare;

type KeyGenIncomingMessage<C> = Incoming<Msg<C, SecurityLevel128, Sha256>>;
type KeyGenOutgoingMessage<C> =
    ProceedResult<Result<Valid<DirtyCoreKeyShare<C>>, KeygenError>, Msg<C, SecurityLevel128, Sha256>>;
type KeyGenResult<C> = Result<Valid<DirtyCoreKeyShare<C>>, KeygenError>;
type ProceedResultType<C> = ProceedResult<KeyGenResult<C>, Msg<C, SecurityLevel128, Sha256>>;
type KeyGenMessage<C> = Msg<C, SecurityLevel128, Sha256>;
type KeyGenRecipientMessages<C> = Vec<RecipientMessage<PartyId, KeyGenStateMessage<C>>>;
type OutgoingKeyGenMessage<C> = Outgoing<KeyGenMessage<C>>;
type CollectMessagesResult<C> = Result<Vec<OutgoingKeyGenMessage<C>>, KeyGenError>;

/// The state for an ECDSA keygen state machine.
pub type EcdsaKeyGenState = KeyGenState<Secp256k1Protocol>;

/// A message for the ECDSA keygen state machine.
pub type EcdsaKeyGenStateMessage = KeyGenStateMessage<Secp256k1>;

/// The output of an ECDSA keygen state machine.
pub type EcdsaKeyGenOutput = KeyGenOutput<ThresholdPrivateKeyShare<Secp256k1>>;

/// The state for an EdDSA keygen state machine.
pub type EddsaKeyGenState = KeyGenState<Ed25519Protocol>;

/// A message for the EdDSA keygen state machine.
pub type EddsaKeyGenStateMessage = KeyGenStateMessage<Ed25519>;

/// The output of an EdDSA keygen state machine.
pub type EddsaKeyGenOutput = KeyGenOutput<ThresholdPrivateKeyShare<Ed25519>>;

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
pub struct KeyGenRoundStateMessage<C: Curve> {
    /// Message exchanged between internal rounds of the DKG protocol.
    /// This field will be None only if a decoding error occurs in
    /// `StateMachineMessage<KeyGenStateMessage>::encoded_bytes_as_output_message`
    /// when attempting to deserialize an KeyGenStateMessage.
    #[serde(bound = "")]
    pub msg: Option<Msg<C, SecurityLevel128, Sha256>>,

    /// The type of message sent between internal rounds of the DKG protocol.
    pub msg_type: KeyGenStateMessageType,
}

impl<C: Curve> fmt::Debug for KeyGenRoundStateMessage<C> {
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

impl<T: Curve> PartialEq for KeyGenRoundStateMessage<T> {
    fn eq(&self, other: &Self) -> bool {
        // Assume that if the enum variant is the same, they're the same
        self.msg_type == other.msg_type && mem::discriminant(&self.msg) == mem::discriminant(&other.msg)
    }
}

/// Distributed Key Generation state machine.
pub struct KeyGenState<P: CurveProtocol> {
    sm_join_handle: thread::JoinHandle<Result<(), StateMachineError>>,
    sender: Sender<KeyGenIncomingMessage<P::Curve>>,
    receiver: Mutex<Receiver<KeyGenOutgoingMessage<P::Curve>>>,
    sorted_parties: SortedParties,
}

impl<P: CurveProtocol> StateMachineState for KeyGenState<P> {
    type RecipientId = PartyId;
    type InputMessage = PartyMessage<KeyGenStateMessage<P::Curve>>;
    type OutputMessage = KeyGenStateMessage<P::Curve>;
    type FinalResult = KeyGenOutput<P::Output>;

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
                    KeyGenOutgoingMessage::SendMsg(msg) => {
                        outgoing_messages.push(msg);
                    }
                    KeyGenOutgoingMessage::NeedsOneMoreMessage => break,
                    KeyGenOutgoingMessage::Output(output) => {
                        (self.sm_join_handle.join().map_err(|e| {
                            StateMachineError::UnexpectedError(anyhow!("error in cggmp21 state machine thread: {e:?}"))
                        })?)?;
                        return match output {
                            Ok(private_key_share) => Ok(StateMachineStateOutput::Final(KeyGenOutput::Success {
                                element: P::Output::from(private_key_share),
                            })),
                            Err(error) => {
                                Ok(StateMachineStateOutput::Final(KeyGenOutput::Abort { reason: error.to_string() }))
                            }
                        };
                    }
                    KeyGenOutgoingMessage::Yielded => continue,
                    KeyGenOutgoingMessage::Error(e) => {
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

impl<C: CurveProtocol> Display for KeyGenState<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "KeyGenState")
    }
}

impl<P: CurveProtocol> KeyGenState<P> {
    /// Construct a new ECDSA-DKG state.
    pub fn new(
        eid: Vec<u8>,
        parties: Vec<PartyId>,
        party: PartyId,
    ) -> Result<(Self, Vec<StateMachineMessage<Self>>), KeyGenError> {
        // Create channels for our state machine to communicate with their state machine
        let (sender_to_keygen, receiver_from_nillion_sm) = std::sync::mpsc::channel();
        let (sender_to_nillion_sm, receiver_from_keygen) = std::sync::mpsc::channel();

        // compute input elements required for their state machine
        let sorted_parties = SortedParties::new(parties);
        let party_index = sorted_parties.index(party).map_err(|e| KeyGenError::Unexpected(e.into()))?;
        let parties_len = sorted_parties.len();

        // Spawn cggmp21 StateMachine in a separate thread.
        let join_handle = thread::spawn(move || {
            Self::run_dkg(eid, party_index, parties_len, sender_to_nillion_sm, receiver_from_nillion_sm).map_err(
                |_| StateMachineError::UnexpectedError(anyhow!("unexpected error inside CGGMP21 state machine")),
            )?;
            Ok(())
        });

        // Get initial round of messages from their state machine
        let state = KeyGenState {
            sm_join_handle: join_handle,
            sender: sender_to_keygen,
            receiver: Mutex::new(receiver_from_keygen),
            sorted_parties,
        };
        let outgoing_messages = state.collect_initial_messages()?;

        // Transform their message into our messages
        let messages = state.to_nillion_sm_messages(outgoing_messages).map_err(|_| KeyGenError::PartyNotFound)?;

        Ok((state, messages))
    }

    // Transforms nillion (our) messages into cggmp21 (their) state machine messages
    fn to_cggmp21_sm_messages(
        &self,
        incoming_message: <Self as StateMachineState>::InputMessage,
    ) -> Result<Incoming<Msg<P::Curve, SecurityLevel128, Sha256>>, StateMachineError> {
        let sender = incoming_message.sender;
        let KeyGenStateMessage::Message(input_message) = incoming_message.message;
        let message = Incoming {
            id: rand::thread_rng().next_u64(),
            sender: self.sorted_parties.index(sender)?,
            msg_type: match input_message.msg_type {
                KeyGenStateMessageType::Broadcast => MessageType::Broadcast,
                KeyGenStateMessageType::P2P => MessageType::P2P,
            },
            msg: input_message.msg.ok_or_else(|| StateMachineError::UnexpectedError(anyhow!(
                "Message was None. This indicates a decoding error occurred in \
                `impl StateMachineMessage<KeyGenStateMessage> for KeyGenStateMessage::encoded_bytes_as_output_message`. \
                Check that the message bytes can be properly decoded into an KeyGenStateMessage."
            )))?,
        };
        Ok(message)
    }

    // Transforms cggmp21 (their) messages into nillion (our) state machine messages
    fn to_nillion_sm_messages(
        &self,
        outgoing_messages: Vec<Outgoing<Msg<P::Curve, SecurityLevel128, Sha256>>>,
    ) -> Result<KeyGenRecipientMessages<P::Curve>, StateMachineError> {
        let mut messages = vec![];
        for message in outgoing_messages {
            let recipient_message = match message.recipient {
                MessageDestination::AllParties => {
                    let msg =
                        KeyGenRoundStateMessage { msg: Some(message.msg), msg_type: KeyGenStateMessageType::Broadcast };
                    RecipientMessage::new(
                        Recipient::Multiple(self.sorted_parties.parties()),
                        KeyGenStateMessage::Message(msg),
                    )
                }
                MessageDestination::OneParty(party_index) => {
                    let party = self.sorted_parties.party(party_index)?;
                    let msg = KeyGenRoundStateMessage { msg: Some(message.msg), msg_type: KeyGenStateMessageType::P2P };
                    RecipientMessage::new(Recipient::Single(party), KeyGenStateMessage::Message(msg))
                }
            };
            messages.push(recipient_message);
        }
        Ok(messages)
    }

    // Collects initial set of messages from their state machine to be sent to other parties
    fn collect_initial_messages(&self) -> CollectMessagesResult<P::Curve> {
        let mut outgoing_messages = vec![];

        // Lock receiver and handle errors
        let receiver = self
            .receiver
            .lock()
            .map_err(|_| KeyGenError::Unexpected(anyhow!("unexpected error when accessing the receiver")))?;

        loop {
            let result = receiver.recv().map_err(|e| KeyGenError::Unexpected(e.into()))?;
            match result {
                KeyGenOutgoingMessage::SendMsg(msg) => {
                    outgoing_messages.push(msg);
                }
                KeyGenOutgoingMessage::Yielded => continue,
                KeyGenOutgoingMessage::Error(e) => {
                    return Err(KeyGenError::Unexpected(e.into()));
                }
                KeyGenOutgoingMessage::NeedsOneMoreMessage => break,
                _ => return Err(KeyGenError::Unexpected(anyhow!("unexpected state when received"))),
            };
        }

        Ok(outgoing_messages)
    }

    fn run_dkg(
        eid: Vec<u8>,
        party_index: u16,
        parties_len: u16,
        sender_to_nillion_sm: Sender<ProceedResultType<P::Curve>>,
        receiver_from_nillion_sm: Receiver<Incoming<Msg<P::Curve, SecurityLevel128, Sha256>>>,
    ) -> Result<(), KeyGenError> {
        // state machine initialization
        let mut rng = rand::thread_rng();
        let eid = ExecutionId::new(&eid);
        let mut keygen_sm = P::keygen_builder(eid, party_index, parties_len).into_state_machine(&mut rng);

        // run state machine
        loop {
            let result = keygen_sm.proceed();
            if matches!(result, ProceedResult::Output(_)) {
                sender_to_nillion_sm
                    .send(result)
                    .map_err(|e| KeyGenError::ChannelDropped(format!("sender result error: {:?}", e)))?;
                break;
            } else if matches!(result, ProceedResult::NeedsOneMoreMessage) {
                sender_to_nillion_sm
                    .send(result)
                    .map_err(|e| KeyGenError::ChannelDropped(format!("sender result error: {:?}", e)))?;
                let received = receiver_from_nillion_sm
                    .recv()
                    .map_err(|e| KeyGenError::ChannelDropped(format!("receiver result error: {:?}", e)))?;
                let _ = keygen_sm.received_msg(received);
            } else {
                sender_to_nillion_sm
                    .send(result)
                    .map_err(|e| KeyGenError::ChannelDropped(format!("send result error: {:?}", e)))?;
            }
        }
        Ok(())
    }
}

/// A protocol for a specific curve.
pub trait CurveProtocol {
    /// The curve used by this protocol.
    type Curve: Curve + 'static;

    /// This protocol's output.
    type Output: Send + Clone + From<CoreKeyShare<Self::Curve>>;

    /// Create a keygen builder for this protocol.
    fn keygen_builder(
        eid: ExecutionId<'_>,
        party_index: u16,
        total_parties: u16,
    ) -> GenericKeygenBuilder<'_, Self::Curve, NonThreshold, SecurityLevel128, Sha256>;
}

/// An identifier for the secp256k1 protocol.
pub struct Secp256k1Protocol;

impl CurveProtocol for Secp256k1Protocol {
    type Curve = Secp256k1;
    type Output = ThresholdPrivateKeyShare<Secp256k1>;

    fn keygen_builder(
        eid: ExecutionId<'_>,
        party_index: u16,
        total_parties: u16,
    ) -> GenericKeygenBuilder<'_, Self::Curve, NonThreshold, SecurityLevel128, Sha256> {
        cggmp21::keygen(eid, party_index, total_parties)
    }
}

/// An identifier for the Ed25519 protocol.
pub struct Ed25519Protocol;

impl CurveProtocol for Ed25519Protocol {
    type Curve = Ed25519;
    type Output = ThresholdPrivateKeyShare<Ed25519>;

    fn keygen_builder(
        eid: ExecutionId<'_>,
        party_index: u16,
        total_parties: u16,
    ) -> GenericKeygenBuilder<'_, Self::Curve, NonThreshold, SecurityLevel128, Sha256> {
        cggmp21::keygen(eid, party_index, total_parties)
    }
}

/// A message for the ECDSA-DKG protocol.
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
#[serde(bound = "")]
#[repr(u8)]
pub enum KeyGenStateMessage<C: Curve> {
    /// A message for the ECDSA-DKG state machine.
    Message(KeyGenRoundStateMessage<C>) = 0,
}

/// An error during the ECDSA-DKG state creation.
#[derive(Debug, thiserror::Error)]
pub enum KeyGenError {
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

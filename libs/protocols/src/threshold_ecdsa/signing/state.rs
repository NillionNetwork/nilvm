//! The ECDSA-SIGNING protocol state machine.
//!
//! This state machine generates the shares of the ECDSA signature. It uses the CGGMP21 Threhsold ECDSA protocol.
use crate::threshold_ecdsa::{
    auxiliary_information::output::EcdsaAuxInfo, signing::output::EcdsaSignatureShareOutput, util::SortedParties,
};
use anyhow::anyhow;
use basic_types::{PartyId, PartyMessage};
use cggmp21::{
    generic_ec::Scalar,
    round_based::state_machine::{ProceedResult, StateMachine},
    signing::{msg::Msg, Presignature, SigningError},
    supported_curves::Secp256k1,
    DataToSign, ExecutionId,
};
use key_share::Validate;
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

use ecdsa_keypair::{privatekey::EcdsaPrivateKeyShare, signature::EcdsaSignatureShare};

type EcdsaSignIncomingMessage = Incoming<Msg<Secp256k1, Sha256>>;
type EcdsaSignOutgoingMessage = ProceedResult<Result<Presignature<Secp256k1>, SigningError>, Msg<Secp256k1, Sha256>>;
type SignResult = Result<Presignature<Secp256k1>, SigningError>;
type ProceedResultType = ProceedResult<SignResult, Msg<Secp256k1, Sha256>>;

/// Proxy for the message types in Signing state machine.
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
enum SignStateMessageType {
    /// Broadcast message type
    Broadcast,
    /// Peer to Peer message type
    P2P,
}

/// Represents the messages sent between internal rounds of the Signing protocol.
#[derive(Clone, Serialize, Deserialize)]
pub struct RoundStateMessage {
    msg: Msg<Secp256k1, Sha256>,
    msg_type: SignStateMessageType,
}

impl fmt::Debug for RoundStateMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Print msg_type first
        write!(f, "RoundStateMessage {{ msg_type: {:?}, ", self.msg_type)?;

        // Match on the msg enum to print details based on the current round
        match &self.msg {
            Msg::Round1a(_) => write!(f, "round: Round1a")?,
            Msg::Round1b(_) => write!(f, "round: Round1b")?,
            Msg::Round2(_) => write!(f, "round: Round2")?,
            Msg::Round3(_) => write!(f, "round: Round3")?,
            Msg::Round4(_) => write!(f, "round: Round4")?,
            Msg::ReliabilityCheck(_) => write!(f, "round: ReliabilityCheck")?,
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
            (Msg::Round1a(_), Msg::Round1a(_)) => true,
            (Msg::Round1b(_), Msg::Round1b(_)) => true,
            (Msg::Round2(_), Msg::Round2(_)) => true,
            (Msg::Round3(_), Msg::Round3(_)) => true,
            (Msg::Round4(_), Msg::Round4(_)) => true,
            (Msg::ReliabilityCheck(_), Msg::ReliabilityCheck(_)) => true,

            // If the variants are different, return false
            _ => false,
        }
    }
}

/// Threshold ECDSA signing state machine.
pub struct EcdsaSignState {
    sm_join_handle: thread::JoinHandle<Result<(), StateMachineError>>,
    pub(crate) sender: Sender<EcdsaSignIncomingMessage>,
    pub(crate) receiver: Mutex<Receiver<EcdsaSignOutgoingMessage>>,
    sorted_parties: SortedParties,
    message_digest: [u8; 32],
}

impl StateMachineState for EcdsaSignState {
    type RecipientId = PartyId;
    type InputMessage = PartyMessage<EcdsaSignStateMessage>;
    type OutputMessage = EcdsaSignStateMessage;
    type FinalResult = EcdsaSignatureShareOutput;

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
                    EcdsaSignOutgoingMessage::SendMsg(msg) => {
                        outgoing_messages.push(msg);
                    }
                    EcdsaSignOutgoingMessage::NeedsOneMoreMessage => break,
                    EcdsaSignOutgoingMessage::Output(output) => {
                        (self.sm_join_handle.join().map_err(|e| {
                            StateMachineError::UnexpectedError(anyhow!("error in cggmp21 state machine thread: {e:?}"))
                        })?)?;
                        return match output {
                            Ok(presignature) => {
                                // Transforms bytes to DataToSign object through a Scalar
                                let message_to_sign =
                                    DataToSign::from_scalar(Scalar::from_be_bytes_mod_order(self.message_digest));
                                // Generates a partial signature, i.e. shares of the signature
                                let share = presignature.issue_partial_signature(message_to_sign);
                                // Outputs with the correct format
                                let ecdsa_signature_share = EcdsaSignatureShare { r: share.r, sigma: share.sigma };
                                Ok(StateMachineStateOutput::Final(EcdsaSignatureShareOutput::Success {
                                    element: ecdsa_signature_share,
                                }))
                            }
                            Err(error) => {
                                Ok(StateMachineStateOutput::Final(EcdsaSignatureShareOutput::Abort { reason: error }))
                            }
                        };
                    }
                    EcdsaSignOutgoingMessage::Yielded => continue,
                    EcdsaSignOutgoingMessage::Error(e) => {
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

impl Display for EcdsaSignState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "EcdsaSignState")
    }
}

impl EcdsaSignState {
    /// Construct a new ECDSA-SIGNING state.
    pub fn new(
        eid: Vec<u8>,
        parties: Vec<PartyId>,
        party: PartyId,
        incomplete_key_share: EcdsaPrivateKeyShare,
        aux_info: EcdsaAuxInfo,
        message_digest: [u8; 32],
    ) -> Result<(Self, Vec<StateMachineMessage<Self>>), EcdsaSignError> {
        // Create channels for our state machine to communicate with their state machine
        let (sender_to_signing, receiver_from_nillion_sm) = std::sync::mpsc::channel();
        let (sender_to_nillion_sm, receiver_from_signing) = std::sync::mpsc::channel();

        // compute input elements required for their state machine
        let sorted_parties = SortedParties::new(parties);
        let party_index = sorted_parties.index(party).map_err(|e| EcdsaSignError::Unexpected(e.into()))?;
        let parties_len = sorted_parties.len();

        // Spawn cggmp21 StateMachine in a separate thread.
        let join_handle = thread::spawn(move || -> Result<(), StateMachineError> {
            EcdsaSignState::run_cggmp21_signature_sm(
                eid,
                party_index,
                parties_len,
                incomplete_key_share,
                aux_info,
                sender_to_nillion_sm,
                receiver_from_nillion_sm,
            )
            .map_err(|_| {
                StateMachineError::UnexpectedError(anyhow!("unexpected error inside CGGMP21 state machine"))
            })?;
            Ok(())
        });

        // Get initial round of messages from their state machine
        let state = EcdsaSignState {
            sm_join_handle: join_handle,
            sender: sender_to_signing,
            receiver: Mutex::new(receiver_from_signing),
            sorted_parties,
            message_digest,
        };
        let outgoing_messages = state.collect_initial_messages()?;

        // Transform their message into our messages
        let messages = state.to_nillion_sm_messages(outgoing_messages).map_err(|_| EcdsaSignError::PartyNotFound)?;

        Ok((state, messages))
    }

    fn run_cggmp21_signature_sm(
        eid: Vec<u8>,
        party_index: u16,
        parties_len: u16,
        incomplete_key_share: EcdsaPrivateKeyShare,
        aux_info: EcdsaAuxInfo,
        sender_to_nillion_sm: Sender<ProceedResultType>,
        receiver_from_nillion_sm: Receiver<Incoming<Msg<Secp256k1, Sha256>>>,
    ) -> Result<(), EcdsaSignError> {
        // state machine initialization
        let mut rng = rand::thread_rng();
        let eid = ExecutionId::new(&eid);
        let parties_indexes_at_keygen: Vec<u16> = (0..parties_len).collect();
        let incomplete_key_share =
            incomplete_key_share.into_inner().into_inner().validate().map_err(|_| EcdsaSignError::InvalidKeyShare)?;
        let key_share = cggmp21::KeyShare::from_parts((incomplete_key_share, aux_info.aux_info))
            .map_err(|_| EcdsaSignError::ValidateError)?;
        let mut signature_sm = cggmp21::signing(eid, party_index, &parties_indexes_at_keygen, &key_share)
            .generate_presignature_sync(&mut rng);

        // run state machine
        loop {
            let result = signature_sm.proceed();
            if matches!(result, ProceedResult::Output(_)) {
                sender_to_nillion_sm
                    .send(result)
                    .map_err(|e| EcdsaSignError::ChannelDropped(format!("sender result error: {:?}", e)))?;
                break;
            } else if matches!(result, ProceedResult::NeedsOneMoreMessage) {
                sender_to_nillion_sm
                    .send(result)
                    .map_err(|e| EcdsaSignError::ChannelDropped(format!("sender result error: {:?}", e)))?;
                let received = receiver_from_nillion_sm
                    .recv()
                    .map_err(|e| EcdsaSignError::ChannelDropped(format!("receiver result error: {:?}", e)))?;
                let _ = signature_sm.received_msg(received);
            } else {
                sender_to_nillion_sm
                    .send(result)
                    .map_err(|e| EcdsaSignError::ChannelDropped(format!("send result error: {:?}", e)))?;
            }
        }
        Ok(())
    }

    // Transforms nillion (our) messages into cggmp21 (their) state machine messages
    fn to_cggmp21_sm_messages(
        &self,
        incoming_message: <crate::threshold_ecdsa::signing::state::EcdsaSignState as StateMachineState>::InputMessage,
    ) -> Result<Incoming<Msg<Secp256k1, Sha256>>, StateMachineError> {
        let sender = incoming_message.sender;
        let EcdsaSignStateMessage::Message(input_message) = incoming_message.message;
        let message = Incoming {
            id: rand::thread_rng().next_u64(),
            sender: self.sorted_parties.index(sender)?,
            msg_type: match input_message.msg_type {
                SignStateMessageType::Broadcast => MessageType::Broadcast,
                SignStateMessageType::P2P => MessageType::P2P,
            },
            msg: input_message.msg,
        };
        Ok(message)
    }

    // Transforms cggmp21 (their) messages into nillion (our) state machine messages
    fn to_nillion_sm_messages(
        &self,
        outgoing_messages: Vec<Outgoing<Msg<Secp256k1, Sha256>>>,
    ) -> Result<Vec<RecipientMessage<PartyId, EcdsaSignStateMessage>>, StateMachineError> {
        let mut messages = vec![];
        for message in outgoing_messages {
            let recipient_message = match message.recipient {
                MessageDestination::AllParties => {
                    let msg = RoundStateMessage { msg: message.msg, msg_type: SignStateMessageType::Broadcast };
                    RecipientMessage::new(
                        Recipient::Multiple(self.sorted_parties.parties()),
                        EcdsaSignStateMessage::Message(msg),
                    )
                }
                MessageDestination::OneParty(party_index) => {
                    let party = self.sorted_parties.party(party_index)?;
                    let msg = RoundStateMessage { msg: message.msg, msg_type: SignStateMessageType::P2P };
                    RecipientMessage::new(Recipient::Single(party), EcdsaSignStateMessage::Message(msg))
                }
            };
            messages.push(recipient_message);
        }
        Ok(messages)
    }

    // Collects initial set of messages from their state machine to be sent to other parties
    fn collect_initial_messages(&self) -> Result<Vec<Outgoing<Msg<Secp256k1, Sha256>>>, EcdsaSignError> {
        let mut outgoing_messages = vec![];

        // Lock receiver and handle errors
        let receiver = self
            .receiver
            .lock()
            .map_err(|_| EcdsaSignError::Unexpected(anyhow!("unexpected error when accessing the receiver")))?;

        loop {
            let result = receiver.recv().map_err(|e| EcdsaSignError::Unexpected(e.into()))?;
            match result {
                EcdsaSignOutgoingMessage::SendMsg(msg) => {
                    outgoing_messages.push(msg);
                }
                EcdsaSignOutgoingMessage::Yielded => continue,
                EcdsaSignOutgoingMessage::Error(e) => {
                    return Err(EcdsaSignError::Unexpected(e.into()));
                }
                EcdsaSignOutgoingMessage::NeedsOneMoreMessage => break,
                _ => return Err(EcdsaSignError::Unexpected(anyhow!("unexpected state when received"))),
            };
        }

        Ok(outgoing_messages)
    }
}

/// A message for the ECDSA-SIGNING protocol.
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
#[repr(u8)]
pub enum EcdsaSignStateMessage {
    /// A message for the ECDSA-SIGNING state machine.
    Message(RoundStateMessage) = 0,
}

/// An error during the ECDSA-SIGNING state creation.
#[derive(Debug, thiserror::Error)]
pub enum EcdsaSignError {
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

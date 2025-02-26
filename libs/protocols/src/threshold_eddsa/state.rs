//! The EdDSA-SIGNING protocol state machine.
//!
//! This state machine generates the EdDSA signature. It uses the FROST signing protocol.

use crate::{threshold_ecdsa::util::SortedParties, threshold_eddsa::output::EddsaSignatureOutput};
use threshold_keypair::{privatekey::ThresholdPrivateKeyShare, signature::EddsaSignature};

use std::fmt;

use anyhow::{anyhow, Context};
use basic_types::{jar::PartyJar, PartyMessage};
use cggmp21::generic_ec::curves::Ed25519;

use givre::{
    ciphersuite,
    signing::{aggregate::aggregate, full_signing::Msg, round1::commit, round2::sign},
};
use key_share::Validate;

use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use state_machine::{
    state::{Recipient, RecipientMessage, StateMachineMessage},
    StateMachineStateExt, StateMachineStateOutput, StateMachineStateResult,
};
use state_machine_derive::StateMachineState;

use shamir_sharing::party::PartyId;

/// The Threshold EdDSA protocol state definitions.
pub mod states {
    use basic_types::jar::PartyJar;
    use cggmp21::generic_ec::curves::Ed25519;
    use givre::signing::{
        round1::{PublicCommitments, SecretNonces},
        round2::SigShare,
    };
    use key_share::{CoreKeyShare, KeyInfo};

    use crate::threshold_ecdsa::util::SortedParties;

    /// The protocol is waiting for the pre processing phase to finish
    pub struct WaitingPublicCommits {
        /// Key Share
        pub(crate) key_share: CoreKeyShare<Ed25519>,
        /// Nonces from pre processing phase
        pub(crate) secret_nonces: SecretNonces<Ed25519>,
        /// Vector of Public Commitments from pre processing phase
        pub(crate) pcommits: PartyJar<PublicCommitments<Ed25519>>,
        /// Message to sign
        pub(crate) msg: Vec<u8>,
        /// Sorted Parties
        pub(crate) sorted_parties: SortedParties,
    }

    /// The protocol is waiting for the signatures shares
    /// from each signer
    pub struct WaitingSigShares {
        /// Public Key Info
        pub(crate) key_info: KeyInfo<Ed25519>,
        /// Vector of Public Commitments from pre processing phase
        pub(crate) pcommits: PartyJar<PublicCommitments<Ed25519>>,
        /// Vector with the Signatures Shares from each signer
        pub(crate) sig_shares: PartyJar<SigShare<Ed25519>>,
        /// Message to sign
        pub(crate) msg: Vec<u8>,
        /// Sorted Parties
        pub(crate) sorted_parties: SortedParties,
    }
}

/// The state machine for the EdDSA protocol.
#[derive(StateMachineState)]
#[state_machine(
    recipient_id = "PartyId",
    input_message = "PartyMessage<EddsaSignStateMessage>",
    output_message = "EddsaSignStateMessage",
    final_result = "EddsaSignatureOutput",
    handle_message_fn = "Self::handle_message"
)]
pub enum EddsaSignState {
    /// We are waiting for the Public Commits
    #[state_machine(completed = "state.pcommits.is_full()", transition_fn = "Self::transition_waiting_public_commits")]
    WaitingPublicCommits(states::WaitingPublicCommits),
    /// We are waiting for the SigShares
    #[state_machine(completed = "state.sig_shares.is_full()", transition_fn = "Self::transition_waiting_sigshares")]
    WaitingSigShares(states::WaitingSigShares),
}

use EddsaSignState::*;
/// Implementations on EddsaSignState Machine
impl EddsaSignState {
    /// Construct a new EddsaSignState
    pub fn new(
        parties: Vec<PartyId>,
        msg: Vec<u8>,
        incomplete_key_share: ThresholdPrivateKeyShare<Ed25519>,
    ) -> Result<(Self, Vec<StateMachineMessage<Self>>), EddsaSignError> {
        // Compute input elements required for givre's functions
        let sorted_parties = SortedParties::new(parties);
        let parties_len = sorted_parties.len();
        let key_share = incomplete_key_share.into_inner();

        // Round 1 of FROST protocol
        let mut csprng = OsRng;
        let (secret_nonces, commits) = commit::<ciphersuite::Ed25519>(&mut csprng, &key_share);
        let pcommits = PartyJar::new(parties_len as usize);

        let next_state = states::WaitingPublicCommits {
            key_share,
            secret_nonces,
            pcommits,
            msg,
            sorted_parties: sorted_parties.clone(),
        };

        let messages = vec![RecipientMessage::new(
            Recipient::Multiple(sorted_parties.parties()),
            EddsaSignStateMessage::Message(Msg::Round1(commits)),
        )];
        Ok((WaitingPublicCommits(next_state), messages))
    }
    #[allow(clippy::indexing_slicing)]
    fn transition_waiting_public_commits(state: states::WaitingPublicCommits) -> StateMachineStateResult<Self> {
        // Steps 1 to 5 of Figure 3 of FROST protocol

        // Build signers_pcommits = [(SignerIndex, PublicCommits)]
        let signers_pcommits: Vec<_> = state
            .pcommits
            .elements()
            .map(|(party_id, pcommit)| {
                state
                    .sorted_parties
                    .index(party_id.clone())
                    .map(|party_index| (party_index, *pcommit))
                    .map_err(|e| anyhow!("Error converting PartyId to u16: {e}"))
            })
            .collect::<Result<_, _>>()
            .map_err(|e| anyhow!("Error converting PartyId to u16: {e}"))?;

        //Obtain Signature Shares
        let sigshare =
            sign::<ciphersuite::Ed25519>(&state.key_share, state.secret_nonces, &state.msg, &signers_pcommits)
                .map_err(|e| anyhow!("Signing Error: {e}"))?;

        // Build next state
        let parties_len = state.sorted_parties.len();
        let sig_shares = PartyJar::new(parties_len as usize);
        let key_info =
            state.key_share.into_inner().key_info.validate().map_err(|e| anyhow!("Error in Validate: {e}"))?;
        let sorted_parties = state.sorted_parties.clone();
        let next_state =
            states::WaitingSigShares { key_info, pcommits: state.pcommits, sig_shares, msg: state.msg, sorted_parties };

        //Build Message
        let message = RecipientMessage::new(
            Recipient::Multiple(state.sorted_parties.parties()),
            EddsaSignStateMessage::Message(Msg::Round2(sigshare)),
        );
        let messages = vec![message];
        Ok(StateMachineStateOutput::Messages(WaitingSigShares(next_state), messages))
    }

    #[allow(clippy::indexing_slicing)]
    fn transition_waiting_sigshares(state: states::WaitingSigShares) -> StateMachineStateResult<Self> {
        // Build signers: [(SignerIndex, PublicCommits, SigShare)],
        let signers: Vec<_> = state
            .pcommits
            .elements()
            .zip(state.sig_shares.elements())
            .map(|((party_id1, commit), (party_id2, sigshare))| {
                if party_id1 != party_id2 {
                    return Err(anyhow!("Error in Validate"));
                }
                state
                    .sorted_parties
                    .index(party_id1.clone())
                    .map(|party| (party, *commit, *sigshare))
                    .map_err(|e| anyhow!("Error in Validate: {e}"))
            })
            .collect::<Result<_, _>>()
            .map_err(|e| anyhow!("Signing Error: {e}"))?;

        // Step 7 of FROST protocol
        let sig = aggregate(&state.key_info, &signers, &state.msg)
            .map_err(|e| anyhow!("Error in the aggregation of SigShares: {e}"))?;

        Ok(StateMachineStateOutput::Final(EddsaSignatureOutput::Success { element: EddsaSignature { signature: sig } }))
    }

    fn handle_message(mut state: Self, message: PartyMessage<EddsaSignStateMessage>) -> StateMachineStateResult<Self> {
        let (party_id, message) = message.into_parts();
        match (message, &mut state) {
            (EddsaSignStateMessage::Message(Msg::Round1(message)), WaitingPublicCommits(inner)) => {
                inner
                    .pcommits
                    .add_element(party_id.clone(), message)
                    .context("adding public commits")
                    .map_err(|e| anyhow!("Error in the adding public commits: {e}"))?;
                state.advance_if_completed()
            }
            (EddsaSignStateMessage::Message(Msg::Round2(message)), WaitingSigShares(inner)) => {
                inner
                    .sig_shares
                    .add_element(party_id, message)
                    .context("adding signature shares")
                    .map_err(|e| anyhow!("Error in the adding signature shares: {e}"))?;
                state.advance_if_completed()
            }
            (message, _) => Ok(StateMachineStateOutput::OutOfOrder(state, PartyMessage::new(party_id, message))),
        }
    }
}

/// A message for the EdDSA-SIGNING protocol.
#[derive(Clone, Serialize, Deserialize)]
#[repr(u8)]
pub enum EddsaSignStateMessage {
    /// A message for the EdDSA-SIGNING state machine.
    Message(Msg<Ed25519>) = 0,
}

impl fmt::Debug for EddsaSignStateMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EddsaSignStateMessage::Message(msg) => {
                write!(f, "EddsaSignStateMessage::Message(")?;
                match msg {
                    Msg::Round1(_) => write!(f, "round: Round1")?,
                    Msg::Round2(_) => write!(f, "round: Round2")?,
                }
                write!(f, ")")
            }
        }
    }
}

impl PartialEq for EddsaSignStateMessage {
    fn eq(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (EddsaSignStateMessage::Message(Msg::Round1(_)), EddsaSignStateMessage::Message(Msg::Round1(_)))
                | (EddsaSignStateMessage::Message(Msg::Round2(_)), EddsaSignStateMessage::Message(Msg::Round2(_)))
        )
    }
}

/// An error during the EdDSA-SIGNING state creation.
#[derive(Debug, thiserror::Error)]
pub enum EddsaSignError {
    /// Unexpected error
    #[error("unexpected error: {0}")]
    Unexpected(anyhow::Error),
}

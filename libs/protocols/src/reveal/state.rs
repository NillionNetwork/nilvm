//! Reveal protocol implementation for multiple revealed secrets.

use anyhow::{anyhow, Context};
use basic_types::{jar::PartyJar, Batches, PartyMessage};
use math_lib::fields::Field;
use serde::{Deserialize, Serialize};
use shamir_sharing::{party::PartyId, secret_sharer::FieldSecretSharer};
use state_machine::{
    errors::StateMachineError,
    state::{Recipient, StateMachineMessage},
    StateMachineState, StateMachineStateExt, StateMachineStateOutput, StateMachineStateResult,
};
use state_machine_derive::StateMachineState;
use std::{collections::HashMap, sync::Arc};

/// Each of the reveal protocol state definitions.
pub mod states {
    use basic_types::jar::PartyJar;
    use math_lib::fields::Field;
    use std::sync::Arc;

    /// We are waiting for shares to reconstruct the secrets.
    pub struct WaitingShares<F: Field, S> {
        /// The number of secrets being reconstructed.
        pub secret_count: usize,

        /// The shares of each party, indexed by their party id.
        pub party_shares: PartyJar<Vec<F::Element>>,

        /// The secret sharer to be used during the reconstruction.
        pub secret_sharer: Arc<S>,
    }
}

/// The state of the reveal protocol.
#[derive(StateMachineState)]
#[state_machine(
    recipient_id = "PartyId",
    input_message = "PartyMessage<RevealStateMessage<F::EncodedElement>>",
    output_message = "RevealStateMessage<F::EncodedElement>",
    final_result = "Vec<F::Element>",
    handle_message_fn = "Self::handle_message"
)]
pub enum RevealState<F, S>
where
    F: Field,
    S: FieldSecretSharer<F>,
{
    /// We are waiting for shares of the underlying secret.
    #[state_machine(completed = "state.party_shares.is_full()", transition_fn = "Self::transition_waiting_shares")]
    WaitingShares(states::WaitingShares<F, S>),
}

use RevealState::*;

/// The mode we want to run REVEAL on.
///
/// The whitepaper runs 2 flavors of REVEAL:
/// * A REVEAL of our shares to all nodes in the network. At the end of this run, all nodes can reconstruct all
///   the secrets.
/// * A REVEAL that reveals the nth element to the nth node in the network. At the end of this run, node n can
///   only reconstruct secret n, given this is the only one it got shares from.
///
/// This enum wraps that behavior: [`RevealMode::All`] is the first one, [`RevealMode::Nth`] is the second one.
#[derive(Debug, Clone)]
pub enum RevealMode<T> {
    /// We are sending all of our shares to every other node.
    All {
        /// The shares of the secrets to be shared.
        ///
        /// Each of the shares will contribute to a single secret in the output of the protocol.
        shares: Vec<T>,
    },

    /// We are sending the nth element with the nth node.
    Nth {
        /// The shares of the secrets to be shared.
        ///
        /// Each batch will produce a single secret in the output of the protocol.
        share_batches: Batches<T>,
    },
}

impl<T> RevealMode<T> {
    /// Constructs a new global REVEAL mode.
    pub fn new_all(shares: Vec<T>) -> Self {
        Self::All { shares }
    }

    /// Constructs a new direct REVEAL mode.
    pub fn new_nth(share_batches: Batches<T>) -> Self {
        Self::Nth { share_batches }
    }
}

impl<F, S> RevealState<F, S>
where
    F: Field,
    S: FieldSecretSharer<F>,
{
    /// Constructs a new REVEAL state.
    ///
    /// # Arguments
    /// - `mode` - The mode this REVEAL is running on.
    /// - `shamir` - The Shamir configuration to be used.
    pub fn new(
        mode: RevealMode<F::Element>,
        secret_sharer: Arc<S>,
    ) -> Result<(Self, Vec<StateMachineMessage<Self>>), PartySecretMismatch> {
        let secret_count = match &mode {
            // One secret per share.
            RevealMode::All { shares } => shares.len(),
            // One secret per batch. Every node is giving us one share in the batch so the output is a single secret.
            RevealMode::Nth { share_batches } => share_batches.len(),
        };
        let messages = Self::build_messages(mode, &secret_sharer)?;
        let party_shares = PartyJar::new(secret_sharer.party_count());
        let state = states::WaitingShares { secret_count, secret_sharer, party_shares };
        Ok((WaitingShares(state), messages))
    }

    #[allow(clippy::indexing_slicing)]
    fn build_messages(
        mode: RevealMode<F::Element>,
        secret_sharer: &S,
    ) -> Result<Vec<StateMachineMessage<Self>>, PartySecretMismatch> {
        let parties = secret_sharer.parties();
        match mode {
            RevealMode::All { shares } => {
                // Every node gets all shares.
                let shares = F::encode(&shares);
                let messages =
                    vec![StateMachineMessage::<Self>::new(Recipient::Multiple(parties), RevealStateMessage(shares))];
                Ok(messages)
            }
            RevealMode::Nth { share_batches } => {
                // We will the nth share in each batch to the nth node. Split them accordingly.
                let mut share_chunks = vec![Vec::new(); parties.len()];
                for batch in share_batches {
                    if parties.len() != batch.len() {
                        return Err(PartySecretMismatch);
                    }
                    for (index, share) in batch.into_iter().enumerate() {
                        share_chunks[index].push(share);
                    }
                }
                // Now hand them off to each of them
                let mut messages = Vec::new();
                for (party_id, shares) in parties.into_iter().zip(share_chunks.into_iter()) {
                    let shares = F::encode(&shares);
                    let message =
                        StateMachineMessage::<Self>::new(Recipient::Single(party_id), RevealStateMessage(shares));
                    messages.push(message);
                }
                Ok(messages)
            }
        }
    }

    fn transition_waiting_shares(state: states::WaitingShares<F, S>) -> StateMachineStateResult<Self> {
        let secrets = Self::recover_secrets(state)?;
        Ok(StateMachineStateOutput::Final(secrets))
    }

    fn handle_message(
        mut state: Self,
        message: <Self as StateMachineState>::InputMessage,
    ) -> StateMachineStateResult<Self> {
        let (party_id, message) = message.into_parts();
        let shares = F::try_decode(&message.0).context("share decoding")?;
        state.waiting_shares_state_mut()?.party_shares.add_element(party_id, shares).context("adding shares")?;
        state.advance_if_completed()
    }

    #[allow(clippy::indexing_slicing)]
    fn recover_secrets(state: states::WaitingShares<F, S>) -> Result<Vec<F::Element>, StateMachineError> {
        let mut secret_shares = vec![HashMap::new(); state.secret_count];
        for (party_id, shares) in state.party_shares.into_elements() {
            if shares.len() != state.secret_count {
                return Err(anyhow!("expected {} shares, found {}", state.secret_count, shares.len()).into());
            }
            for (index, share) in shares.into_iter().enumerate() {
                secret_shares[index].insert(party_id.clone(), share);
            }
        }

        let mut secrets = Vec::new();
        for shares in secret_shares {
            let secret = state
                .secret_sharer
                .recover(shares.into_iter())
                .map_err(|e| anyhow!("failed to reconstruct shares: {e}"))?;
            secrets.push(secret);
        }
        Ok(secrets)
    }
}

/// A message for the REVEAL state machine, which sets the shares from a particular party.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RevealStateMessage<T>(pub Vec<T>);

/// There's a mismatch during a direct REVEAL mode between the number of parties and the number of shares.
#[derive(Debug, thiserror::Error)]
#[error("the number of secrets is different than the number of parties")]
pub struct PartySecretMismatch;

#[allow(clippy::arithmetic_side_effects, clippy::indexing_slicing)]
#[cfg(test)]
mod test {
    use super::*;
    use anyhow::Result;
    use math_lib::{
        fields::PrimeField,
        modular::{ModularNumber, U64SafePrime},
    };
    use shamir_sharing::secret_sharer::{SecretSharerProperties, ShamirSecretSharer};
    use state_machine::StateMachine;

    type Prime = U64SafePrime;
    type U64Field = PrimeField<Prime>;
    type Sharer = ShamirSecretSharer<Prime>;
    type State = RevealState<U64Field, Sharer>;

    fn make_secret_sharer() -> Arc<Sharer> {
        let parties = vec![PartyId::from(10), PartyId::from(20)];
        let secret_sharer = Sharer::new(parties[0].clone(), 1, parties).unwrap();
        Arc::new(secret_sharer)
    }

    #[test]
    fn message_building_global_mode() {
        let secret_sharer = make_secret_sharer();
        let mode = RevealMode::new_all(vec![ModularNumber::ONE, ModularNumber::two()]);
        let messages = State::build_messages(mode, &secret_sharer).unwrap();
        // There should be a single message for all nodes.
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].recipient(), &Recipient::Multiple(secret_sharer.parties()));
    }

    #[test]
    fn message_building_direct_mode() {
        let secret_sharer = make_secret_sharer();
        let share_batches = Batches::from(vec![
            vec![ModularNumber::ONE, ModularNumber::two()],
            vec![ModularNumber::from_u32(3), ModularNumber::from_u32(4)],
        ]);
        let mode = RevealMode::new_nth(share_batches);
        let messages = State::build_messages(mode, &secret_sharer).unwrap();
        // There should be a 2 messages, one per node.
        let parties = secret_sharer.parties();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].recipient(), &Recipient::Single(parties[0].clone()));
        assert_eq!(messages[1].recipient(), &Recipient::Single(parties[1].clone()));

        // The nth share goes to the nth node
        assert_eq!(messages[0].contents().0, U64Field::encode(&[ModularNumber::ONE, ModularNumber::from_u32(3)]));
        assert_eq!(messages[1].contents().0, U64Field::encode(&[ModularNumber::two(), ModularNumber::from_u32(4)]));
    }

    #[test]
    fn waiting_shares_state_checks() -> Result<()> {
        // Note: these shares are only used to generate the output message so they're unrelated to
        // the rest of the test case.
        let shares = vec![ModularNumber::from_u32(42), ModularNumber::from_u32(43)];
        let mut sm = StateMachine::new(State::new(RevealMode::new_all(shares), make_secret_sharer())?.0);
        assert!(!sm.is_state_completed());
        assert!(!sm.is_finished());

        sm.handle_message(PartyMessage::new(
            PartyId::from(10),
            RevealStateMessage(U64Field::encode(&[ModularNumber::from_u32(1345), ModularNumber::from_u32(44)])),
        ))?;

        // The second one consumes the state machine and returns the output.
        let secrets = sm
            .handle_message(PartyMessage::new(
                PartyId::from(20),
                RevealStateMessage(U64Field::encode(&[ModularNumber::from_u32(1353), ModularNumber::from_u32(46)])),
            ))?
            .into_final()?;
        assert_eq!(secrets, vec![ModularNumber::from_u32(1337), ModularNumber::from_u32(42)]);
        Ok(())
    }

    #[test]
    fn too_few_shares() {
        let party_shares = PartyJar::new_with_elements([
            (PartyId::from(10), vec![ModularNumber::from_u32(100)]),
            (PartyId::from(20), vec![]),
        ])
        .unwrap();

        let state = states::WaitingShares { secret_count: 1, party_shares, secret_sharer: make_secret_sharer() };
        assert!(State::recover_secrets(state).is_err());
    }

    #[test]
    fn too_many_shares() {
        let party_shares = PartyJar::new_with_elements(
            [
                (PartyId::from(10), vec![ModularNumber::from_u32(100)]),
                (PartyId::from(20), vec![ModularNumber::from_u32(150), ModularNumber::from_u32(42)]),
            ]
            .iter()
            .cloned(),
        )
        .unwrap();
        let state =
            states::WaitingShares::<U64Field, _> { secret_count: 1, party_shares, secret_sharer: make_secret_sharer() };

        assert!(RevealState::recover_secrets(state).is_err());
    }
}

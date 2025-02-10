//! Preprocessing for PRIVATE OUTPUT EQUALITY State Machine
//!
//! This is the Private Output Equality Protocol. It is used to obtain evaluate privately the equality of two values.

use crate::random::random_bitwise::BitwiseNumberShares;
use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use basic_types::PartyMessage;

use state_machine::{
    sm::StateMachineOutput, state::StateMachineMessage, StateMachine, StateMachineState, StateMachineStateExt,
    StateMachineStateOutput, StateMachineStateResult,
};
use state_machine_derive::StateMachineState;

use math_lib::modular::{AsBits, EncodedModularNumber, ModularNumber, SafePrime};

use shamir_sharing::{
    party::PartyId,
    secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer},
};

use crate::reveal::state::{PartySecretMismatch, RevealMode, RevealState, RevealStateMessage};

use crate::conditionals::poly_eval::online::{
    output::PolyEvalStateOutput,
    state::{PolyEvalState, PolyEvalStateMessage},
};

use super::output::{PrivateOutputEqualityShares, PrivateOutputEqualityStateOutput};
use crate::conditionals::equality::offline::output::PrepPrivateOutputEqualityShares;

/// The protocol states.
pub mod states {
    use math_lib::{
        fields::PrimeField,
        modular::{ModularNumber, SafePrime},
    };

    use crate::{
        conditionals::{
            equality::offline::output::PrepPrivateOutputEqualityShares,
            poly_eval::online::{output::PolyEvalShares, PolyEvalStateMachine},
        },
        reveal::RevealStateMachine,
    };
    use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer};
    use std::sync::Arc;

    /// We are waiting for REVEAL
    pub struct WaitingReveal<T>
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        /// The Secret Sharer
        pub(crate) secret_sharer: Arc<ShamirSecretSharer<T>>,

        /// The REVEAL state machine.
        pub(crate) reveal_state_machine: RevealStateMachine<PrimeField<T>, ShamirSecretSharer<T>>,

        /// The output of REVEAL.
        pub(crate) reveal_output: Vec<ModularNumber<T>>,

        /// The preprocessing elements for PREP PRIVATE OUTPUT EQUALITY
        pub(crate) prep_private_equality: Vec<PrepPrivateOutputEqualityShares<T>>,
    }

    /// We are waiting for PREP-POLY-EVAL
    pub struct WaitingPolyEval<T>
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        /// The POLY-EVAL state machine.
        pub(crate) poly_eval_state_machine: PolyEvalStateMachine<T>,

        /// The number of elements to be produced.
        pub(crate) poly_eval_outputs: Vec<PolyEvalShares<T>>,
    }
}

/// The PRIVATE OUTPUT EQUALITY protocol state.
#[derive(StateMachineState)]
#[state_machine(
    recipient_id = "PartyId",
    input_message = "PartyMessage<PrivateOutputEqualityStateMessage>",
    output_message = "PrivateOutputEqualityStateMessage",
    final_result = "PrivateOutputEqualityStateOutput<PrivateOutputEqualityShares<T>>",
    handle_message_fn = "Self::handle_message"
)]
pub enum PrivateOutputEqualityState<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// We are waiting for REVEAL.
    #[state_machine(submachine = "state.reveal_state_machine", transition_fn = "Self::transition_waiting_reveal")]
    WaitingReveal(states::WaitingReveal<T>),

    /// We are waiting for POLY-EVAL
    #[state_machine(submachine = "state.poly_eval_state_machine", transition_fn = "Self::transition_waiting_poly_eval")]
    WaitingPolyEval(states::WaitingPolyEval<T>),
}

use PrivateOutputEqualityState::*;

impl<T> PrivateOutputEqualityState<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Construct a new PRIVATE OUTPUT EQUALITY state.
    pub fn new(
        x: Vec<ModularNumber<T>>,
        y: Vec<ModularNumber<T>>,
        prep_private_equality: Vec<PrepPrivateOutputEqualityShares<T>>,
        secret_sharer: Arc<ShamirSecretSharer<T>>,
    ) -> Result<(Self, Vec<StateMachineMessage<Self>>), PrivateOutputEqualityError> {
        // Computing x - y the difference between the inputs.
        let z: Vec<ModularNumber<T>> = x.into_iter().zip(y.iter()).map(|(x, y)| x - y).collect();

        // Computing the masking of z with bitwise random shares.
        let z_masked = z
            .into_iter()
            .zip(prep_private_equality.iter())
            .map(|(z, prep)| z + &prep.bitwise_number_shares.merge_bits())
            .collect::<Vec<ModularNumber<T>>>();

        // Call the REVEAL Protocol on z_masked
        let (reveal_state, messages) = RevealState::new(RevealMode::new_all(z_masked), secret_sharer.clone())?;
        let reveal_state_machine = StateMachine::new(reveal_state);
        let next_state = states::WaitingReveal {
            secret_sharer,
            reveal_state_machine,
            prep_private_equality,
            reveal_output: Vec::new(),
        };
        let messages =
            messages.into_iter().map(|message| message.wrap(&PrivateOutputEqualityStateMessage::Reveal)).collect();
        Ok((WaitingReveal(next_state), messages))
    }

    fn hamming_distance(x: &BitwiseNumberShares<T>, y: &ModularNumber<T>) -> ModularNumber<T> {
        let mut distance = ModularNumber::ZERO;
        let y = y.into_value();
        for (i, share) in x.shares().iter().enumerate() {
            distance = distance + share.xor_mask(y.bit(i)).value();
        }
        distance
    }

    fn transition_waiting_reveal(state: states::WaitingReveal<T>) -> StateMachineStateResult<Self> {
        let distances: Vec<ModularNumber<T>> = state
            .reveal_output
            .iter()
            .zip(state.prep_private_equality.iter())
            .map(|(masked_reveal, prep)| {
                let distance = Self::hamming_distance(&prep.bitwise_number_shares, masked_reveal);
                distance + &ModularNumber::ONE
            })
            .collect();

        let (polynomials, prep_poly_evals) =
            state.prep_private_equality.into_iter().map(|prep| (prep.lagrange_polynomial, prep.prep_poly_eval)).unzip();
        let (poly_eval_state, messages) =
            PolyEvalState::new(distances, polynomials, prep_poly_evals, state.secret_sharer.clone())
                .map_err(|e| anyhow!("PrepPolyEvalState Error: {e}"))?;

        let next_state = states::WaitingPolyEval {
            poly_eval_state_machine: StateMachine::new(poly_eval_state),
            poly_eval_outputs: Vec::new(),
        };

        let messages =
            messages.into_iter().map(|message| message.wrap(&PrivateOutputEqualityStateMessage::PolyEval)).collect();
        Ok(StateMachineStateOutput::Messages(WaitingPolyEval(next_state), messages))
    }

    fn transition_waiting_poly_eval(state: states::WaitingPolyEval<T>) -> StateMachineStateResult<Self> {
        let mut outputs = Vec::<PrivateOutputEqualityShares<T>>::with_capacity(state.poly_eval_outputs.len());
        for output in state.poly_eval_outputs.into_iter() {
            let prep_poly_eval = output.poly_x;
            let output = PrivateOutputEqualityShares { equality_output: prep_poly_eval };
            outputs.push(output);
        }
        let output = PrivateOutputEqualityStateOutput { outputs };
        Ok(StateMachineStateOutput::Final(output))
    }

    fn handle_message(
        mut state: Self,
        message: PartyMessage<PrivateOutputEqualityStateMessage>,
    ) -> StateMachineStateResult<Self> {
        use PrivateOutputEqualityStateMessage::*;
        let (party_id, message) = message.into_parts();
        match (message, &mut state) {
            (Reveal(message), WaitingReveal(inner)) => {
                match inner.reveal_state_machine.handle_message(PartyMessage::new(party_id, message))? {
                    StateMachineOutput::Final(secrets) => {
                        inner.reveal_output = secrets;
                        state.try_next()
                    }
                    output => state.wrap_message(output, PrivateOutputEqualityStateMessage::Reveal),
                }
            }
            (PolyEval(message), WaitingPolyEval(inner)) => {
                match inner.poly_eval_state_machine.handle_message(PartyMessage::new(party_id, message))? {
                    StateMachineOutput::Final(PolyEvalStateOutput::Success { outputs }) => {
                        inner.poly_eval_outputs = outputs;
                        state.try_next()
                    }
                    output => state.wrap_message(output, PrivateOutputEqualityStateMessage::PolyEval),
                }
            }
            (message, _) => Ok(StateMachineStateOutput::OutOfOrder(state, PartyMessage::new(party_id, message))),
        }
    }
}

/// A message for the Private EQUALITY protocol.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[repr(u8)]
pub enum PrivateOutputEqualityStateMessage {
    /// A message for the underlying Reveal state machine.
    Reveal(RevealStateMessage<EncodedModularNumber>) = 0,

    /// A message for the PRIVATE OUTPUT EQUALITY sub state machine.
    PolyEval(PolyEvalStateMessage) = 1,
}

/// An error during the creation of the Private EQUALITY state.
#[derive(Debug, thiserror::Error)]
pub enum PrivateOutputEqualityError {
    /// An error during Reveal
    #[error("Reveal: {0}")]
    Reveal(#[from] PartySecretMismatch),
}

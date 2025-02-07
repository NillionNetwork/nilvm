//! Preprocessing for PREP PRIVATE OUTPUT EQUALITY State Machine
//!
//! This is the Evaluate Polynomial functionality. It is used to obtain evaluate privately a polynomial.

use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use basic_types::PartyMessage;

use state_machine::{
    sm::StateMachineOutput, state::StateMachineMessage, StateMachine, StateMachineState, StateMachineStateExt,
    StateMachineStateOutput, StateMachineStateResult,
};
use state_machine_derive::StateMachineState;

use math_lib::{
    decoders::lagrange_polynomial,
    fields::PrimeField,
    modular::{ModularNumber, SafePrime},
    polynomial::{point::Point, point_sequence::PointSequence},
};

use shamir_sharing::{
    party::PartyId,
    secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer},
};

use crate::random::random_bitwise::{
    RanBitwiseCreateError, RanBitwiseMode, RanBitwiseState, RanBitwiseStateMessage, RanBitwiseStateOutput,
};

use crate::conditionals::poly_eval::offline::{
    output::PrepPolyEvalStateOutput,
    state::{PrepPolyEvalState, PrepPolyEvalStateMessage},
};

use crate::conditionals::equality::offline::output::{
    PrepPrivateOutputEqualityShares, PrepPrivateOutputEqualityStateOutput,
};

/// The protocol states.
pub mod states {
    use crate::random::random_bitwise::{BitwiseNumberShares, RanBitwiseStateMachine};
    use math_lib::{fields::PrimeField, modular::SafePrime, polynomial::Polynomial};

    use crate::conditionals::poly_eval::offline::{output::PrepPolyEvalShares, PrepPolyEvalStateMachine};
    use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer};
    use std::sync::Arc;

    /// We are waiting for RANDOM-BITWISE.
    pub struct WaitingRanBitwise<T>
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        /// The RANDOM-BITWISE state machine.
        pub(crate) random_bitwise_state_machine: RanBitwiseStateMachine<T>,

        /// The secret sharer to be used.
        pub(crate) secret_sharer: Arc<ShamirSecretSharer<T>>,

        /// The number of elements to be produced.
        pub(crate) element_count: usize,

        /// The degree of the polynomial evaluation.
        pub(crate) poly_eval_degree: u64,

        /// The bitwise numbers produced by RANDOM-BITWISE.
        pub(crate) bitwise_numbers: Vec<BitwiseNumberShares<T>>,
    }

    /// We are waiting for PREP-POLY-EVAL
    pub struct WaitingPrepPolyEval<T>
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        /// The RANDOM-BITWISE state machine.
        pub(crate) prep_poly_eval_state_machine: PrepPolyEvalStateMachine<T>,

        /// The number of elements to be produced.
        pub(crate) element_count: usize,

        /// The Lagrange Polynomial
        pub(crate) lagrange_polynomial: Polynomial<PrimeField<T>>,

        /// The bitwise numbers produced by RANDOM-BITWISE.
        pub(crate) bitwise_numbers: Vec<BitwiseNumberShares<T>>,

        /// The prep_poly_eval preprocessing data.
        pub(crate) prep_poly_eval_outputs: Vec<PrepPolyEvalShares<T>>,
    }
}

/// The Prep PRIVATE OUTPUT EQUALITY protocol state.
#[derive(StateMachineState)]
#[state_machine(
    recipient_id = "PartyId",
    input_message = "PartyMessage<PrepPrivateOutputEqualityStateMessage>",
    output_message = "PrepPrivateOutputEqualityStateMessage",
    final_result = "PrepPrivateOutputEqualityStateOutput<PrepPrivateOutputEqualityShares<T>>",
    handle_message_fn = "Self::handle_message"
)]
pub enum PrepPrivateOutputEqualityState<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// We are waiting for RANDOM-BITWISE.
    #[state_machine(
        submachine = "state.random_bitwise_state_machine",
        transition_fn = "Self::transition_waiting_random_bitwise"
    )]
    WaitingRanBitwise(states::WaitingRanBitwise<T>),

    /// We are waiting for PREP-POLY-EVAL
    #[state_machine(
        submachine = "state.prep_poly_eval_state_machine",
        transition_fn = "Self::transition_waiting_prep_poly_eval"
    )]
    WaitingPrepPolyEval(states::WaitingPrepPolyEval<T>),
}

use PrepPrivateOutputEqualityState::*;

impl<T> PrepPrivateOutputEqualityState<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Construct a new Prep PRIVATE OUTPUT EQUALITY state.
    pub fn new(
        element_count: usize,
        poly_eval_degree: u64,
        secret_sharer: Arc<ShamirSecretSharer<T>>,
    ) -> Result<(Self, Vec<StateMachineMessage<Self>>), PrepPrivateOutputEqualityError> {
        // Call the RAN Bitwise Protocol
        let (bitwise_state, messages) =
            RanBitwiseState::new(RanBitwiseMode::Full, element_count, secret_sharer.clone())?;
        let state = states::WaitingRanBitwise {
            secret_sharer,
            element_count,
            poly_eval_degree,
            random_bitwise_state_machine: StateMachine::new(bitwise_state),
            bitwise_numbers: Vec::new(),
        };
        let messages = messages
            .into_iter()
            .map(|message| message.wrap(&PrepPrivateOutputEqualityStateMessage::RanBitwise))
            .collect();
        Ok((WaitingRanBitwise(state), messages))
    }

    fn transition_waiting_random_bitwise(state: states::WaitingRanBitwise<T>) -> StateMachineStateResult<Self> {
        let mut point_sequence = PointSequence::<PrimeField<T>>::default();
        for x in 0..state.poly_eval_degree + 1 {
            let num = if x == 1 { ModularNumber::ONE } else { ModularNumber::ZERO };
            point_sequence.push(Point::new(x.into(), num));
        }

        let lagrange_poly =
            lagrange_polynomial(&point_sequence).map_err(|e| anyhow!("Lagrange Interpolation Error: {e}"))?;

        let (prep_poly_eval_state, messages) =
            PrepPolyEvalState::new(state.element_count, state.poly_eval_degree, state.secret_sharer)
                .map_err(|e| anyhow!("PrepPolyEvalState Error: {e}"))?;

        let next_state = states::WaitingPrepPolyEval {
            element_count: state.element_count,
            lagrange_polynomial: lagrange_poly,
            prep_poly_eval_state_machine: StateMachine::new(prep_poly_eval_state),
            bitwise_numbers: state.bitwise_numbers,
            prep_poly_eval_outputs: Vec::new(),
        };

        let messages = messages
            .into_iter()
            .map(|message| message.wrap(&PrepPrivateOutputEqualityStateMessage::PrepPolyEval))
            .collect();
        Ok(StateMachineStateOutput::Messages(WaitingPrepPolyEval(next_state), messages))
    }

    fn transition_waiting_prep_poly_eval(state: states::WaitingPrepPolyEval<T>) -> StateMachineStateResult<Self> {
        let mut outputs = Vec::<PrepPrivateOutputEqualityShares<T>>::with_capacity(state.element_count);
        for (bitwise_number_shares, prep_poly_eval) in
            state.bitwise_numbers.into_iter().zip(state.prep_poly_eval_outputs.into_iter())
        {
            let output = PrepPrivateOutputEqualityShares {
                bitwise_number_shares,
                lagrange_polynomial: state.lagrange_polynomial.clone(),
                prep_poly_eval,
            };
            outputs.push(output);
        }
        let output = PrepPrivateOutputEqualityStateOutput::Success { shares: outputs };
        Ok(StateMachineStateOutput::Final(output))
    }
    fn handle_message(
        mut state: Self,
        message: PartyMessage<PrepPrivateOutputEqualityStateMessage>,
    ) -> StateMachineStateResult<Self> {
        use PrepPrivateOutputEqualityStateMessage::*;
        let (party_id, message) = message.into_parts();
        match (message, &mut state) {
            (RanBitwise(message), WaitingRanBitwise(inner)) => {
                match inner.random_bitwise_state_machine.handle_message(PartyMessage::new(party_id, message))? {
                    StateMachineOutput::Final(RanBitwiseStateOutput::Success { shares }) => {
                        inner.bitwise_numbers = shares;
                        state.try_next()
                    }
                    StateMachineOutput::Final(_) => {
                        Ok(StateMachineStateOutput::Final(PrepPrivateOutputEqualityStateOutput::RanBitwiseAbort))
                    }
                    output => state.wrap_message(output, PrepPrivateOutputEqualityStateMessage::RanBitwise),
                }
            }
            (PrepPolyEval(message), WaitingPrepPolyEval(inner)) => {
                match inner.prep_poly_eval_state_machine.handle_message(PartyMessage::new(party_id, message))? {
                    StateMachineOutput::Final(PrepPolyEvalStateOutput::Success { outputs }) => {
                        inner.prep_poly_eval_outputs = outputs;
                        state.try_next()
                    }
                    StateMachineOutput::Final(_) => {
                        Ok(StateMachineStateOutput::Final(PrepPrivateOutputEqualityStateOutput::PrepPolyEvalAbort))
                    }
                    output => state.wrap_message(output, PrepPrivateOutputEqualityStateMessage::PrepPolyEval),
                }
            }
            (message, _) => Ok(StateMachineStateOutput::OutOfOrder(state, PartyMessage::new(party_id, message))),
        }
    }
}

/// A message for the Prep PRIVATE OUTPUT EQUALITY protocol.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[repr(u8)]
pub enum PrepPrivateOutputEqualityStateMessage {
    /// A message for the RAN BITWISE sub state machine.
    RanBitwise(RanBitwiseStateMessage) = 0,

    /// A message for the RAN BITWISE sub state machine.
    PrepPolyEval(PrepPolyEvalStateMessage) = 1,
}

/// An error during the creation of the Prep PRIVATE OUTPUT EQUALITY state.
#[derive(Debug, thiserror::Error)]
pub enum PrepPrivateOutputEqualityError {
    /// An error during the creation of the RAN BITWISE state.
    #[error("RanBitwiseCreateError: {0}")]
    RanBitwise(#[from] RanBitwiseCreateError),
}

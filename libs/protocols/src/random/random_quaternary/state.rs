//! The RAN-QUATERNARY protocol.

use super::{QuatShare, QuaternaryShares};
use crate::{
    multiplication::multiplication_shares::{
        state::{MultState, MultStateMessage},
        OperandShares,
    },
    random::random_bitwise::{
        BitwiseNumberShares, RanBitwiseCreateError, RanBitwiseMode, RanBitwiseState, RanBitwiseStateMessage,
        RanBitwiseStateOutput,
    },
};
use anyhow::{anyhow, Error};
use basic_types::{PartyId, PartyMessage};
use math_lib::modular::{Modular, ModularNumber, SafePrime};
use serde::{Deserialize, Serialize};
use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer};
use state_machine::{
    sm::StateMachineOutput, state::StateMachineMessage, StateMachine, StateMachineState, StateMachineStateExt,
    StateMachineStateOutput, StateMachineStateResult,
};
use state_machine_derive::StateMachineState;
use std::sync::Arc;

/// The states of this protocol.
pub mod states {
    use crate::{
        multiplication::multiplication_shares::MultStateMachine,
        random::random_bitwise::{BitwiseNumberShares, RanBitwiseStateMachine},
    };
    use math_lib::modular::{ModularNumber, SafePrime};
    use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer};
    use std::sync::Arc;

    /// We are waiting for RANDOM-BITWISE.
    pub struct WaitingRanBitwise<T>
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        /// The RAN-BIT state machine.
        pub(crate) random_bitwise_state_machine: RanBitwiseStateMachine<T>,

        /// The secret sharer to be used.
        pub(crate) secret_sharer: Arc<ShamirSecretSharer<T>>,

        /// The bit shares produced by RANDOM-BITWISE.
        pub(crate) bitwise_numbers: Vec<BitwiseNumberShares<T>>,
    }

    /// We are waiting for MULT.
    pub struct WaitingMult<T>
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        /// The MULT state machine.
        pub(crate) mult_state_machine: MultStateMachine<T>,

        /// The bit shares produced by RANDOM-BITWISE.
        pub(crate) bitwise_numbers: Vec<BitwiseNumberShares<T>>,

        /// The product output as produced by MULT.
        pub(crate) cross_terms: Vec<ModularNumber<T>>,
    }
}

/// The output of the RANDOM-BITWISE protocol.
pub enum RanQuaternaryStateOutput<T: Modular> {
    /// The protocol was successful.
    Success {
        /// The output shares.
        shares: Vec<QuaternaryShares<T>>,
    },

    /// RANDOM-BITWISE aborted.
    RanBitwiseAbort,
}

/// The RAN-QUATERNARY state machine.
#[derive(StateMachineState)]
#[state_machine(
    recipient_id = "PartyId",
    input_message = "PartyMessage<RanQuaternaryStateMessage>",
    output_message = "RanQuaternaryStateMessage",
    final_result = "RanQuaternaryStateOutput<T>",
    handle_message_fn = "Self::handle_message"
)]
pub enum RanQuaternaryState<T>
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

    /// We are waiting for MULT.
    #[state_machine(submachine = "state.mult_state_machine", transition_fn = "Self::transition_waiting_mult")]
    WaitingMult(states::WaitingMult<T>),
}

use RanQuaternaryState::*;

impl<T> RanQuaternaryState<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Construct a new RANDOM-BITWISE state.
    pub fn new(
        mode: RanBitwiseMode,
        elements: usize,
        secret_sharer: Arc<ShamirSecretSharer<T>>,
    ) -> Result<(Self, Vec<StateMachineMessage<Self>>), RanQuaternaryError> {
        let (random_bitwise_state, messages) = RanBitwiseState::new(mode, elements, secret_sharer.clone())?;
        let messages =
            messages.into_iter().map(|message| message.wrap(&RanQuaternaryStateMessage::RanBitwise)).collect();
        let state = states::WaitingRanBitwise {
            random_bitwise_state_machine: StateMachine::new(random_bitwise_state),
            secret_sharer,
            bitwise_numbers: Vec::new(),
        };
        Ok((WaitingRanBitwise(state), messages))
    }

    fn transition_waiting_random_bitwise(state: states::WaitingRanBitwise<T>) -> StateMachineStateResult<Self> {
        let operands = Self::compute_cross_operands(&state.bitwise_numbers)?;
        let (mult_state, messages) = MultState::new(operands, state.secret_sharer.clone())
            .map_err(|e| anyhow!("failed to create MULT state: {e}"))?;
        let messages = messages.into_iter().map(|message| message.wrap(&RanQuaternaryStateMessage::Mult)).collect();
        let next_state = states::WaitingMult {
            mult_state_machine: StateMachine::new(mult_state),
            bitwise_numbers: state.bitwise_numbers,
            cross_terms: Vec::new(),
        };
        Ok(StateMachineStateOutput::Messages(WaitingMult(next_state), messages))
    }

    fn transition_waiting_mult(state: states::WaitingMult<T>) -> StateMachineStateResult<Self> {
        let shares = Self::combine_cross_terms(state.bitwise_numbers, state.cross_terms)
            .map_err(|e| anyhow!("could not combine cross terms {e}"))?;
        Ok(StateMachineStateOutput::Final(RanQuaternaryStateOutput::Success { shares }))
    }

    fn handle_message(
        mut state: Self,
        message: PartyMessage<RanQuaternaryStateMessage>,
    ) -> StateMachineStateResult<Self> {
        use RanQuaternaryStateMessage::*;
        let (party_id, message) = message.into_parts();
        match (message, &mut state) {
            (RanBitwise(message), WaitingRanBitwise(inner)) => {
                match inner.random_bitwise_state_machine.handle_message(PartyMessage::new(party_id, message))? {
                    StateMachineOutput::Final(RanBitwiseStateOutput::Success { shares }) => {
                        inner.bitwise_numbers = shares;
                        state.try_next()
                    }
                    StateMachineOutput::Final(RanBitwiseStateOutput::RanBitAbort) => {
                        Ok(StateMachineStateOutput::Final(RanQuaternaryStateOutput::RanBitwiseAbort))
                    }
                    StateMachineOutput::Final(RanBitwiseStateOutput::Abort) => {
                        Ok(StateMachineStateOutput::Final(RanQuaternaryStateOutput::RanBitwiseAbort))
                    }
                    output => state.wrap_message(output, RanQuaternaryStateMessage::RanBitwise),
                }
            }
            (Mult(message), WaitingMult(inner)) => {
                match inner.mult_state_machine.handle_message(PartyMessage::new(party_id, message))? {
                    StateMachineOutput::Final(values) => {
                        inner.cross_terms = values;
                        state.try_next()
                    }
                    output => state.wrap_message(output, RanQuaternaryStateMessage::Mult),
                }
            }
            (message, _) => Ok(StateMachineStateOutput::OutOfOrder(state, PartyMessage::new(party_id, message))),
        }
    }

    #[allow(clippy::indexing_slicing)]
    fn compute_cross_operands(bitwise_numbers: &[BitwiseNumberShares<T>]) -> Result<Vec<OperandShares<T>>, Error> {
        let mut operands = Vec::new();
        for bitwise_number in bitwise_numbers.iter() {
            // SAFETY: We filtered so that we only have chunks of size 2.
            let iter = bitwise_number
                .shares()
                .chunks(2)
                .filter(|chunk| chunk.len() == 2)
                .map(|chunk| (chunk[0].clone(), chunk[1].clone()));
            for (ith, next) in iter {
                let operand = OperandShares::single(*ith.value(), *next.value());
                operands.push(operand);
            }
        }
        Ok(operands)
    }

    #[allow(clippy::indexing_slicing)]
    fn combine_cross_terms(
        bitwise_numbers: Vec<BitwiseNumberShares<T>>,
        cross_terms: Vec<ModularNumber<T>>,
    ) -> Result<Vec<QuaternaryShares<T>>, RanQuaternaryError> {
        let len = bitwise_numbers.first().ok_or(RanQuaternaryError::NoElements)?.shares().len();
        let half = len.checked_div(2).ok_or(RanQuaternaryError::IntegerOverflow)?;
        let cross: Vec<_> = cross_terms.chunks(half).map(|chunk| chunk.to_vec()).collect();
        let mut quats = Vec::new();
        for (bitwise, cross) in bitwise_numbers.into_iter().zip(cross.into_iter()) {
            let mut quat = Vec::new();
            for (chunk, cross) in bitwise.shares().chunks(2).zip(cross) {
                let q = match chunk.len() {
                    2 => QuatShare::new(
                        ModularNumber::from(chunk[0].clone()),
                        ModularNumber::from(chunk[1].clone()),
                        cross,
                    ),
                    1 => QuatShare::single(ModularNumber::from(chunk[0].clone())),
                    _ => return Err(RanQuaternaryError::NoElements),
                };
                quat.push(q);
            }
            quats.push(QuaternaryShares::from(quat));
        }
        Ok(quats)
    }
}

/// A message for the RAN-QUATERNARY protocol.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[repr(u8)]
pub enum RanQuaternaryStateMessage {
    /// A message for the RANDOM-BITWISE state machine.
    RanBitwise(RanBitwiseStateMessage) = 0,

    /// A message for the MULT state machine.
    Mult(MultStateMessage) = 1,
}

/// An error during the RANDOM-BITWISE creation.
#[derive(Debug, thiserror::Error)]
pub enum RanQuaternaryError {
    /// An error during the RAN-BIT creation.
    #[error("ran bit: {0}")]
    RanBitwise(#[from] RanBitwiseCreateError),

    /// No elements found.
    #[error("no elements found")]
    NoElements,

    /// An integer overflow error.
    #[error("integer overflow")]
    IntegerOverflow,
}

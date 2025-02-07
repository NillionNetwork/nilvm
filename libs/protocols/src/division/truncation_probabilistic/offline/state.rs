//! PREP-TRUNCPR protocol.
//!
//! This is the preprocessing protocol that speeds up the online phase in the TRUNCPR protocol.

use super::{PrepTruncPrShares, PrepTruncPrStateOutput};
use crate::random::random_bitwise::{
    RanBitwiseCreateError, RanBitwiseMode, RanBitwiseState, RanBitwiseStateMessage, RanBitwiseStateOutput,
};
use basic_types::{batches::NotEnoughElements, PartyId, PartyMessage};
use math_lib::modular::{AsBits, SafePrime};
use serde::{Deserialize, Serialize};
use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer};
use state_machine::{
    sm::StateMachineOutput, state::StateMachineMessage, StateMachine, StateMachineState, StateMachineStateExt,
    StateMachineStateOutput, StateMachineStateResult,
};
use state_machine_derive::StateMachineState;
use std::sync::Arc;

/// The protocol states.
pub mod states {
    use crate::random::random_bitwise::{BitwiseNumberShares, RanBitwiseStateMachine};
    use math_lib::modular::SafePrime;
    use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer};

    /// We are waiting for RAN-BIT.
    pub struct WaitingRanBitwise<T>
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        /// The RAN-BIT state machine.
        pub(crate) ran_bit_state_machine: RanBitwiseStateMachine<T>,

        /// The bitwise shares produced by RANDOM-BITWISE.
        pub(crate) bitwise_shares: Vec<BitwiseNumberShares<T>>,
    }
}

/// The PREP-TRUNCPR protocol state.
#[derive(StateMachineState)]
#[state_machine(
    recipient_id = "PartyId",
    input_message = "PartyMessage<PrepTruncPrStateMessage>",
    output_message = "PrepTruncPrStateMessage",
    final_result = "PrepTruncPrStateOutput<PrepTruncPrShares<T>>",
    handle_message_fn = "Self::handle_message"
)]
pub enum PrepTruncPrState<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// We are waiting for RAN-BIT.
    #[state_machine(submachine = "state.ran_bit_state_machine", transition_fn = "Self::transition_waiting_ran_bit")]
    WaitingRanBitwise(states::WaitingRanBitwise<T>),
}

use PrepTruncPrState::*;

impl<T> PrepTruncPrState<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Construct a new PREP-TRUNCPR state.
    pub fn new(
        element_count: usize,
        kappa: usize,
        k: usize,
        secret_sharer: Arc<ShamirSecretSharer<T>>,
    ) -> Result<(Self, Vec<StateMachineMessage<Self>>), PrepTruncPrCreateError> {
        // Check sizes
        let prime_length = T::MODULO.bits();
        if k.checked_add(kappa).ok_or(PrepTruncPrCreateError::IntegerOverflow)? >= prime_length {
            return Err(PrepTruncPrCreateError::StatisticalAndMaxSecretLargeForFieldSize);
        }

        let batch_size_bits = k.checked_add(kappa).ok_or(PrepTruncPrCreateError::IntegerOverflow)?;

        let (ran_bit_state, messages) =
            RanBitwiseState::new(RanBitwiseMode::new_sized(batch_size_bits), element_count, secret_sharer.clone())?;
        let state = states::WaitingRanBitwise {
            ran_bit_state_machine: StateMachine::new(ran_bit_state),
            bitwise_shares: Vec::new(),
        };
        let messages = messages.into_iter().map(|message| message.wrap(&PrepTruncPrStateMessage::RanBitwise)).collect();
        Ok((WaitingRanBitwise(state), messages))
    }

    fn transition_waiting_ran_bit(state: states::WaitingRanBitwise<T>) -> StateMachineStateResult<Self> {
        let shares = state.bitwise_shares.into_iter().map(|ran_bits_r| PrepTruncPrShares { ran_bits_r }).collect();

        let output = PrepTruncPrStateOutput::Success { shares };
        Ok(StateMachineStateOutput::Final(output))
    }

    fn handle_message(
        mut state: Self,
        message: PartyMessage<PrepTruncPrStateMessage>,
    ) -> StateMachineStateResult<Self> {
        use PrepTruncPrStateMessage::*;
        let (party_id, message) = message.into_parts();
        match (message, &mut state) {
            (RanBitwise(message), WaitingRanBitwise(inner)) => {
                match inner.ran_bit_state_machine.handle_message(PartyMessage::new(party_id, message))? {
                    StateMachineOutput::Final(RanBitwiseStateOutput::Success { shares }) => {
                        inner.bitwise_shares = shares;
                        state.try_next()
                    }
                    StateMachineOutput::Final(_) => {
                        Ok(StateMachineStateOutput::Final(PrepTruncPrStateOutput::RanAbort))
                    }
                    output => state.wrap_message(output, PrepTruncPrStateMessage::RanBitwise),
                }
            }
        }
    }
}

/// A message for the PREP-TRUNCPR protocol.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[repr(u8)]
pub enum PrepTruncPrStateMessage {
    /// A message for the RAN-BIT state machine.
    RanBitwise(RanBitwiseStateMessage) = 0,
}

/// An error during the creation of the PREP-TRUNCPR state.
#[derive(Debug, thiserror::Error)]
pub enum PrepTruncPrCreateError {
    /// An integer overflow.
    #[error("integer overflow")]
    IntegerOverflow,

    /// An error during the RAN-BIT creation.
    #[error("RAN-BIT: {0}")]
    RanBitwise(#[from] RanBitwiseCreateError),

    /// An error when statistical parameter kappa
    /// and k are larger than the field size
    #[error("Statistical parameter kappa and k are too large for current field size")]
    StatisticalAndMaxSecretLargeForFieldSize,

    /// An error during the batch process.
    #[error("Batch: {0}")]
    CreateBatch(#[from] NotEnoughElements),
}

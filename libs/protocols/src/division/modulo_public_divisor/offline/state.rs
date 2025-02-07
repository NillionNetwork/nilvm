//! PREP-MODULO protocol.
//!
//! This is the preprocessing protocol that speeds up the online phase in the MODULO protocol.

use super::{PrepModuloShares, PrepModuloStateOutput};
use anyhow::anyhow;
use basic_types::{batches::NotEnoughElements, Batches, PartyId, PartyMessage};
use math_lib::modular::{AsBits, SafePrime};

use crate::{
    conditionals::less_than::offline::{
        state::{PrepCompareCreateError, PrepCompareState, PrepCompareStateMessage},
        PrepCompareStateOutput,
    },
    random::random_bitwise::{
        RanBitwiseCreateError, RanBitwiseMode, RanBitwiseState, RanBitwiseStateMessage, RanBitwiseStateOutput,
    },
};
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
    use crate::{
        conditionals::less_than::{offline::output::PrepCompareShares, PrepCompareStateMachine},
        random::random_bitwise::{BitwiseNumberShares, RanBitwiseStateMachine},
    };
    use basic_types::Batches;

    // use basic_types::Batches;
    use math_lib::modular::SafePrime;
    use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer};
    use std::sync::Arc;

    /// We are waiting for RAN-BIT.
    pub struct WaitingRanBitwise<T>
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        /// The RAN-BIT state machine.
        pub(crate) ran_bit_state_machine: RanBitwiseStateMachine<T>,

        /// The secret sharer to be used.
        pub(crate) secret_sharer: Arc<ShamirSecretSharer<T>>,

        /// The number of PREP-COMPARE shares per element.
        pub(crate) batch_size_prep_compare: usize,

        /// The total number of PREP-COMPARE shares.
        pub(crate) total_prep_compare: usize,

        /// The bitwise shares produced by RANDOM-BITWISE.
        pub(crate) bitwise_shares: Vec<BitwiseNumberShares<T>>,
    }

    /// We are waiting for the first PREP-COMPARISON comparison
    pub struct WaitingCompare<T>
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        /// The number of PREP-COMPARE shares per element.
        pub(crate) batch_size_prep_compare: usize,

        /// The bitwise shares produced by RANDOM-BITWISE.
        pub(crate) bitwise_shares: Vec<BitwiseNumberShares<T>>,

        /// The first PREP-COMPARE state machine.
        pub(crate) prep_compare_state_machine: PrepCompareStateMachine<T>,

        /// The shares produced by the first PREP-COMPARE.
        pub(crate) prep_compare_shares: Batches<PrepCompareShares<T>>,
    }
}

/// The PREP-MODULO protocol state.
#[derive(StateMachineState)]
#[state_machine(
    recipient_id = "PartyId",
    input_message = "PartyMessage<PrepModuloStateMessage>",
    output_message = "PrepModuloStateMessage",
    final_result = "PrepModuloStateOutput<PrepModuloShares<T>>",
    handle_message_fn = "Self::handle_message"
)]
pub enum PrepModuloState<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// We are waiting for RAN-BIT.
    #[state_machine(submachine = "state.ran_bit_state_machine", transition_fn = "Self::transition_waiting_ran_bit")]
    WaitingRanBitwise(states::WaitingRanBitwise<T>),

    /// We are waiting for PREP-COMPARISON.
    #[state_machine(
        submachine = "state.prep_compare_state_machine",
        transition_fn = "Self::transition_waiting_compare"
    )]
    WaitingCompare(states::WaitingCompare<T>),
}

use PrepModuloState::*;

impl<T> PrepModuloState<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Construct a new PREP-MODULO state.
    pub fn new(
        element_count: usize,
        kappa: usize,
        k: usize,
        secret_sharer: Arc<ShamirSecretSharer<T>>,
    ) -> Result<(Self, Vec<StateMachineMessage<Self>>), PrepModuloCreateError> {
        // Check sizes
        let prime_length = T::MODULO.bits();
        if k.checked_add(kappa).ok_or(PrepModuloCreateError::IntegerOverflow)? >= prime_length {
            return Err(PrepModuloCreateError::StatisticalAndMaxSecretLargeForFieldSize);
        }

        let batch_size_bits = k.checked_add(kappa).ok_or(PrepModuloCreateError::IntegerOverflow)?;

        let batch_size_prep_compare = 2;
        let total_prep_compare =
            element_count.checked_mul(batch_size_prep_compare).ok_or(PrepModuloCreateError::IntegerOverflow)?;

        let (ran_bit_state, messages) =
            RanBitwiseState::new(RanBitwiseMode::new_sized(batch_size_bits), element_count, secret_sharer.clone())?;
        let state = states::WaitingRanBitwise {
            ran_bit_state_machine: StateMachine::new(ran_bit_state),
            secret_sharer,
            batch_size_prep_compare,
            total_prep_compare,
            bitwise_shares: Vec::new(),
        };
        let messages = messages.into_iter().map(|message| message.wrap(&PrepModuloStateMessage::RanBitwise)).collect();
        Ok((WaitingRanBitwise(state), messages))
    }

    fn transition_waiting_ran_bit(state: states::WaitingRanBitwise<T>) -> StateMachineStateResult<Self> {
        let (prep_compare_state, messages) =
            PrepCompareState::new(state.total_prep_compare, state.secret_sharer.clone())
                .map_err(|e| anyhow!("failed to create LAMBDA state: {e}"))?;
        let messages = messages.into_iter().map(|message| message.wrap(&PrepModuloStateMessage::PrepCompare)).collect();
        let next_state = states::WaitingCompare {
            bitwise_shares: state.bitwise_shares,
            prep_compare_state_machine: StateMachine::new(prep_compare_state),
            batch_size_prep_compare: state.batch_size_prep_compare,
            prep_compare_shares: Batches::default(),
        };
        Ok(StateMachineStateOutput::Messages(WaitingCompare(next_state), messages))
    }

    fn transition_waiting_compare(state: states::WaitingCompare<T>) -> StateMachineStateResult<Self> {
        let zipped = state.bitwise_shares.into_iter().zip(state.prep_compare_shares);
        let mut shares = Vec::new();
        for (bit, prep_compare) in zipped {
            let share = PrepModuloShares { ran_bits_r: bit, prep_compare };
            shares.push(share);
        }
        let output = PrepModuloStateOutput::Success { shares };
        Ok(StateMachineStateOutput::Final(output))
    }

    fn handle_message(mut state: Self, message: PartyMessage<PrepModuloStateMessage>) -> StateMachineStateResult<Self> {
        use PrepModuloStateMessage::*;
        let (party_id, message) = message.into_parts();
        match (message, &mut state) {
            (RanBitwise(message), WaitingRanBitwise(inner)) => {
                match inner.ran_bit_state_machine.handle_message(PartyMessage::new(party_id, message))? {
                    StateMachineOutput::Final(RanBitwiseStateOutput::Success { shares }) => {
                        inner.bitwise_shares = shares;
                        state.try_next()
                    }
                    StateMachineOutput::Final(_) => Ok(StateMachineStateOutput::Final(PrepModuloStateOutput::RanAbort)),
                    output => state.wrap_message(output, PrepModuloStateMessage::RanBitwise),
                }
            }
            (PrepCompare(message), WaitingCompare(inner)) => {
                match inner.prep_compare_state_machine.handle_message(PartyMessage::new(party_id, message))? {
                    StateMachineOutput::Final(PrepCompareStateOutput::Success { shares }) => {
                        let prep_compare_shares = Batches::from_flattened_fixed(shares, inner.batch_size_prep_compare)
                            .map_err(|e| anyhow!("failed to construct PREP-COMPARE batches: {e}"))?;
                        inner.prep_compare_shares = prep_compare_shares;
                        state.try_next()
                    }
                    StateMachineOutput::Final(_) => {
                        Ok(StateMachineStateOutput::Final(PrepModuloStateOutput::PrepCompareAbort))
                    }
                    output => state.wrap_message(output, PrepModuloStateMessage::PrepCompare),
                }
            }
            (message, _) => Ok(StateMachineStateOutput::OutOfOrder(state, PartyMessage::new(party_id, message))),
        }
    }
}

/// A message for the PREP-MODULO protocol.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[repr(u8)]
pub enum PrepModuloStateMessage {
    /// A message for the RAN-BIT state machine.
    RanBitwise(RanBitwiseStateMessage) = 0,

    /// A message for the PREP-COMPARE state machine.
    PrepCompare(PrepCompareStateMessage) = 1,
}

/// An error during the creation of the PREP-MODULO state.
#[derive(Debug, thiserror::Error)]
pub enum PrepModuloCreateError {
    /// An integer overflow.
    #[error("integer overflow")]
    IntegerOverflow,

    /// An error during the RAN-BIT creation.
    #[error("RAN-BIT: {0}")]
    RanBitwise(#[from] RanBitwiseCreateError),

    /// An error during PREP-COMPARE creation.
    #[error("PREP-COMPARE: {0}")]
    PrepCompare(#[from] PrepCompareCreateError),

    /// An error when statistical parameter kappa
    /// and k are larger than the field size
    #[error("Statistical parameter kappa and k are too large for current field size")]
    StatisticalAndMaxSecretLargeForFieldSize,

    /// An error during the batch process.
    #[error("Batch: {0}")]
    CreateBatch(#[from] NotEnoughElements),
}

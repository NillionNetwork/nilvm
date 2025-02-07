//! PREP-DIV-INT-SECRET protocol.
//!
//! This is the preprocessing protocol that speeds up the online phase in the DIV-INT-SECRET protocol.

use super::PrepDivisionIntegerSecretShares;
use crate::{
    conditionals::less_than::offline::{
        state::{PrepCompareCreateError, PrepCompareState, PrepCompareStateMessage},
        PrepCompareStateOutput,
    },
    division::{
        modulo2m_public_divisor::offline::{
            state::{PrepModulo2mState, PrepModulo2mStateMessage},
            PrepModulo2mStateOutput,
        },
        truncation_probabilistic::offline::{
            state::{PrepTruncPrState, PrepTruncPrStateMessage},
            PrepTruncPrStateOutput,
        },
    },
    random::random_bitwise::{RanBitwiseMode, RanBitwiseState, RanBitwiseStateMessage, RanBitwiseStateOutput},
};
use anyhow::anyhow;
use basic_types::{batches::NotEnoughElements, Batches, PartyId, PartyMessage};
use math_lib::modular::{AsBits, SafePrime};
use serde::{Deserialize, Serialize};
use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer};
use state_machine::{
    errors::StateMachineError, sm::StateMachineOutput, state::StateMachineMessage, StateMachine, StateMachineState,
    StateMachineStateExt, StateMachineStateOutput, StateMachineStateResult,
};
use state_machine_derive::StateMachineState;
use std::{f64::consts::SQRT_2, sync::Arc};

/// ALPHA parameter used when calculating initial guess
const ALPHA: f64 = 1.5 - SQRT_2;

/// The protocol states.
pub mod states {
    use crate::{
        conditionals::less_than::{offline::output::PrepCompareShares, PrepCompareStateMachine},
        division::{
            modulo2m_public_divisor::{offline::PrepModulo2mShares, PrepModulo2mStateMachine},
            truncation_probabilistic::{offline::PrepTruncPrShares, PrepTruncPrStateMachine},
        },
        random::random_bitwise::{BitwiseNumberShares, RanBitwiseStateMachine},
    };
    use basic_types::Batches;
    use math_lib::modular::SafePrime;
    use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer};
    use std::sync::Arc;

    /// We are waiting for the PREP-COMPARE
    pub struct WaitingCompare<T>
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        /// The PREP-COMPARE state machine.
        pub(crate) prep_compare_state_machine: PrepCompareStateMachine<T>,

        /// The shares produced by  PREP-COMPARE.
        pub(crate) prep_compare_shares: Vec<PrepCompareShares<T>>,

        /// Number of elements in this batch.
        pub(crate) element_count: usize,

        /// The batch size needed per element
        pub(crate) batch_size: usize,

        /// Statistic Kappa security parameter
        pub(crate) kappa: usize,

        /// K, the size of the representation in bits
        pub(crate) k: usize,

        /// The fixed point precision
        pub(crate) precision: usize,

        /// The Shamir Secret Sharer used.
        pub(crate) secret_sharer: Arc<ShamirSecretSharer<T>>,
    }

    /// The protocol is waiting for TRUNCPR
    pub struct WaitingTruncPr<T>
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        /// The TRUNCPR state machine
        pub(crate) state_machine: PrepTruncPrStateMachine<T>,

        /// The shares produced by PREP-COMPARE.
        pub(crate) prep_compare_shares: Batches<PrepCompareShares<T>>,

        /// The truncated values
        pub(crate) prep_truncpr_shares: Vec<PrepTruncPrShares<T>>,

        /// The batch size needed per element
        pub(crate) batch_size: usize,

        /// Number of elements in this batch.
        pub(crate) element_count: usize,

        /// Statistic Kappa security parameter
        pub(crate) kappa: usize,

        /// K, the size of the representation in bits
        pub(crate) k: usize,

        /// The Shamir Secret Sharer used.
        pub(crate) secret_sharer: Arc<ShamirSecretSharer<T>>,
    }

    /// The protocol is waiting for TRUNC
    pub struct WaitingTrunc<T>
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        /// The TRUNC state machine
        pub(crate) state_machine: PrepModulo2mStateMachine<T>,

        /// The shares produced by  PREP-COMPARE.
        pub(crate) prep_compare_shares: Batches<PrepCompareShares<T>>,

        /// The shares produced by PREP-TRUNCPR
        pub(crate) prep_truncpr_shares: Batches<PrepTruncPrShares<T>>,

        /// The shares produced by PREP-TRUNC
        pub(crate) prep_trunc_shares: Vec<PrepModulo2mShares<T>>,

        /// Number of elements in this batch.
        pub(crate) element_count: usize,

        /// The Shamir Secret Sharer used.
        pub(crate) secret_sharer: Arc<ShamirSecretSharer<T>>,
    }

    /// The protocol is waiting for RANDOM-BITWISE
    pub struct WaitingRanBitwise<T>
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        /// The RANDOM-BITWISE state machine
        pub(crate) state_machine: RanBitwiseStateMachine<T>,

        /// The shares produced by  PREP-COMPARE.
        pub(crate) prep_compare_shares: Batches<PrepCompareShares<T>>,

        /// The shares produced by PREP-TRUNCPR
        pub(crate) prep_truncpr_shares: Batches<PrepTruncPrShares<T>>,

        /// The shares produced by PREP-TRUNC
        pub(crate) prep_trunc_shares: Vec<PrepModulo2mShares<T>>,

        /// The shares produced by RANDOM-BITWISE
        pub(crate) prep_bit_decompose: Vec<BitwiseNumberShares<T>>,
    }
}

/// The PREP-DIV-UINT-SECRET protocol state.
#[derive(StateMachineState)]
#[state_machine(
    recipient_id = "PartyId",
    input_message = "PartyMessage<PrepDivisionIntegerSecretStateMessage>",
    output_message = "PrepDivisionIntegerSecretStateMessage",
    final_result = "PrepDivisionIntegerSecretStateOutput<PrepDivisionIntegerSecretShares<T>>",
    handle_message_fn = "Self::handle_message"
)]
pub enum PrepDivisionIntegerSecretState<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// We are waiting for PREP-COMPARE.
    #[state_machine(
        submachine = "state.prep_compare_state_machine",
        transition_fn = "Self::transition_waiting_compare"
    )]
    WaitingCompare(states::WaitingCompare<T>),

    /// We are waiting for PREP-TRUNCPR.
    #[state_machine(submachine = "state.state_machine", transition_fn = "Self::transition_waiting_truncpr")]
    WaitingTruncPr(states::WaitingTruncPr<T>),

    /// We are waiting for PREP-TRUNC.
    #[state_machine(submachine = "state.state_machine", transition_fn = "Self::transition_waiting_trunc")]
    WaitingTrunc(states::WaitingTrunc<T>),

    /// We are waiting for RANDOM-BITWISE.
    #[state_machine(submachine = "state.state_machine", transition_fn = "Self::transition_waiting_random_bitwise")]
    WaitingRanBitwise(states::WaitingRanBitwise<T>),
}

use PrepDivisionIntegerSecretState::*;

use super::PrepDivisionIntegerSecretStateOutput;

impl<T> PrepDivisionIntegerSecretState<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Construct a new PREP-DIV-INT-SECRET state.
    pub fn new(
        element_count: usize,
        kappa: usize,
        k: usize,
        secret_sharer: Arc<ShamirSecretSharer<T>>,
    ) -> Result<(Self, Vec<StateMachineMessage<Self>>), PrepDivisionIntegerSecretCreateError> {
        // Check sizes
        let prime_length = T::MODULO.bits();
        let length = k
            .checked_mul(2)
            .ok_or(PrepDivisionIntegerSecretCreateError::IntegerOverflow)?
            .checked_add(kappa)
            .ok_or(PrepDivisionIntegerSecretCreateError::IntegerOverflow)?;
        if length >= prime_length {
            return Err(PrepDivisionIntegerSecretCreateError::StatisticalAndMaxSecretLargeForFieldSize);
        }
        let precision = k / 2;
        // We need 2 compares for sign calculation and 2 compares for correcting estimate.
        let batch_size = 4;
        let total_prep_compare =
            element_count.checked_mul(batch_size).ok_or(PrepDivisionIntegerSecretCreateError::IntegerOverflow)?;

        let (prep_compare_state, messages) = PrepCompareState::new(total_prep_compare, secret_sharer.clone())?;
        let state = states::WaitingCompare {
            prep_compare_state_machine: StateMachine::new(prep_compare_state),
            prep_compare_shares: Vec::new(),
            batch_size,
            element_count,
            kappa,
            k,
            precision,
            secret_sharer,
        };
        let messages = messages
            .into_iter()
            .map(|message| message.wrap(&PrepDivisionIntegerSecretStateMessage::PrepCompare))
            .collect();
        Ok((WaitingCompare(state), messages))
    }

    fn transition_waiting_compare(state: states::WaitingCompare<T>) -> StateMachineStateResult<Self> {
        let prep_compare_shares = Batches::from_flattened_fixed(state.prep_compare_shares, state.batch_size)
            .map_err(|e| StateMachineError::UnexpectedError(anyhow!("Not enough PREP-COMPARE shares {e}")))?;

        let total_rounds = ((-(state.precision as f64)) / ALPHA.log2()).log2().ceil() as usize;
        // We need 2 iterations of multiplication-and-truncation for every round in the loop plus a multiplication-and-truncation after the loop.
        let batch_size = total_rounds
            .checked_mul(2)
            .ok_or_else(|| anyhow!("integer overflow"))?
            .checked_add(1)
            .ok_or_else(|| anyhow!("integer overflow"))?;
        let element_count = state.element_count.checked_mul(batch_size).ok_or_else(|| anyhow!("integer overflow"))?;
        let (prep_truncpr_state, messages) =
            PrepTruncPrState::new(element_count, state.kappa, state.k, state.secret_sharer.clone()).map_err(|e| {
                StateMachineError::UnexpectedError(anyhow!("Unable to create PREP-TRUNCPR state machine {e}"))
            })?;
        let messages = messages
            .into_iter()
            .map(|message| message.wrap(&PrepDivisionIntegerSecretStateMessage::PrepTruncPr))
            .collect();
        let next_state = states::WaitingTruncPr {
            state_machine: StateMachine::new(prep_truncpr_state),
            prep_compare_shares,
            prep_truncpr_shares: vec![],
            batch_size,
            element_count: state.element_count,
            kappa: state.kappa,
            k: state.k,
            secret_sharer: state.secret_sharer,
        };
        Ok(StateMachineStateOutput::Messages(WaitingTruncPr(next_state), messages))
    }

    fn transition_waiting_truncpr(state: states::WaitingTruncPr<T>) -> StateMachineStateResult<Self> {
        let prep_truncpr_batches = Batches::from_flattened_fixed(state.prep_truncpr_shares, state.batch_size)
            .map_err(|e| StateMachineError::UnexpectedError(anyhow!("Not enough PREP-TRUNCPR shares {e}")))?;
        let (prep_trunc_state, messages) =
            PrepModulo2mState::new(state.element_count, state.kappa, state.k, state.secret_sharer.clone()).map_err(
                |e| StateMachineError::UnexpectedError(anyhow!("Unable to create PREP-TRUNCPR state machine {e}")),
            )?;
        let messages = messages
            .into_iter()
            .map(|message| message.wrap(&PrepDivisionIntegerSecretStateMessage::PrepTrunc))
            .collect();
        let next_state = states::WaitingTrunc {
            secret_sharer: state.secret_sharer,
            state_machine: StateMachine::new(prep_trunc_state),
            prep_compare_shares: state.prep_compare_shares,
            prep_truncpr_shares: prep_truncpr_batches,
            prep_trunc_shares: vec![],
            element_count: state.element_count,
        };
        Ok(StateMachineStateOutput::Messages(WaitingTrunc(next_state), messages))
    }

    fn transition_waiting_trunc(state: states::WaitingTrunc<T>) -> StateMachineStateResult<Self> {
        let (bitwise_state, messages) =
            RanBitwiseState::new(RanBitwiseMode::Full, state.element_count, state.secret_sharer.clone()).map_err(
                |e| StateMachineError::UnexpectedError(anyhow!("Unable to create RANDOM-BITWISE state machine {e}")),
            )?;
        let state = states::WaitingRanBitwise {
            state_machine: StateMachine::new(bitwise_state),
            prep_compare_shares: state.prep_compare_shares,
            prep_truncpr_shares: state.prep_truncpr_shares,
            prep_trunc_shares: state.prep_trunc_shares,
            prep_bit_decompose: Vec::new(),
        };
        let messages = messages
            .into_iter()
            .map(|message| message.wrap(&PrepDivisionIntegerSecretStateMessage::RanBitwise))
            .collect();
        Ok(StateMachineStateOutput::Messages(WaitingRanBitwise(state), messages))
    }

    fn transition_waiting_random_bitwise(state: states::WaitingRanBitwise<T>) -> StateMachineStateResult<Self> {
        let shares =
            state
                .prep_compare_shares
                .into_iter()
                .zip(state.prep_truncpr_shares)
                .zip(state.prep_trunc_shares)
                .zip(state.prep_bit_decompose)
                .map(|(((prep_compare, prep_truncpr), prep_trunc), prep_bit_decompose)| {
                    PrepDivisionIntegerSecretShares { prep_compare, prep_truncpr, prep_trunc, prep_bit_decompose }
                })
                .collect();
        Ok(StateMachineStateOutput::Final(PrepDivisionIntegerSecretStateOutput::Success { shares }))
    }

    fn handle_message(
        mut state: Self,
        message: PartyMessage<PrepDivisionIntegerSecretStateMessage>,
    ) -> StateMachineStateResult<Self> {
        use PrepDivisionIntegerSecretStateMessage::*;
        let (party_id, message) = message.into_parts();
        match (message, &mut state) {
            (PrepCompare(message), WaitingCompare(inner)) => {
                match inner.prep_compare_state_machine.handle_message(PartyMessage::new(party_id, message))? {
                    StateMachineOutput::Final(PrepCompareStateOutput::Success { shares }) => {
                        inner.prep_compare_shares = shares;
                        state.try_next()
                    }
                    StateMachineOutput::Final(_) => {
                        Ok(StateMachineStateOutput::Final(PrepDivisionIntegerSecretStateOutput::PrepCompareAbort))
                    }
                    output => state.wrap_message(output, PrepDivisionIntegerSecretStateMessage::PrepCompare),
                }
            }
            (PrepTruncPr(message), WaitingTruncPr(inner)) => {
                match inner.state_machine.handle_message(PartyMessage::new(party_id, message))? {
                    StateMachineOutput::Final(PrepTruncPrStateOutput::Success { shares }) => {
                        inner.prep_truncpr_shares = shares;
                        state.try_next()
                    }
                    StateMachineOutput::Final(_) => {
                        Ok(StateMachineStateOutput::Final(PrepDivisionIntegerSecretStateOutput::PrepTruncPrAbort))
                    }
                    output => state.wrap_message(output, PrepDivisionIntegerSecretStateMessage::PrepTruncPr),
                }
            }
            (PrepTrunc(message), WaitingTrunc(inner)) => {
                match inner.state_machine.handle_message(PartyMessage::new(party_id, message))? {
                    StateMachineOutput::Final(PrepModulo2mStateOutput::Success { shares }) => {
                        inner.prep_trunc_shares = shares;
                        state.try_next()
                    }
                    StateMachineOutput::Final(_) => {
                        Ok(StateMachineStateOutput::Final(PrepDivisionIntegerSecretStateOutput::PrepTruncPrAbort))
                    }
                    output => state.wrap_message(output, PrepDivisionIntegerSecretStateMessage::PrepTrunc),
                }
            }
            (RanBitwise(message), WaitingRanBitwise(inner)) => {
                match inner.state_machine.handle_message(PartyMessage::new(party_id, message))? {
                    StateMachineOutput::Final(RanBitwiseStateOutput::Success { shares }) => {
                        inner.prep_bit_decompose = shares;
                        state.try_next()
                    }
                    StateMachineOutput::Final(_) => {
                        Ok(StateMachineStateOutput::Final(PrepDivisionIntegerSecretStateOutput::RanBitwiseAbort))
                    }
                    output => state.wrap_message(output, PrepDivisionIntegerSecretStateMessage::RanBitwise),
                }
            }
            (message, _) => Ok(StateMachineStateOutput::OutOfOrder(state, PartyMessage::new(party_id, message))),
        }
    }
}

/// A message for the PREP-DIV-INT-SECRET protocol.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[repr(u8)]
pub enum PrepDivisionIntegerSecretStateMessage {
    /// A message for the PREP-COMPARE state machine.
    PrepCompare(PrepCompareStateMessage) = 0,

    /// A message for the PREP-TRUNCPR state machine
    PrepTruncPr(PrepTruncPrStateMessage) = 1,

    /// A message for the PREP-TRUNC state machine
    PrepTrunc(PrepModulo2mStateMessage) = 2,

    /// A message for the RANDOM-BITWISE state machine
    RanBitwise(RanBitwiseStateMessage) = 3,
}

/// An error during the creation of the PREP-DIV-INT-SECRET state.
#[derive(Debug, thiserror::Error)]
pub enum PrepDivisionIntegerSecretCreateError {
    /// An integer overflow.
    #[error("integer overflow")]
    IntegerOverflow,

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

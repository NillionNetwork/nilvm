//! The SCALE protocol state machine.

use crate::{
    bit_operations::{
        bit_decompose::{BitDecomposeCreateError, BitDecomposeOperands, BitDecomposeState, BitDecomposeStateMessage},
        postfix_or::{PostfixOrState, PostfixOrStateMessage},
    },
    random::random_bitwise::BitwiseNumberShares,
};
use anyhow::{anyhow, Error};
use basic_types::{PartyId, PartyMessage};
use math_lib::modular::{ModularNumber, SafePrime};
use serde::{Deserialize, Serialize};
use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer};
use state_machine::{
    sm::StateMachineOutput, state::StateMachineMessage, StateMachine, StateMachineState, StateMachineStateExt,
    StateMachineStateOutput, StateMachineStateResult,
};
use state_machine_derive::StateMachineState;
use std::sync::Arc;

/// The states of the protocol.
pub mod states {
    use crate::{
        bit_operations::{bit_decompose::BitDecomposeStateMachine, postfix_or::PostfixOrStateMachine},
        random::random_bitwise::BitwiseNumberShares,
    };
    use math_lib::modular::SafePrime;
    use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer};
    use std::sync::Arc;

    /// We are waiting for BIT-DECOMPOSE.
    pub struct WaitingBitDecompose<T>
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        /// The secret sharer.
        pub(crate) secret_sharer: Arc<ShamirSecretSharer<T>>,

        /// The BIT-DECOMPOSE state machine.
        pub(crate) state_machine: BitDecomposeStateMachine<T>,

        /// Provided precision.
        pub(crate) precision: usize,

        /// The revealed values.
        pub(crate) results: Vec<BitwiseNumberShares<T>>,
    }

    /// We are waiting for POSTFIX-OR.
    pub struct WaitingPostfixOr<T>
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        /// The POSTFIX-OR state machine.
        pub(crate) state_machine: PostfixOrStateMachine<T>,

        /// Provided precision.
        pub(crate) precision: usize,

        /// The results of the postfix or.
        pub(crate) results: Vec<BitwiseNumberShares<T>>,
    }
}

/// The SCALE protocol state.
#[derive(StateMachineState)]
#[state_machine(
    recipient_id = "PartyId",
    input_message = "PartyMessage<ScaleStateMessage>",
    output_message = "ScaleStateMessage",
    final_result = "(Vec<ModularNumber<T>>, ModularNumber<T>)",
    handle_message_fn = "Self::handle_message"
)]
pub enum ScaleState<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// We are waiting for the bit decompose to finish.
    #[state_machine(submachine = "state.state_machine", transition_fn = "Self::transition_waiting_bit_decompose")]
    WaitingBitDecompose(states::WaitingBitDecompose<T>),

    /// We are waiting for the postfix or to finish.
    #[state_machine(submachine = "state.state_machine", transition_fn = "Self::transition_waiting_postfix_or")]
    WaitingPostfixOr(states::WaitingPostfixOr<T>),
}

use ScaleState::*;

impl<T> ScaleState<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Construct a new SCALE state.
    pub fn new(
        operands: Vec<BitDecomposeOperands<T>>,
        precision: usize,
        secret_sharer: Arc<ShamirSecretSharer<T>>,
    ) -> Result<(Self, Vec<StateMachineMessage<Self>>), ScaleCreateError> {
        let (state, messages) = BitDecomposeState::new(operands, secret_sharer.clone())?;
        let messages = messages.into_iter().map(|message| message.wrap(&ScaleStateMessage::BitDecompose)).collect();

        let next_state = states::WaitingBitDecompose {
            secret_sharer,
            state_machine: StateMachine::new(state),
            precision,
            results: vec![],
        };
        Ok((Self::WaitingBitDecompose(next_state), messages))
    }

    /// After the bit decompose are finished, call the prefix or.
    fn transition_waiting_bit_decompose(state: states::WaitingBitDecompose<T>) -> StateMachineStateResult<Self> {
        let (or_state, messages) = PostfixOrState::new(state.results, state.secret_sharer.clone())
            .map_err(|e| anyhow!("failed to create PREFIX-OR state: {e}"))?;
        let messages = messages.into_iter().map(|message| message.wrap(&ScaleStateMessage::PostfixOr)).collect();

        let next_state = states::WaitingPostfixOr {
            state_machine: StateMachine::new(or_state),
            precision: state.precision,
            results: vec![],
        };
        Ok(StateMachineStateOutput::Messages(Self::WaitingPostfixOr(next_state), messages))
    }

    /// After the prefix or is finished.
    fn transition_waiting_postfix_or(state: states::WaitingPostfixOr<T>) -> StateMachineStateResult<Self> {
        let (sizes, two_to_exponent) = Self::powers_of_two(state.precision);
        let scales = Self::calculate_scales(state.results, sizes, state.precision)?;

        Ok(StateMachineStateOutput::Final((scales, two_to_exponent)))
    }

    /// Precompute powers of 2.
    fn powers_of_two(precision: usize) -> (Vec<ModularNumber<T>>, ModularNumber<T>) {
        let two = ModularNumber::two();
        let mut sizes = Vec::with_capacity(precision + 1);
        let mut two_i = ModularNumber::ONE;
        sizes.push(two_i);
        for _ in 0..precision {
            two_i = two_i * &two;
            sizes.push(two_i);
        }
        let two_to_exponent = two_i * &two;
        // Reverse sizes for scale calculation
        sizes.reverse();
        (sizes, two_to_exponent)
    }

    /// Calculates scales based on postfix ORs.
    fn calculate_scales(
        scales: Vec<BitwiseNumberShares<T>>,
        sizes: Vec<ModularNumber<T>>,
        precision: usize,
    ) -> Result<Vec<ModularNumber<T>>, Error> {
        // We iterate for each divisor.
        scales
            .into_iter()
            .map(|bits| {
                // First scale, vs[0].
                let mut last = bits.shares().first().ok_or_else(|| anyhow!("scale element not found"))?;
                let mut sum = ModularNumber::ZERO;
                // We iterate through rest of the scales, there are precision many.
                // v = sum([2**(f - i) * (vs[i-1] - vs[i]) for i in range(1, len(vs))])
                for i in 1..precision {
                    // 2**(f-i)
                    let size = sizes.get(i).ok_or_else(|| anyhow!("size element not found"))?;
                    // The i-th scale element for divisor j, vs[i].
                    let scale = bits.shares().get(i).ok_or_else(|| anyhow!("scale element not found"))?;
                    // vs[i-1] - vs[i]
                    let diff = last.value() - scale.value();
                    sum = sum + &(size * &diff);
                    last = scale;
                }
                // Last addition is 2**(f-f) * (vs[f-1] - 0) = vs[f-1].
                sum = sum + last.value();
                Ok(sum)
            })
            .collect()
    }

    fn handle_message(mut state: Self, message: PartyMessage<ScaleStateMessage>) -> StateMachineStateResult<Self> {
        use ScaleStateMessage::*;
        let (party_id, message) = message.into_parts();
        match (message, &mut state) {
            (BitDecompose(message), WaitingBitDecompose(inner)) => {
                match inner.state_machine.handle_message(PartyMessage::new(party_id, message))? {
                    StateMachineOutput::Final(values) => {
                        inner.results = values;
                        state.try_next()
                    }
                    output => state.wrap_message(output, ScaleStateMessage::BitDecompose),
                }
            }
            (PostfixOr(message), WaitingPostfixOr(inner)) => {
                match inner.state_machine.handle_message(PartyMessage::new(party_id, message))? {
                    StateMachineOutput::Final(values) => {
                        inner.results = values;
                        state.try_next()
                    }
                    output => state.wrap_message(output, ScaleStateMessage::PostfixOr),
                }
            }
            (message, _) => Ok(StateMachineStateOutput::OutOfOrder(state, PartyMessage::new(party_id, message))),
        }
    }
}

/// A message for the BIT-DECOMPOSE protocol.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[repr(u8)]
pub enum ScaleStateMessage {
    /// A message for the REVEAL state machine.
    BitDecompose(BitDecomposeStateMessage) = 0,

    /// A message for the PREFIX-OR state machine.
    PostfixOr(PostfixOrStateMessage) = 1,
}

/// An error during the SCALE state creation.
#[derive(Debug, thiserror::Error)]
pub enum ScaleCreateError {
    /// Given operands are empty.
    #[error("Empty operand")]
    Empty,

    /// An error during the BIT-DECOMPOSE creation.
    #[error("BIT-DECOMPOSE: {0}")]
    BitDecompose(#[from] BitDecomposeCreateError),
}

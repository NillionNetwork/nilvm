//! PREP-COMPARE protocol.
//!
//! This is the preprocessing protocol that speeds up the online phase in the COMPARE protocol.

use super::{PrepCompareShares, PrepCompareStateOutput};
use crate::{
    multiplication::{
        multiplication_shares::{
            state::{MultState, MultStateMessage},
            OperandShares,
        },
        multiplication_unbounded::prefix::{
            PrepPrefixMultState, PrepPrefixMultStateMessage, PrepPrefixMultStateOutput,
        },
    },
    random::{
        random_bitwise::{
            BitwiseNumberShares, RanBitwiseCreateError, RanBitwiseMode, RanBitwiseState, RanBitwiseStateMessage,
            RanBitwiseStateOutput,
        },
        random_integer::state::{RandomIntegerState, RandomIntegerStateMessage, RandomMode},
        random_quaternary::QuaternaryShares,
    },
};
use anyhow::{anyhow, Error};
use basic_types::{Batches, PartyId, PartyMessage};
use math_lib::modular::{AsBits, ModularNumber, SafePrime};
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
        multiplication::{
            multiplication_shares::MultStateMachine,
            multiplication_unbounded::{prefix::PrefixMultTuple, PrepPrefixMultStateMachine},
        },
        random::{
            random_bitwise::{BitwiseNumberShares, RanBitwiseStateMachine},
            random_integer::RandomIntegerStateMachine,
            random_quaternary::{QuaternaryShares, RanQuaternaryStateMachine},
        },
    };
    use basic_types::Batches;
    use math_lib::modular::{ModularNumber, SafePrime};
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

        /// The bitwise numbers produced by RANDOM-BITWISE.
        pub(crate) bitwise_numbers: Vec<BitwiseNumberShares<T>>,
    }

    /// We are waiting for RANDOM-BITWISE.
    pub struct WaitingRanQuaternary<T>
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        /// The RANDOM-BITWISE state machine.
        pub(crate) ran_quaternary_state_machine: RanQuaternaryStateMachine<T>,

        /// The secret sharer to be used.
        pub(crate) secret_sharer: Arc<ShamirSecretSharer<T>>,

        /// The number of elements to be produced.
        pub(crate) element_count: usize,

        /// The bitwise numbers produced by RANDOM-BITWISE.
        pub(crate) bitwise_numbers: Vec<BitwiseNumberShares<T>>,

        /// The quaternary numbers produced by RAN-QUATERNARY.
        pub(crate) quaternary_numbers: Vec<QuaternaryShares<T>>,
    }

    /// We are waiting for the MULT that computes the least comparison bit.
    pub struct WaitingCompareLeastBitMult<T>
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        /// The MULT state machine.
        pub(crate) mult_state_machine: MultStateMachine<T>,

        /// The secret sharer to be used.
        pub(crate) secret_sharer: Arc<ShamirSecretSharer<T>>,

        /// The number of elements to be produced.
        pub(crate) element_count: usize,

        /// The bitwise numbers produced by RANDOM-BITWISE.
        pub(crate) bitwise_numbers: Vec<BitwiseNumberShares<T>>,

        /// The quaternary numbers produced by RAN-QUATERNARY.
        pub(crate) quaternary_numbers: Vec<QuaternaryShares<T>>,

        /// The output produced by MULT.
        pub(crate) product_shares: Vec<ModularNumber<T>>,
    }

    /// We are waiting for the MULT that computes the most comparison bit.
    pub struct WaitingCompareMostBitMult<T>
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        /// The MULT state machine.
        pub(crate) mult_state_machine: MultStateMachine<T>,

        /// The secret sharer to be used.
        pub(crate) secret_sharer: Arc<ShamirSecretSharer<T>>,

        /// The number of elements to be produced.
        pub(crate) element_count: usize,

        /// The bitwise numbers produced by RANDOM-BITWISE.
        pub(crate) bitwise_numbers: Vec<BitwiseNumberShares<T>>,

        /// The quaternary numbers produced by RAN-QUATERNARY.
        pub(crate) quaternary_numbers: Vec<QuaternaryShares<T>>,

        /// The output produced by the least bit MULT.
        pub(crate) compare_least_bit_shares: Vec<ModularNumber<T>>,

        /// The output produced by this MULT.
        pub(crate) product_shares: Vec<ModularNumber<T>>,
    }

    /// We are waiting for PREP-PREFIX-MULT.
    pub struct WaitingPrepPrefixMult<T>
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        /// The PREP-PREFIX-MULT state machine.
        pub(crate) prep_prefix_mult_state_machine: PrepPrefixMultStateMachine<T>,

        /// The secret sharer to be used.
        pub(crate) secret_sharer: Arc<ShamirSecretSharer<T>>,

        /// The number of elements to be produced.
        pub(crate) element_count: usize,

        /// The bitwise numbers produced by RANDOM-BITWISE.
        pub(crate) bitwise_numbers: Vec<BitwiseNumberShares<T>>,

        /// The quaternary numbers produced by RAN-QUATERNARY.
        pub(crate) quaternary_numbers: Vec<QuaternaryShares<T>>,

        /// The output produced by the least bit MULT.
        pub(crate) compare_least_bit_shares: Vec<ModularNumber<T>>,

        /// The output produced by this MULT.
        pub(crate) compare_most_bit_shares: Vec<ModularNumber<T>>,

        /// The tuples produced by PREP-PREFIX-MULT.
        pub(crate) prefix_mult_tuples: Batches<PrefixMultTuple<T>>,
    }

    /// We are waiting for RAN-ZERO.
    pub struct WaitingRanZero<T>
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        /// The PREP-PREFIX-MULT state machine.
        pub(crate) ran_zero_state_machine: RandomIntegerStateMachine<T>,

        /// The bitwise numbers produced by RANDOM-BITWISE.
        pub(crate) bitwise_numbers: Vec<BitwiseNumberShares<T>>,

        /// The quaternary numbers produced by RAN-QUATERNARY.
        pub(crate) quaternary_numbers: Vec<QuaternaryShares<T>>,

        /// The output produced by the least bit MULT.
        pub(crate) compare_least_bit_shares: Vec<ModularNumber<T>>,

        /// The output produced by this MULT.
        pub(crate) compare_most_bit_shares: Vec<ModularNumber<T>>,

        /// The tuples produced by PREP-PREFIX-MULT.
        pub(crate) prefix_mult_tuples: Batches<PrefixMultTuple<T>>,

        /// The size of each batch.
        pub(crate) batch_size: usize,

        /// The random shares of zeros produced by RAN-ZERO.
        pub(crate) zero_shares: Batches<ModularNumber<T>>,
    }
}

/// The PREP-COMPARE protocol state.
#[derive(StateMachineState)]
#[state_machine(
    recipient_id = "PartyId",
    input_message = "PartyMessage<PrepCompareStateMessage>",
    output_message = "PrepCompareStateMessage",
    final_result = "PrepCompareStateOutput<PrepCompareShares<T>>",
    handle_message_fn = "Self::handle_message"
)]
pub enum PrepCompareState<T>
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

    /// We are waiting for RAN-QUATERNARY.
    #[state_machine(
        submachine = "state.ran_quaternary_state_machine",
        transition_fn = "Self::transition_waiting_ran_quaternary"
    )]
    WaitingRanQuaternary(states::WaitingRanQuaternary<T>),

    /// We are waiting for the least compare bit MULT.
    #[state_machine(
        submachine = "state.mult_state_machine",
        transition_fn = "Self::transition_waiting_compare_least_bit_mult"
    )]
    WaitingCompareLeastBitMult(states::WaitingCompareLeastBitMult<T>),

    /// We are waiting for the most compare bit MULT.
    #[state_machine(
        submachine = "state.mult_state_machine",
        transition_fn = "Self::transition_waiting_compare_most_bit_mult"
    )]
    WaitingCompareMostBitMult(states::WaitingCompareMostBitMult<T>),

    /// We are waiting for PREP-PREFIX-MULT.
    #[state_machine(
        submachine = "state.prep_prefix_mult_state_machine",
        transition_fn = "Self::transition_waiting_prep_prefix_mult"
    )]
    WaitingPrepPrefixMult(states::WaitingPrepPrefixMult<T>),

    /// We are waiting for RAN-ZERO.
    #[state_machine(submachine = "state.ran_zero_state_machine", transition_fn = "Self::transition_waiting_ran_zero")]
    WaitingRanZero(states::WaitingRanZero<T>),
}

use crate::random::random_quaternary::{RanQuaternaryState, RanQuaternaryStateMessage, RanQuaternaryStateOutput};
use PrepCompareState::*;

impl<T> PrepCompareState<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Construct a new PREP-COMPARE state.
    pub fn new(
        element_count: usize,
        secret_sharer: Arc<ShamirSecretSharer<T>>,
    ) -> Result<(Self, Vec<StateMachineMessage<Self>>), PrepCompareCreateError> {
        let (bitwise_state, messages) =
            RanBitwiseState::new(RanBitwiseMode::Full, element_count, secret_sharer.clone())?;
        let state = states::WaitingRanBitwise {
            secret_sharer,
            element_count,
            random_bitwise_state_machine: StateMachine::new(bitwise_state),
            bitwise_numbers: Vec::new(),
        };
        let messages = messages.into_iter().map(|message| message.wrap(&PrepCompareStateMessage::RanBitwise)).collect();
        Ok((WaitingRanBitwise(state), messages))
    }

    fn transition_waiting_random_bitwise(state: states::WaitingRanBitwise<T>) -> StateMachineStateResult<Self> {
        let (ran_quaternary_state, messages) =
            RanQuaternaryState::new(RanBitwiseMode::Full, state.element_count, state.secret_sharer.clone())
                .map_err(|e| anyhow!("failed to create RAN-QUATERNARY state: {e}"))?;
        let next_state = states::WaitingRanQuaternary {
            secret_sharer: state.secret_sharer,
            element_count: state.element_count,
            bitwise_numbers: state.bitwise_numbers,
            quaternary_numbers: Vec::new(),
            ran_quaternary_state_machine: StateMachine::new(ran_quaternary_state),
        };
        let messages =
            messages.into_iter().map(|message| message.wrap(&PrepCompareStateMessage::RanQuaternary)).collect();
        Ok(StateMachineStateOutput::Messages(WaitingRanQuaternary(next_state), messages))
    }

    fn transition_waiting_ran_quaternary(state: states::WaitingRanQuaternary<T>) -> StateMachineStateResult<Self> {
        let operands = Self::build_compare_least_bit_operands(&state.bitwise_numbers, &state.quaternary_numbers)?;
        let (mult_state, messages) = MultState::new(operands, state.secret_sharer.clone())
            .map_err(|e| anyhow!("failed to create MULT state: {e}"))?;
        let messages =
            messages.into_iter().map(|message| message.wrap(&PrepCompareStateMessage::CompareLeastBitMult)).collect();
        let next_state = states::WaitingCompareLeastBitMult {
            secret_sharer: state.secret_sharer,
            element_count: state.element_count,
            bitwise_numbers: state.bitwise_numbers,
            quaternary_numbers: state.quaternary_numbers,
            mult_state_machine: StateMachine::new(mult_state),
            product_shares: Vec::new(),
        };
        Ok(StateMachineStateOutput::Messages(WaitingCompareLeastBitMult(next_state), messages))
    }

    fn transition_waiting_compare_least_bit_mult(
        state: states::WaitingCompareLeastBitMult<T>,
    ) -> StateMachineStateResult<Self> {
        let least_bit_shares = Self::build_compare_least_bit_shares(
            &state.bitwise_numbers,
            &state.quaternary_numbers,
            state.product_shares,
        )?;
        let operands = Self::build_compare_most_bit_operands(&state.bitwise_numbers, &least_bit_shares)?;
        let (mult_state, messages) = MultState::new(operands, state.secret_sharer.clone())
            .map_err(|e| anyhow!("failed to create MULT state: {e}"))?;
        let messages =
            messages.into_iter().map(|message| message.wrap(&PrepCompareStateMessage::CompareMostBitMult)).collect();
        let next_state = states::WaitingCompareMostBitMult {
            secret_sharer: state.secret_sharer.clone(),
            element_count: state.element_count,
            bitwise_numbers: state.bitwise_numbers,
            quaternary_numbers: state.quaternary_numbers,
            compare_least_bit_shares: least_bit_shares,
            mult_state_machine: StateMachine::new(mult_state),
            product_shares: Vec::new(),
        };
        Ok(StateMachineStateOutput::Messages(WaitingCompareMostBitMult(next_state), messages))
    }

    fn transition_waiting_compare_most_bit_mult(
        state: states::WaitingCompareMostBitMult<T>,
    ) -> StateMachineStateResult<Self> {
        let most_bit_shares = Self::build_compare_most_bit_shares(
            &state.bitwise_numbers,
            &state.compare_least_bit_shares,
            state.product_shares,
        )?;
        let batch_size = T::MODULO
            .bits()
            .checked_sub(1)
            .ok_or_else(|| anyhow!("integer underflow"))?
            .checked_div(2)
            .ok_or_else(|| anyhow!("integer underflow"))?;
        let (prep_prefix_state, messages) =
            PrepPrefixMultState::new(state.element_count, batch_size, state.secret_sharer.clone())
                .map_err(|e| anyhow!("failed to create PREP-PREFIX-MULT state: {e}"))?;
        let messages =
            messages.into_iter().map(|message| message.wrap(&PrepCompareStateMessage::PrepPrefixMult)).collect();
        let next_state = states::WaitingPrepPrefixMult {
            secret_sharer: state.secret_sharer.clone(),
            bitwise_numbers: state.bitwise_numbers,
            quaternary_numbers: state.quaternary_numbers,
            compare_least_bit_shares: state.compare_least_bit_shares,
            compare_most_bit_shares: most_bit_shares,
            prep_prefix_mult_state_machine: StateMachine::new(prep_prefix_state),
            element_count: state.element_count,
            prefix_mult_tuples: Batches::default(),
        };
        Ok(StateMachineStateOutput::Messages(WaitingPrepPrefixMult(next_state), messages))
    }

    fn transition_waiting_prep_prefix_mult(state: states::WaitingPrepPrefixMult<T>) -> StateMachineStateResult<Self> {
        let batch_size = T::MODULO
            .bits()
            .checked_add(1)
            .ok_or_else(|| anyhow!("integer overflow"))?
            .checked_div(2)
            .ok_or_else(|| anyhow!("integer underflow"))?;
        let zero_element_count =
            batch_size.checked_mul(state.element_count).ok_or_else(|| anyhow!("integer overflow"))?;
        let (ran_zero_state, messages) =
            RandomIntegerState::new(RandomMode::ZerosOfDegree2T, zero_element_count, state.secret_sharer)
                .map_err(|e| anyhow!("failed to create RAN-ZERO state: {e}"))?;
        let messages = messages.into_iter().map(|message| message.wrap(&PrepCompareStateMessage::RanZero)).collect();
        let next_state = states::WaitingRanZero {
            ran_zero_state_machine: StateMachine::new(ran_zero_state),
            bitwise_numbers: state.bitwise_numbers,
            quaternary_numbers: state.quaternary_numbers,
            compare_least_bit_shares: state.compare_least_bit_shares,
            compare_most_bit_shares: state.compare_most_bit_shares,
            prefix_mult_tuples: state.prefix_mult_tuples,
            batch_size,
            zero_shares: Batches::default(),
        };
        Ok(StateMachineStateOutput::Messages(WaitingRanZero(next_state), messages))
    }

    fn transition_waiting_ran_zero(state: states::WaitingRanZero<T>) -> StateMachineStateResult<Self> {
        let zipped = state
            .bitwise_numbers
            .into_iter()
            .zip(state.quaternary_numbers)
            .zip(state.compare_least_bit_shares)
            .zip(state.compare_most_bit_shares)
            .zip(state.prefix_mult_tuples)
            .zip(state.zero_shares);
        let mut shares = Vec::new();
        for (
            ((((bitwise_number, quaternary), comparison_least_bit), comparison_most_bit), prefix_mult_tuples),
            zero_shares,
        ) in zipped
        {
            shares.push(PrepCompareShares {
                bitwise: bitwise_number.merge_bits(),
                quaternary,
                comparison_least_bit,
                comparison_most_bit,
                prefix_mult_tuples,
                zero_shares,
            });
        }
        let output = PrepCompareStateOutput::Success { shares };
        Ok(StateMachineStateOutput::Final(output))
    }

    fn handle_message(
        mut state: Self,
        message: PartyMessage<PrepCompareStateMessage>,
    ) -> StateMachineStateResult<Self> {
        use PrepCompareStateMessage::*;
        let (party_id, message) = message.into_parts();
        match (message, &mut state) {
            (RanBitwise(message), WaitingRanBitwise(inner)) => {
                match inner.random_bitwise_state_machine.handle_message(PartyMessage::new(party_id, message))? {
                    StateMachineOutput::Final(RanBitwiseStateOutput::Success { shares }) => {
                        inner.bitwise_numbers = shares;
                        state.try_next()
                    }
                    StateMachineOutput::Final(_) => {
                        Ok(StateMachineStateOutput::Final(PrepCompareStateOutput::RanBitwiseAbort))
                    }
                    output => state.wrap_message(output, PrepCompareStateMessage::RanBitwise),
                }
            }
            (RanQuaternary(message), WaitingRanQuaternary(inner)) => {
                match inner.ran_quaternary_state_machine.handle_message(PartyMessage::new(party_id, message))? {
                    StateMachineOutput::Final(RanQuaternaryStateOutput::Success { shares }) => {
                        inner.quaternary_numbers = shares;
                        state.try_next()
                    }
                    output => state.wrap_message(output, PrepCompareStateMessage::RanQuaternary),
                }
            }
            (CompareLeastBitMult(message), WaitingCompareLeastBitMult(inner)) => {
                match inner.mult_state_machine.handle_message(PartyMessage::new(party_id, message))? {
                    StateMachineOutput::Final(values) => {
                        inner.product_shares = values;
                        state.try_next()
                    }
                    output => state.wrap_message(output, PrepCompareStateMessage::CompareLeastBitMult),
                }
            }
            (CompareMostBitMult(message), WaitingCompareMostBitMult(inner)) => {
                match inner.mult_state_machine.handle_message(PartyMessage::new(party_id, message))? {
                    StateMachineOutput::Final(values) => {
                        inner.product_shares = values;
                        state.try_next()
                    }
                    output => state.wrap_message(output, PrepCompareStateMessage::CompareMostBitMult),
                }
            }
            (PrepPrefixMult(message), WaitingPrepPrefixMult(inner)) => {
                match inner.prep_prefix_mult_state_machine.handle_message(PartyMessage::new(party_id, message))? {
                    StateMachineOutput::Final(PrepPrefixMultStateOutput::Success { mut shares }) => {
                        shares.iter_mut().for_each(|s| s.reverse());
                        inner.prefix_mult_tuples = shares;
                        state.try_next()
                    }
                    StateMachineOutput::Final(PrepPrefixMultStateOutput::InvRanAbort) => {
                        Ok(StateMachineStateOutput::Final(PrepCompareStateOutput::Abort))
                    }
                    output => state.wrap_message(output, PrepCompareStateMessage::PrepPrefixMult),
                }
            }
            (RanZero(message), WaitingRanZero(inner)) => {
                match inner.ran_zero_state_machine.handle_message(PartyMessage::new(party_id, message))? {
                    StateMachineOutput::Final(values) => {
                        inner.zero_shares = Batches::from_flattened_fixed(values, inner.batch_size)
                            .map_err(|e| anyhow!("batch construction failed: {e}"))?;
                        state.try_next()
                    }
                    output => state.wrap_message(output, PrepCompareStateMessage::RanZero),
                }
            }
            (message, _) => Ok(StateMachineStateOutput::OutOfOrder(state, PartyMessage::new(party_id, message))),
        }
    }

    fn build_compare_least_bit_operands(
        bitwise_numbers: &[BitwiseNumberShares<T>],
        quaternary_numbers: &[QuaternaryShares<T>],
    ) -> Result<Vec<OperandShares<T>>, Error> {
        let mut operands = Vec::new();
        for (bitwise, quaternary) in bitwise_numbers.iter().zip(quaternary_numbers.iter()) {
            let left = bitwise.least()?;
            let right = *quaternary.least()?;
            operands.push(OperandShares::single(left.into(), right));
        }
        Ok(operands)
    }

    fn build_compare_least_bit_shares(
        bitwise_numbers: &[BitwiseNumberShares<T>],
        quaternary_numbers: &[QuaternaryShares<T>],
        bit_products: Vec<ModularNumber<T>>,
    ) -> Result<Vec<ModularNumber<T>>, Error> {
        let mut numbers = Vec::new();
        for ((bitwise, quaternary), bit_product) in
            bitwise_numbers.iter().zip(quaternary_numbers.iter()).zip(bit_products.into_iter())
        {
            let least_least_bit = bitwise.least()?;
            let second_least_bit = quaternary.least()?;
            let number = ModularNumber::from(least_least_bit) + second_least_bit;
            let number = number - &(bit_product + &bit_product);
            numbers.push(number);
        }
        Ok(numbers)
    }

    fn build_compare_most_bit_operands(
        bitwise_numbers: &[BitwiseNumberShares<T>],
        compare_least_bit_shares: &[ModularNumber<T>],
    ) -> Result<Vec<OperandShares<T>>, Error> {
        let mut operands = Vec::new();
        for (bitwise_numbers, least_bit_shares) in bitwise_numbers.iter().zip(compare_least_bit_shares.iter()) {
            let right = bitwise_numbers.most()?;
            operands.push(OperandShares::single(*least_bit_shares, right.into()));
        }
        Ok(operands)
    }

    fn build_compare_most_bit_shares(
        bitwise_numbers: &[BitwiseNumberShares<T>],
        compare_least_bit_shares: &[ModularNumber<T>],
        bit_products: Vec<ModularNumber<T>>,
    ) -> Result<Vec<ModularNumber<T>>, Error> {
        let mut numbers = Vec::new();
        let zipped = bitwise_numbers.iter().zip(compare_least_bit_shares.iter()).zip(bit_products);
        for ((bitwise_numbers, least_bit_shares), bit_product) in zipped {
            let second_least_bit = bitwise_numbers.most()?;
            let number = least_bit_shares + second_least_bit.value();
            let number = number - &(bit_product + &bit_product);
            numbers.push(number);
        }
        Ok(numbers)
    }
}

/// A message for the PREP-COMPARE protocol.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[repr(u8)]
pub enum PrepCompareStateMessage {
    /// A message for the RANDOM-BITWISE state machine.
    RanBitwise(RanBitwiseStateMessage) = 0,

    /// A message for the RAN-QUATERNARY state machine.
    RanQuaternary(RanQuaternaryStateMessage) = 1,

    /// A message for the least bit compare MULT state machine.
    CompareLeastBitMult(MultStateMessage) = 2,

    /// A message for the most bit compare MULT state machine.
    CompareMostBitMult(MultStateMessage) = 3,

    /// A message for the PREP-PREFIX-MULT state machine.
    PrepPrefixMult(PrepPrefixMultStateMessage) = 4,

    /// A message for the RAN-ZERO state machine.
    RanZero(RandomIntegerStateMessage) = 5,
}

/// An error during the creation of the PREP-COMPARE state.
#[derive(Debug, thiserror::Error)]
pub enum PrepCompareCreateError {
    /// An integer overflow.
    #[error("integer overflow")]
    IntegerOverflow,

    /// An error during the RANDOM-BITWISE creation.
    #[error("RANDOM-BITWISE: {0}")]
    RanBitwise(#[from] RanBitwiseCreateError),
}

#[allow(clippy::arithmetic_side_effects, clippy::indexing_slicing)]
#[cfg(test)]
mod test {
    use super::*;
    use crate::random::{random_bit::BitShare, random_quaternary::QuatShare};
    use math_lib::modular::U64SafePrime;

    type Prime = U64SafePrime;
    type State = PrepCompareState<Prime>;

    #[test]
    fn compare_least_bit_operands() {
        let s0 = ModularNumber::ONE;
        let r0 = ModularNumber::from_u32(3);
        let bitwise = &[BitwiseNumberShares::from(vec![BitShare::from(s0), BitShare::from(ModularNumber::two())])];
        let quaternary =
            &[QuaternaryShares::from(vec![QuatShare::new(r0, ModularNumber::from_u32(4), ModularNumber::from_u32(7))])];
        let operands = State::build_compare_least_bit_operands(bitwise, quaternary).expect("build failed");
        assert_eq!(operands.len(), 1);
        assert_eq!(operands[0].left[0], s0);
        assert_eq!(operands[0].right[0], r0);
    }

    #[test]
    fn compare_least_bit_shares() {
        let s0 = ModularNumber::from_u32(100);
        let r0 = ModularNumber::from_u32(300);
        let bitwise =
            &[BitwiseNumberShares::from(vec![BitShare::from(s0), BitShare::from(ModularNumber::from_u32(20))])];
        let quaternary = &[QuaternaryShares::from(vec![QuatShare::new(
            r0,
            ModularNumber::from_u32(40),
            ModularNumber::from_u32(70),
        )])];
        let p = ModularNumber::from_u32(100);
        let products = vec![p];
        let shares = State::build_compare_least_bit_shares(bitwise, quaternary, products).expect("build failed");
        assert_eq!(shares.len(), 1);
        let expected = s0 + &r0 - &(ModularNumber::two() * &p);
        assert_eq!(shares[0], expected);
    }

    #[test]
    fn compare_most_bit_operands() {
        let s_l = ModularNumber::from_u32(20);
        let numbers =
            &[BitwiseNumberShares::from(vec![BitShare::from(ModularNumber::from_u32(10)), BitShare::from(s_l)])];
        let least_bit = ModularNumber::from_u64(35);
        let operands = State::build_compare_most_bit_operands(numbers, &[least_bit]).expect("build failed");
        assert_eq!(operands.len(), 1);
        assert_eq!(operands[0].left[0], least_bit);
        assert_eq!(operands[0].right[0], s_l);
    }

    #[test]
    fn compare_most_bit_shares() {
        let s_l = ModularNumber::from_u32(20);
        let numbers =
            &[BitwiseNumberShares::from(vec![BitShare::from(ModularNumber::from_u32(10)), BitShare::from(s_l)])];
        let w0 = ModularNumber::from_u64(123);
        let p = ModularNumber::from_u32(40);
        let products = vec![p];
        let operands = State::build_compare_most_bit_shares(numbers, &[w0], products).expect("build failed");
        let expected = w0 + &s_l - &(ModularNumber::two() * &p);
        assert_eq!(operands.len(), 1);
        assert_eq!(operands[0], expected);
    }
}

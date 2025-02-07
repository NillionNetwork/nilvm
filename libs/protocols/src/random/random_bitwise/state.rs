//! The RANDOM-BITWISE protocol.

use crate::{
    multiplication::multiplication_public_output::state::{PubMultState, PubMultStateMessage, PubOperandShares},
    random::{
        random_bit::BitShare,
        random_bitwise::BitwiseNumberShares,
        random_integer::state::{RandomIntegerState, RandomIntegerStateMessage, RandomMode},
    },
};
use anyhow::{anyhow, Error};
use basic_types::{Batches, PartyId, PartyMessage};
use math_lib::modular::{AsBits, CheckedSub, Integer, Modular, ModularNumber, SafePrime};
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
    use super::RanBitwiseMode;
    use crate::{
        multiplication::multiplication_public_output::PubMultStateMachine,
        random::{
            random_bit::{BitShare, RanBitStateMachine},
            random_integer::RandomIntegerStateMachine,
        },
    };
    use basic_types::Batches;
    use math_lib::modular::{ModularNumber, SafePrime};
    use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer};
    use std::sync::Arc;

    /// We are waiting for RAN-BIT.
    pub struct WaitingRanBit<T>
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        /// The RAN-BIT state machine.
        pub(crate) ran_bit_state_machine: RanBitStateMachine<T>,

        /// The secret sharer to be used.
        pub(crate) secret_sharer: Arc<ShamirSecretSharer<T>>,

        /// The mode of operation for the protocol.
        pub(crate) mode: RanBitwiseMode,

        /// The number of bits per element.
        pub(crate) batch_size: usize,

        /// The bit shares produced by RAN-BIT.
        pub(crate) bit_shares: Batches<BitShare<T>>,
    }

    /// We are waiting for RAN.
    pub struct WaitingRan<T>
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        /// The RAN state machine.
        pub(crate) ran_state_machine: RandomIntegerStateMachine<T>,

        /// The secret sharer to be used.
        pub(crate) secret_sharer: Arc<ShamirSecretSharer<T>>,

        /// The number of zero bits in our prime.
        pub(crate) batch_size: usize,

        /// The number of masks needed.
        pub(crate) masks_needed: usize,

        /// The verification value shares produced by RAN.
        pub(crate) verification_shares: Batches<ModularNumber<T>>,

        /// The bit shares produced by RAN-BIT.
        pub(crate) bit_shares: Batches<BitShare<T>>,
    }

    /// We are waiting for RAN-ZERO.
    pub struct WaitingRanZero<T>
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        /// The MULT state machine.
        pub(crate) ran_zero_state_machine: RandomIntegerStateMachine<T>,

        /// The secret sharer to be used.
        pub(crate) secret_sharer: Arc<ShamirSecretSharer<T>>,

        /// The number of zero bits in our prime.
        pub(crate) batch_size: usize,

        /// The bit shares produced by RAN-BIT.
        pub(crate) bit_shares: Batches<BitShare<T>>,

        /// The verification value shares produced by RAN.
        pub(crate) verification_shares: Batches<ModularNumber<T>>,

        /// The zero sharings as produced by RAN-ZERO.
        pub(crate) zeros: Batches<ModularNumber<T>>,
    }

    /// We are waiting for PUB-MULT.
    pub struct WaitingPubMult<T>
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        /// The PUB-MULT state machine.
        pub(crate) pub_mult_state_machine: PubMultStateMachine<T>,

        /// The bit shares produced by RAN-BIT.
        pub(crate) bit_shares: Batches<BitShare<T>>,

        /// The REVEAL output.
        pub(crate) verification_values: Vec<ModularNumber<T>>,
    }
}

/// The output of the RANDOM-BITWISE protocol.
pub enum RanBitwiseStateOutput<T: Modular> {
    /// The protocol was successful.
    Success {
        /// The output shares.
        shares: Vec<BitwiseNumberShares<T>>,
    },

    /// RAN-BIT aborted.
    RanBitAbort,

    /// The protocol failed.
    ///
    /// This happens naturally with probability 1/p based.
    Abort,
}

/// The RANDOM-BITWISE state machine.
#[derive(StateMachineState)]
#[state_machine(
    recipient_id = "PartyId",
    input_message = "PartyMessage<RanBitwiseStateMessage>",
    output_message = "RanBitwiseStateMessage",
    final_result = "RanBitwiseStateOutput<T>",
    handle_message_fn = "Self::handle_message"
)]
pub enum RanBitwiseState<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// We are waiting for RAN-BIT.
    #[state_machine(submachine = "state.ran_bit_state_machine", transition_fn = "Self::transition_waiting_ran_bit")]
    WaitingRanBit(states::WaitingRanBit<T>),

    /// We are waiting for RAN.
    #[state_machine(submachine = "state.ran_state_machine", transition_fn = "Self::transition_waiting_ran")]
    WaitingRan(states::WaitingRan<T>),

    /// We are waiting for RAN-ZERO.
    #[state_machine(submachine = "state.ran_zero_state_machine", transition_fn = "Self::transition_waiting_ran_zero")]
    WaitingRanZero(states::WaitingRanZero<T>),

    /// We are waiting for PUB-MULT.
    #[state_machine(submachine = "state.pub_mult_state_machine", transition_fn = "Self::transition_waiting_pub_mult")]
    WaitingPubMult(states::WaitingPubMult<T>),
}

use crate::random::random_bit::{RandomBitCreateError, RandomBitState, RandomBitStateMessage, RandomBitStateOutput};
use RanBitwiseState::*;

/// The mode we want to run RANDOM-BITWISE on.
///
/// There are 2 flavors of RANDOM-BITWISE:
/// * A FULL RANDOM-BITWISE where the number of bits equals the size of field.
/// * A SIZED RANDOM-BITWISE where the number of bits is specified by the caller.
///
/// This enum wraps that behavior: [`RanBitwiseMode::Full`] is the first one,
/// [`RanBitwiseMode::Sized`] is the second one.
#[derive(Debug, Clone)]
pub enum RanBitwiseMode {
    /// We are generating a uniformly random bitwise number in the field.
    Full,

    /// We are generating a uniformly random bitwise number of size bits.
    Sized {
        /// The bit size of the bitwise shared number.
        size: usize,
    },
}

impl RanBitwiseMode {
    /// Constructs a new direct RANDOM-BITWISE mode.
    pub fn new_sized(size: usize) -> Self {
        Self::Sized { size }
    }
}

impl<T> RanBitwiseState<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Construct a new RANDOM-BITWISE state.
    pub fn new(
        mode: RanBitwiseMode,
        elements: usize,
        secret_sharer: Arc<ShamirSecretSharer<T>>,
    ) -> Result<(Self, Vec<StateMachineMessage<Self>>), RanBitwiseCreateError> {
        let batch_size = match mode {
            RanBitwiseMode::Full => T::Normal::BITS,
            RanBitwiseMode::Sized { size } => size,
        };
        let total_bits = batch_size.checked_mul(elements).ok_or(RanBitwiseCreateError::IntegerOverflow)?;
        let (ran_bit_state, messages) = RandomBitState::new(total_bits, secret_sharer.clone())?;
        let messages = messages.into_iter().map(|message| message.wrap(&RanBitwiseStateMessage::RanBit)).collect();
        let state = states::WaitingRanBit {
            ran_bit_state_machine: StateMachine::new(ran_bit_state),
            secret_sharer,
            mode,
            batch_size,
            bit_shares: Batches::default(),
        };
        Ok((WaitingRanBit(state), messages))
    }

    fn transition_waiting_ran_bit(state: states::WaitingRanBit<T>) -> StateMachineStateResult<Self> {
        match state.mode {
            RanBitwiseMode::Full => {
                // SAFETY: modulo can't be zero so this can't underflow.
                let prime_minus_one = T::MODULO.checked_sub(&T::Normal::from(1)).unwrap();
                let prime_minus_one_zero_bits =
                    Self::count_zero_bits(&prime_minus_one).map_err(|e| anyhow!("counting error {e}"))?;
                let batch_size = prime_minus_one_zero_bits;
                let masks_needed =
                    batch_size.checked_mul(state.bit_shares.len()).ok_or_else(|| anyhow!("integer overflow"))?;
                let (ran_state, messages) =
                    RandomIntegerState::new(RandomMode::RandomOfDegreeT, masks_needed, state.secret_sharer.clone())
                        .map_err(|e| anyhow!("failed to create RAN state: {e}"))?;
                let messages = messages.into_iter().map(|message| message.wrap(&RanBitwiseStateMessage::Ran)).collect();
                let next_state = states::WaitingRan {
                    ran_state_machine: StateMachine::new(ran_state),
                    secret_sharer: state.secret_sharer,
                    bit_shares: state.bit_shares,
                    batch_size,
                    masks_needed,
                    verification_shares: Batches::default(),
                };
                Ok(StateMachineStateOutput::Messages(WaitingRan(next_state), messages))
            }
            RanBitwiseMode::Sized { size: _ } => {
                let shares = state.bit_shares.into_iter().map(BitwiseNumberShares::from).collect();
                Ok(StateMachineStateOutput::Final(RanBitwiseStateOutput::Success { shares }))
            }
        }
    }

    fn transition_waiting_ran(state: states::WaitingRan<T>) -> StateMachineStateResult<Self> {
        let (ran_zero_state, messages) =
            RandomIntegerState::new(RandomMode::ZerosOfDegree2T, state.masks_needed, state.secret_sharer.clone())
                .map_err(|e| anyhow!("failed to create RAN-ZERO state: {e}"))?;
        let messages = messages.into_iter().map(|message| message.wrap(&RanBitwiseStateMessage::RanZero)).collect();
        let next_state = states::WaitingRanZero {
            ran_zero_state_machine: StateMachine::new(ran_zero_state),
            secret_sharer: state.secret_sharer,
            bit_shares: state.bit_shares,
            batch_size: state.batch_size,
            verification_shares: state.verification_shares,
            zeros: Batches::default(),
        };
        Ok(StateMachineStateOutput::Messages(WaitingRanZero(next_state), messages))
    }

    fn transition_waiting_ran_zero(state: states::WaitingRanZero<T>) -> StateMachineStateResult<Self> {
        let operands = Self::compute_verification_operands(&state.bit_shares, state.verification_shares, state.zeros)?;
        let (pub_mult_state, messages) = PubMultState::new(operands, state.secret_sharer)
            .map_err(|e| anyhow!("failed to create PUB-MULT state: {e}"))?;
        let messages = messages.into_iter().map(|message| message.wrap(&RanBitwiseStateMessage::PubMult)).collect();
        let next_state = states::WaitingPubMult {
            pub_mult_state_machine: StateMachine::new(pub_mult_state),
            bit_shares: state.bit_shares,
            verification_values: Vec::new(),
        };
        Ok(StateMachineStateOutput::Messages(WaitingPubMult(next_state), messages))
    }

    fn transition_waiting_pub_mult(state: states::WaitingPubMult<T>) -> StateMachineStateResult<Self> {
        let zero = ModularNumber::ZERO;
        if state.verification_values.into_iter().any(|value| value == zero) {
            Ok(StateMachineStateOutput::Final(RanBitwiseStateOutput::Abort))
        } else {
            let shares = state.bit_shares.into_iter().map(BitwiseNumberShares::from).collect();
            Ok(StateMachineStateOutput::Final(RanBitwiseStateOutput::Success { shares }))
        }
    }

    fn handle_message(mut state: Self, message: PartyMessage<RanBitwiseStateMessage>) -> StateMachineStateResult<Self> {
        use RanBitwiseStateMessage::*;
        let (party_id, message) = message.into_parts();
        match (message, &mut state) {
            (RanBit(message), WaitingRanBit(inner)) => {
                match inner.ran_bit_state_machine.handle_message(PartyMessage::new(party_id, message))? {
                    StateMachineOutput::Final(RandomBitStateOutput::Success { shares: bit_shares }) => {
                        let bit_shares = Batches::from_flattened_fixed(bit_shares, inner.batch_size)
                            .map_err(|e| anyhow!("failed to construct bit batches: {e}"))?;
                        inner.bit_shares = bit_shares;
                        state.try_next()
                    }
                    StateMachineOutput::Final(RandomBitStateOutput::Abort) => {
                        Ok(StateMachineStateOutput::Final(RanBitwiseStateOutput::RanBitAbort))
                    }
                    output => state.wrap_message(output, RanBitwiseStateMessage::RanBit),
                }
            }
            (Ran(message), WaitingRan(inner)) => {
                match inner.ran_state_machine.handle_message(PartyMessage::new(party_id, message))? {
                    StateMachineOutput::Final(shares) => {
                        let shares = Batches::from_flattened_fixed(shares, inner.batch_size)
                            .map_err(|e| anyhow!("failed to construct bit batches: {e}"))?;
                        inner.verification_shares = shares;
                        state.try_next()
                    }
                    output => state.wrap_message(output, RanBitwiseStateMessage::Ran),
                }
            }
            (RanZero(message), WaitingRanZero(inner)) => {
                match inner.ran_zero_state_machine.handle_message(PartyMessage::new(party_id, message))? {
                    StateMachineOutput::Final(shares) => {
                        let shares = Batches::from_flattened_fixed(shares, inner.batch_size)
                            .map_err(|e| anyhow!("failed to construct bit batches: {e}"))?;
                        inner.zeros = shares;
                        state.try_next()
                    }
                    output => state.wrap_message(output, RanBitwiseStateMessage::RanZero),
                }
            }
            (PubMult(message), WaitingPubMult(inner)) => {
                match inner.pub_mult_state_machine.handle_message(PartyMessage::new(party_id, message))? {
                    StateMachineOutput::Final(values) => {
                        inner.verification_values = values;
                        state.try_next()
                    }
                    output => state.wrap_message(output, RanBitwiseStateMessage::PubMult),
                }
            }
            (message, _) => Ok(StateMachineStateOutput::OutOfOrder(state, PartyMessage::new(party_id, message))),
        }
    }

    fn count_zero_bits(number: &T::Normal) -> Result<usize, RanBitwiseCreateError> {
        let mut value: usize = 0;
        for bit in 0..number.bits() {
            if !number.bit(bit) {
                value = value.checked_add(1).ok_or(RanBitwiseCreateError::IntegerOverflow)?;
            }
        }
        Ok(value)
    }

    fn compute_verification_operands(
        bit_shares: &Batches<BitShare<T>>,
        masks: Batches<ModularNumber<T>>,
        zeros: Batches<ModularNumber<T>>,
    ) -> Result<Vec<PubOperandShares<T>>, Error> {
        // SAFETY: modulo can't be zero so this can't underflow.
        let prime_minus_one = T::MODULO.checked_sub(&T::Normal::from(1)).unwrap();
        let zero_bits: Vec<_> = (0..prime_minus_one.bits()).filter(|&bit| !prime_minus_one.bit(bit)).collect();
        let mut operands = Vec::new();
        for ((bit_shares, masks), zeros) in bit_shares.iter().zip(masks.into_iter()).zip(zeros.into_iter()) {
            for ((&bit, mask), zero) in zero_bits.iter().zip(masks.into_iter()).zip(zeros.into_iter()) {
                let operand = Self::compute_verification_operand(bit, bit_shares, mask, zero)?;
                operands.push(operand);
            }
        }
        Ok(operands)
    }

    fn compute_verification_operand(
        bit: usize,
        bit_shares: &[BitShare<T>],
        mask: ModularNumber<T>,
        zero: ModularNumber<T>,
    ) -> Result<PubOperandShares<T>, Error> {
        let starting_bit = bit.checked_add(1).ok_or_else(|| anyhow!("integer overflow"))?;
        let bit_share = bit_shares.get(bit).ok_or_else(|| anyhow!("failed to find bit share {bit}"))?;
        let mut filter = ModularNumber::ONE - bit_share.value();
        for j in starting_bit..T::MODULO.bits() {
            let bit_share = bit_shares.get(j).ok_or_else(|| anyhow!("failed to find bit share {j}"))?;
            let xored_share = bit_share.xor_mask(T::MODULO.bit(j));
            filter = filter + &ModularNumber::from(xored_share);
        }
        Ok(PubOperandShares::single(mask, filter, zero))
    }
}

/// A message for the RANDOM-BITWISE protocol.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[repr(u8)]
pub enum RanBitwiseStateMessage {
    /// A message for the RAN-BIT state machine.
    RanBit(RandomBitStateMessage) = 0,

    /// A message for the RAN state machine.
    Ran(RandomIntegerStateMessage) = 1,

    /// A message for the RAN-ZERO state machine.
    RanZero(RandomIntegerStateMessage) = 2,

    /// A message for the PUB-MULT state machine.
    PubMult(PubMultStateMessage) = 3,
}

/// An error during the RANDOM-BITWISE creation.
#[derive(Debug, thiserror::Error)]
pub enum RanBitwiseCreateError {
    /// An error during the RAN-BIT creation.
    #[error("ran bit: {0}")]
    RanBit(#[from] RandomBitCreateError),

    /// An integer overflow error.
    #[error("integer overflow")]
    IntegerOverflow,
}

#[allow(clippy::arithmetic_side_effects, clippy::indexing_slicing)]
#[cfg(test)]
mod test {
    use super::*;
    use math_lib::modular::{U64SafePrime, UintType};

    type Prime = U64SafePrime;
    type Inner = <Prime as UintType>::Normal;
    type State = RanBitwiseState<Prime>;

    #[test]
    fn zero_bit_counting() {
        assert_eq!(State::count_zero_bits(&Inner::from(255u64)).unwrap(), 0);
        assert_eq!(State::count_zero_bits(&Inner::from(254u64)).unwrap(), 1);
        assert_eq!(State::count_zero_bits(&Inner::from(4u64)).unwrap(), 2);
    }
}

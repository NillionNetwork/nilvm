//! The LESS-THAN-ZERO protocol state machine.

use crate::{
    conditionals::less_than::quaternary::{QuatComparands, QuatLessState, QuatLessStateMessage},
    multiplication::multiplication_shares::{
        state::{MultState, MultStateMessage},
        OperandShares,
    },
    random::{
        random_bit::{BitShare, BitShareError},
        random_quaternary::QuaternaryShares,
    },
    reveal::state::{PartySecretMismatch, RevealMode, RevealState, RevealStateMessage},
};
use anyhow::anyhow;
use basic_types::{PartyId, PartyMessage};
use math_lib::modular::{AsBits, EncodedModularNumber, Modular, ModularNumber, SafePrime};
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
        multiplication::multiplication_shares::MultStateMachine, random::random_quaternary::QuaternaryShares,
        reveal::RevealStateMachine,
    };
    use math_lib::{
        fields::PrimeField,
        modular::{ModularNumber, SafePrime},
    };
    use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer};
    use std::sync::Arc;

    use crate::conditionals::less_than::quaternary::QuatLessStateMachine;

    /// We are waiting for the masked comparand REVEAL.
    pub struct WaitingReveal<T>
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        /// The REVEAL state machine.
        pub(crate) reveal_state_machine: RevealStateMachine<PrimeField<T>, ShamirSecretSharer<T>>,

        /// The secret sharer we're using.
        pub(crate) secret_sharer: Arc<ShamirSecretSharer<T>>,

        /// The random quaternary element shares.
        pub(crate) random_shares: Vec<QuaternaryShares<T>>,

        /// The revealed comparands.
        pub(crate) comparands: Vec<ModularNumber<T>>,
    }

    /// We are waiting for the bit less than.
    pub struct WaitingBitLessThan<T>
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        /// The QUATERNARY-LESS-THAN state machine.
        pub(crate) quaternary_state_machine: QuatLessStateMachine<T>,

        /// The secret sharer we're using.
        pub(crate) secret_sharer: Arc<ShamirSecretSharer<T>>,

        /// The random quaternary element shares.
        pub(crate) random_shares: Vec<QuaternaryShares<T>>,

        /// The revealed comparands.
        pub(crate) comparands: Vec<ModularNumber<T>>,

        /// The comparison outputs.
        pub(crate) comparators: Vec<ModularNumber<T>>,
    }

    /// We are waiting for the MULT.
    pub struct WaitingMult<T>
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        /// The MULT state machine.
        pub(crate) mult_state_machine: MultStateMachine<T>,

        /// The random quaternary element shares.
        pub(crate) random_shares: Vec<QuaternaryShares<T>>,

        /// The revealed comparands.
        pub(crate) comparands: Vec<ModularNumber<T>>,

        /// The multiplication outputs.
        pub(crate) products: Vec<ModularNumber<T>>,
    }
}

/// The two comparands that need to be compared.
#[derive(Clone)]
pub struct ZeroComparands<T: Modular> {
    /// The secret comparand.
    pub secret: ModularNumber<T>,

    /// The secret random quaternary number.
    pub quaternary: QuaternaryShares<T>,
}

/// The LESS-THAN-ZERO protocol state.
#[derive(StateMachineState)]
#[state_machine(
    recipient_id = "PartyId",
    input_message = "PartyMessage<LessThanZeroStateMessage>",
    output_message = "LessThanZeroStateMessage",
    final_result = "Vec<ModularNumber<T>>",
    handle_message_fn = "Self::handle_message"
)]
pub enum LessThanZeroState<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// We are waiting for the REVEAL.
    #[state_machine(submachine = "state.reveal_state_machine", transition_fn = "Self::transition_waiting_reveal")]
    WaitingReveal(states::WaitingReveal<T>),

    /// We are waiting for the QUATERNARY-LESS-THAN.
    #[state_machine(
        submachine = "state.quaternary_state_machine",
        transition_fn = "Self::transition_waiting_quaternary"
    )]
    WaitingBitLessThan(states::WaitingBitLessThan<T>),

    /// We are waiting for the MULT.
    #[state_machine(submachine = "state.mult_state_machine", transition_fn = "Self::transition_waiting_mult")]
    WaitingMult(states::WaitingMult<T>),
}

use LessThanZeroState::*;

impl<T> LessThanZeroState<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Construct a new LESS-THAN-ZERO state.
    pub fn new(
        comparands: Vec<ZeroComparands<T>>,
        secret_sharer: Arc<ShamirSecretSharer<T>>,
    ) -> Result<(Self, Vec<StateMachineMessage<Self>>), LessThanZeroError> {
        let shares = Self::build_masked_comparand_shares(&comparands)?;
        let (reveal_state, messages) = RevealState::new(RevealMode::new_all(shares), secret_sharer.clone())?;
        let random_shares = comparands.into_iter().map(|comparand| comparand.quaternary).collect();
        let state = states::WaitingReveal {
            reveal_state_machine: StateMachine::new(reveal_state),
            secret_sharer,
            random_shares,
            comparands: Vec::new(),
        };
        let messages = messages.into_iter().map(|message| message.wrap(&LessThanZeroStateMessage::Reveal)).collect();
        Ok((WaitingReveal(state), messages))
    }

    fn transition_waiting_reveal(state: states::WaitingReveal<T>) -> StateMachineStateResult<Self> {
        let comparands = Self::build_quaternary_comparands(&state.comparands, &state.random_shares);
        let (quaternary_state, messages) = QuatLessState::new(comparands, state.secret_sharer.clone())
            .map_err(|e| anyhow!("failed to create QUATERNARY-LESS-THAN state: {e}"))?;
        let state = states::WaitingBitLessThan {
            quaternary_state_machine: StateMachine::new(quaternary_state),
            secret_sharer: state.secret_sharer,
            comparands: state.comparands,
            random_shares: state.random_shares,
            comparators: Vec::new(),
        };
        let messages =
            messages.into_iter().map(|message| message.wrap(&LessThanZeroStateMessage::BitLessThan)).collect();
        Ok(StateMachineStateOutput::Messages(WaitingBitLessThan(state), messages))
    }

    fn transition_waiting_quaternary(state: states::WaitingBitLessThan<T>) -> StateMachineStateResult<Self> {
        let operands = Self::build_operands(&state.comparators, &state.random_shares)
            .map_err(|e| anyhow!("failed to build operands {e}"))?;
        let (mult_state, messages) = MultState::new(operands, state.secret_sharer.clone())
            .map_err(|e| anyhow!("failed to create MULT state: {e}"))?;
        let next_state = states::WaitingMult {
            mult_state_machine: StateMachine::new(mult_state),
            comparands: state.comparands,
            random_shares: state.random_shares,
            products: Vec::new(),
        };
        let messages = messages.into_iter().map(|message| message.wrap(&LessThanZeroStateMessage::Mult)).collect();
        Ok(StateMachineStateOutput::Messages(WaitingMult(next_state), messages))
    }

    fn transition_waiting_mult(state: states::WaitingMult<T>) -> StateMachineStateResult<Self> {
        let mut shares = Vec::new();
        for ((c, r), p) in state.comparands.iter().zip(state.random_shares.iter()).zip(state.products.iter()) {
            let r0 = r.least().map_err(|e| anyhow!("r has no elements: {e}"))?;
            let z = r0 + p;
            let c0 = c.into_value().bit(0);
            let w = BitShare::from(z).xor_mask(c0);
            shares.push(w.into());
        }
        Ok(StateMachineStateOutput::Final(shares))
    }

    fn handle_message(
        mut state: Self,
        message: PartyMessage<LessThanZeroStateMessage>,
    ) -> StateMachineStateResult<Self> {
        use LessThanZeroStateMessage::*;
        let (party_id, message) = message.into_parts();
        match (message, &mut state) {
            (Reveal(message), WaitingReveal(inner)) => {
                match inner.reveal_state_machine.handle_message(PartyMessage::new(party_id, message))? {
                    StateMachineOutput::Final(values) => {
                        inner.comparands = values;
                        state.try_next()
                    }
                    output => state.wrap_message(output, Reveal),
                }
            }
            (BitLessThan(message), WaitingBitLessThan(inner)) => {
                match inner.quaternary_state_machine.handle_message(PartyMessage::new(party_id, message))? {
                    StateMachineOutput::Final(values) => {
                        inner.comparators = values;
                        state.try_next()
                    }
                    output => state.wrap_message(output, BitLessThan),
                }
            }
            (Mult(message), WaitingMult(inner)) => {
                match inner.mult_state_machine.handle_message(PartyMessage::new(party_id, message))? {
                    StateMachineOutput::Final(values) => {
                        inner.products = values;
                        state.try_next()
                    }
                    output => state.wrap_message(output, Mult),
                }
            }
            (message, _) => Ok(StateMachineStateOutput::OutOfOrder(state, PartyMessage::new(party_id, message))),
        }
    }

    fn build_masked_comparand_shares(
        comparands: &[ZeroComparands<T>],
    ) -> Result<Vec<ModularNumber<T>>, LessThanZeroError> {
        let mut shares = Vec::new();
        for comparand in comparands {
            let two_secret = ModularNumber::two() * &comparand.secret;
            let share = two_secret + &comparand.quaternary.merge_bits();
            shares.push(share);
        }
        Ok(shares)
    }

    fn build_quaternary_comparands(
        comparands: &[ModularNumber<T>],
        random_elements: &[QuaternaryShares<T>],
    ) -> Vec<QuatComparands<T>> {
        let mut quaternary = Vec::new();
        for (c, r) in comparands.iter().zip(random_elements.iter()) {
            let comparand = QuatComparands::new(*c, r.clone());
            quaternary.push(comparand);
        }
        quaternary
    }

    fn build_operands(
        comparators: &[ModularNumber<T>],
        random_elements: &[QuaternaryShares<T>],
    ) -> Result<Vec<OperandShares<T>>, LessThanZeroError> {
        let mut operands = Vec::new();
        for (comp, r) in comparators.iter().zip(random_elements.iter()) {
            let r0 = r.least()?;
            let z = ModularNumber::ONE - &(ModularNumber::two() * r0);
            let operand = OperandShares::single(*comp, z);
            operands.push(operand);
        }
        Ok(operands)
    }
}

/// A message for the LESS-THAN-ZERO protocol.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[repr(u8)]
pub enum LessThanZeroStateMessage {
    /// A message for the masked comparand REVEAL state machine.
    Reveal(RevealStateMessage<EncodedModularNumber>) = 0,

    /// A message for the underlying QUATERNARY-LESS-THAN state machine.
    BitLessThan(QuatLessStateMessage) = 1,

    /// A message for the underlying MULT state machine.
    Mult(MultStateMessage) = 2,
}

/// An error during the LESS-THAN-ZERO state creation.
#[derive(Debug, thiserror::Error)]
pub enum LessThanZeroError {
    /// An error during the REVEAL creation.
    #[error("REVEAL: {0}")]
    Reveal(#[from] PartySecretMismatch),

    /// No elements found.
    #[error("no elements found")]
    NoElements,

    /// An bit share error.
    #[error("bit share: {0}")]
    BitShare(#[from] BitShareError),
}

#[allow(clippy::arithmetic_side_effects, clippy::indexing_slicing)]
#[cfg(test)]
mod test {
    use super::*;
    use crate::random::random_quaternary::QuatShare;
    use math_lib::modular::U64SafePrime;
    use rstest::rstest;

    type Prime = U64SafePrime;
    type State = LessThanZeroState<Prime>;

    fn quaternary_shares(secret: u64) -> QuaternaryShares<U64SafePrime> {
        let mut shares = Vec::new();
        let mut secret = secret;
        loop {
            let low = ModularNumber::from_u64(secret % 2);
            secret /= 2;
            let high = ModularNumber::from_u64(secret % 2);
            secret /= 2;
            let cross = low * &high;
            let share = QuatShare::new(low, high, cross);
            shares.push(share);
            if secret == 0 {
                break;
            }
        }
        let secret: QuaternaryShares<U64SafePrime> = QuaternaryShares::from(shares);
        secret
    }

    #[rstest]
    #[case(0, 0, 1)]
    #[case(2, 3, 18446744072637906946)]
    fn building_operands(#[case] left: u64, #[case] right: u64, #[case] expected: u64) {
        let comparators = vec![ModularNumber::from_u64(left)];
        let random_elements = vec![quaternary_shares(right)];
        let operands = State::build_operands(&comparators, &random_elements).unwrap();
        assert_eq!(ModularNumber::from_u64(left), operands[0].left[0]);
        assert_eq!(ModularNumber::from_u64(expected), operands[0].right[0]);
    }
}

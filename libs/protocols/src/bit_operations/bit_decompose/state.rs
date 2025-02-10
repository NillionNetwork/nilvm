//! The BIT-DECOMPOSE protocol state machine.

use crate::{
    bit_operations::{
        bit_adder_secret::{SecretBitAdderOperands, SecretBitAdderState, SecretBitAdderStateMessage},
        bit_less_than::{BitLessThanState, BitLessThanStateMessage, Comparands},
    },
    random::random_bitwise::BitwiseNumberShares,
    reveal::state::{PartySecretMismatch, RevealMode, RevealState, RevealStateMessage},
};
use anyhow::{anyhow, Error};
use basic_types::{PartyId, PartyMessage};
use math_lib::modular::{AsBits, EncodedModularNumber, Modular, ModularNumber, SafePrime};
use num_bigint::BigUint;
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
        bit_operations::{bit_adder_secret::SecretBitAdderStateMachine, bit_less_than::BitLessThanStateMachine},
        random::random_bitwise::BitwiseNumberShares,
        reveal::RevealStateMachine,
    };
    use math_lib::{
        fields::PrimeField,
        modular::{ModularNumber, SafePrime},
    };
    use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer};
    use std::sync::Arc;

    use super::BitDecomposeOperands;

    /// We are waiting for REVEAL.
    pub struct WaitingReveal<T>
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        /// The secret sharer.
        pub(crate) secret_sharer: Arc<ShamirSecretSharer<T>>,

        /// The REVEAL state machine.
        pub(crate) state_machine: RevealStateMachine<PrimeField<T>, ShamirSecretSharer<T>>,

        /// The revealed values.
        pub(crate) results: Vec<ModularNumber<T>>,

        /// Operands.
        pub(crate) operands: Vec<BitDecomposeOperands<T>>,
    }

    /// We are waiting for BIT-LESS-THAN.
    pub struct WaitingBitLessThan<T>
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        /// The secret sharer.
        pub(crate) secret_sharer: Arc<ShamirSecretSharer<T>>,

        /// The BIT-LESS-THAN state machine.
        pub(crate) state_machine: BitLessThanStateMachine<T>,

        /// The results of the bit less than.
        pub(crate) results: Vec<ModularNumber<T>>,

        /// Operands.
        pub(crate) operands: Vec<BitDecomposeOperands<T>>,

        /// The revealed values.
        pub(crate) revealed: Vec<ModularNumber<T>>,
    }

    /// We are waiting for SECRET-BIT-ADDER.
    pub struct WaitingSecretAdder<T>
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        /// The SECRET-BIT-ADDER state machine.
        pub(crate) state_machine: SecretBitAdderStateMachine<T>,

        /// The results of the adder.
        pub(crate) results: Vec<BitwiseNumberShares<T>>,
    }
}

/// The operands for the bit decompose.
#[derive(Clone, Debug)]
pub struct BitDecomposeOperands<T: Modular> {
    /// The number to be bit decomposed.
    pub number: ModularNumber<T>,

    /// The bitwise shared random number.
    pub bitwise: BitwiseNumberShares<T>,
}

impl<T: Modular> BitDecomposeOperands<T> {
    /// Constructs a new operand shares.
    pub fn new(number: ModularNumber<T>, bitwise: BitwiseNumberShares<T>) -> Self {
        Self { number, bitwise }
    }
}

/// The Carry Bit Decompose protocol state.
#[derive(StateMachineState)]
#[state_machine(
    recipient_id = "PartyId",
    input_message = "PartyMessage<BitDecomposeStateMessage>",
    output_message = "BitDecomposeStateMessage",
    final_result = "Vec<BitwiseNumberShares<T>>",
    handle_message_fn = "Self::handle_message"
)]
pub enum BitDecomposeState<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// We are waiting for the reveal to finish.
    #[state_machine(submachine = "state.state_machine", transition_fn = "Self::transition_waiting_reveal")]
    WaitingReveal(states::WaitingReveal<T>),

    /// We are waiting for the bit less than to finish.
    #[state_machine(submachine = "state.state_machine", transition_fn = "Self::transition_waiting_bit_less_than")]
    WaitingBitLessThan(states::WaitingBitLessThan<T>),

    /// We are waiting for the secret bit adder to finish.
    #[state_machine(submachine = "state.state_machine", transition_fn = "Self::transition_waiting_secret_adder")]
    WaitingSecretAdder(states::WaitingSecretAdder<T>),
}

use BitDecomposeState::*;

impl<T> BitDecomposeState<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Construct a new BIT-DECOMPOSE state.
    pub fn new(
        operands: Vec<BitDecomposeOperands<T>>,
        secret_sharer: Arc<ShamirSecretSharer<T>>,
    ) -> Result<(Self, Vec<StateMachineMessage<Self>>), BitDecomposeCreateError> {
        let differences: Vec<_> =
            operands.iter().map(|operand| operand.number - &operand.bitwise.merge_bits()).collect();

        let (state, messages) = RevealState::new(RevealMode::new_all(differences), secret_sharer.clone())?;
        let messages = messages.into_iter().map(|message| message.wrap(&BitDecomposeStateMessage::Reveal)).collect();

        let next_state =
            states::WaitingReveal { secret_sharer, state_machine: StateMachine::new(state), results: vec![], operands };
        Ok((Self::WaitingReveal(next_state), messages))
    }

    /// After the reveals are finished, call the bit less than.
    fn transition_waiting_reveal(state: states::WaitingReveal<T>) -> StateMachineStateResult<Self> {
        let comparands = Self::build_bit_less_than_operands(&state.results, &state.operands);

        let (less_state, messages) = BitLessThanState::new(comparands, state.secret_sharer.clone())
            .map_err(|e| anyhow!("failed to create BIT-LESS-THAN state: {e}"))?;
        let messages =
            messages.into_iter().map(|message| message.wrap(&BitDecomposeStateMessage::BitLessThan)).collect();

        let next_state = states::WaitingBitLessThan {
            secret_sharer: state.secret_sharer,
            state_machine: StateMachine::new(less_state),
            results: vec![],
            operands: state.operands,
            revealed: state.results,
        };
        Ok(StateMachineStateOutput::Messages(Self::WaitingBitLessThan(next_state), messages))
    }

    /// After the bit less than is finished, call the secret bit adder.
    fn transition_waiting_bit_less_than(state: states::WaitingBitLessThan<T>) -> StateMachineStateResult<Self> {
        let operands = Self::build_bit_adder_operands(state.results, state.operands, state.revealed)?;

        let (adder_state, messages) = SecretBitAdderState::new(operands, state.secret_sharer.clone())
            .map_err(|e| anyhow!("failed to create SECRET-BIT-ADDER state: {e}"))?;
        let messages =
            messages.into_iter().map(|message| message.wrap(&BitDecomposeStateMessage::SecretAdder)).collect();

        let next_state = states::WaitingSecretAdder { state_machine: StateMachine::new(adder_state), results: vec![] };
        Ok(StateMachineStateOutput::Messages(Self::WaitingSecretAdder(next_state), messages))
    }

    /// After the reveals are finished, call the secret bit adder.
    fn transition_waiting_secret_adder(state: states::WaitingSecretAdder<T>) -> StateMachineStateResult<Self> {
        Ok(StateMachineStateOutput::Final(state.results))
    }

    /// Build operands for the bit less than.
    fn build_bit_less_than_operands(
        results: &[ModularNumber<T>],
        operands: &[BitDecomposeOperands<T>],
    ) -> Vec<Comparands<T>> {
        results
            .iter()
            .zip(operands.iter())
            .map(|(revealed, operand)| Comparands::new(-revealed - &ModularNumber::ONE, operand.bitwise.clone()))
            .collect()
    }

    /// Build operands for the bit adder.
    fn build_bit_adder_operands(
        results: Vec<ModularNumber<T>>,
        operands: Vec<BitDecomposeOperands<T>>,
        revealed: Vec<ModularNumber<T>>,
    ) -> Result<Vec<SecretBitAdderOperands<T>>, Error> {
        // value = 2**bits - T::MODULO
        let value = ModularNumber::<T>::try_from(&(BigUint::new(vec![1]) << (T::MODULO.bits() - 1)))
            .map_err(|e| anyhow!("failed to create modular number: {e}"))?
            * &ModularNumber::two();
        Ok(results
            .iter()
            .zip(operands.iter())
            .zip(revealed.iter())
            .map(|((result, operand), revealed)| {
                Self::build_bit_adder_operand(value, result, revealed, operand.bitwise.clone())
            })
            .collect())
    }

    /// Build individual bit adder operand.
    fn build_bit_adder_operand(
        value: ModularNumber<T>,
        result: &ModularNumber<T>,
        revealed: &ModularNumber<T>,
        bitwise: BitwiseNumberShares<T>,
    ) -> SecretBitAdderOperands<T> {
        let left = (value + revealed).into_value();
        let right = revealed.into_value();
        let right_bitwise = (0..T::MODULO.bits())
            .map(|i| {
                (ModularNumber::from_u32(left.bit(i) as u32) - &ModularNumber::from_u32(right.bit(i) as u32)) * result
                    + &ModularNumber::from_u32(right.bit(i) as u32)
            })
            .collect::<Vec<_>>();
        SecretBitAdderOperands::new(bitwise, right_bitwise.into())
    }

    fn handle_message(
        mut state: Self,
        message: PartyMessage<BitDecomposeStateMessage>,
    ) -> StateMachineStateResult<Self> {
        use BitDecomposeStateMessage::*;
        let (party_id, message) = message.into_parts();
        match (message, &mut state) {
            (Reveal(message), WaitingReveal(inner)) => {
                match inner.state_machine.handle_message(PartyMessage::new(party_id, message))? {
                    StateMachineOutput::Final(values) => {
                        inner.results = values;
                        state.try_next()
                    }
                    output => state.wrap_message(output, BitDecomposeStateMessage::Reveal),
                }
            }
            (BitLessThan(message), WaitingBitLessThan(inner)) => {
                match inner.state_machine.handle_message(PartyMessage::new(party_id, message))? {
                    StateMachineOutput::Final(values) => {
                        inner.results = values;
                        state.try_next()
                    }
                    output => state.wrap_message(output, BitDecomposeStateMessage::BitLessThan),
                }
            }
            (SecretAdder(message), WaitingSecretAdder(inner)) => {
                match inner.state_machine.handle_message(PartyMessage::new(party_id, message))? {
                    StateMachineOutput::Final(values) => {
                        inner.results = values;
                        state.try_next()
                    }
                    output => state.wrap_message(output, BitDecomposeStateMessage::SecretAdder),
                }
            }
            (message, _) => Ok(StateMachineStateOutput::OutOfOrder(state, PartyMessage::new(party_id, message))),
        }
    }
}

/// A message for the BIT-DECOMPOSE protocol.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[repr(u8)]
pub enum BitDecomposeStateMessage {
    /// A message for the REVEAL state machine.
    Reveal(RevealStateMessage<EncodedModularNumber>) = 0,

    /// A message for the BIT-LESS-THAN state machine.
    BitLessThan(BitLessThanStateMessage) = 1,

    /// A message for the SECRET-BIT-ADDER state machine.
    SecretAdder(SecretBitAdderStateMessage) = 2,
}

/// An error during the BIT-DECOMPOSE state creation.
#[derive(Debug, thiserror::Error)]
pub enum BitDecomposeCreateError {
    /// Given operands are empty.
    #[error("Empty operand")]
    Empty,

    /// An error during the REVEAL creation.
    #[error("REVEAL: {0}")]
    Reveal(#[from] PartySecretMismatch),
}

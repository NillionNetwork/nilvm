//! The MIXED-BIT-ADDER protocol state machine.

use crate::{
    bit_operations::bit_adder::{BitAdderCreateError, BitAdderOperands, BitAdderState, BitAdderStateMessage},
    random::random_bitwise::BitwiseNumberShares,
};
use basic_types::{PartyId, PartyMessage};
use math_lib::modular::{AsBits, Modular, ModularNumber, SafePrime};
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
    use crate::{bit_operations::bit_adder::BitAdderStateMachine, random::random_bitwise::BitwiseNumberShares};
    use math_lib::modular::SafePrime;
    use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer};

    /// We are waiting for BIT-ADDER.
    pub struct WaitingAdder<T>
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        /// The BIT-ADDER state machine.
        pub(crate) state_machine: BitAdderStateMachine<T>,

        /// The results of the adder.
        pub(crate) results: Vec<BitwiseNumberShares<T>>,
    }
}

/// The operands for the mixed bit adder.
#[derive(Clone, Debug)]
pub struct MixedBitAdderOperands<T: Modular> {
    /// The first operand.
    pub left: ModularNumber<T>,

    /// The second operand.
    pub right: BitwiseNumberShares<T>,
}

impl<T: Modular> MixedBitAdderOperands<T> {
    /// Constructs a new operand shares.
    pub fn new(left: ModularNumber<T>, right: BitwiseNumberShares<T>) -> Self {
        Self { left, right }
    }
}

/// The Mixed Bit Adder protocol state.
#[derive(StateMachineState)]
#[state_machine(
    recipient_id = "PartyId",
    input_message = "PartyMessage<MixedBitAdderStateMessage>",
    output_message = "MixedBitAdderStateMessage",
    final_result = "Vec<BitwiseNumberShares<T>>",
    handle_message_fn = "Self::handle_message"
)]
pub enum MixedBitAdderState<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// We are waiting for the adder to finish.
    #[state_machine(submachine = "state.state_machine", transition_fn = "Self::transition_waiting_adder")]
    WaitingAdder(states::WaitingAdder<T>),
}

use MixedBitAdderState::*;

#[allow(clippy::expect_used)]
impl<T> MixedBitAdderState<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Construct a new MIXED-BIT-ADDER state.
    pub fn new(
        operands: Vec<MixedBitAdderOperands<T>>,
        secret_sharer: Arc<ShamirSecretSharer<T>>,
    ) -> Result<(Self, Vec<StateMachineMessage<Self>>), MixedBitAdderCreateError> {
        let adder_operands = Self::build_operands(operands)?;

        let (state, messages) = BitAdderState::new(adder_operands, secret_sharer)?;
        let messages = messages.into_iter().map(|message| message.wrap(&MixedBitAdderStateMessage::Adder)).collect();

        let next_state = states::WaitingAdder { state_machine: StateMachine::new(state), results: vec![] };
        Ok((Self::WaitingAdder(next_state), messages))
    }

    fn transition_waiting_adder(state: states::WaitingAdder<T>) -> StateMachineStateResult<Self> {
        Ok(StateMachineStateOutput::Final(state.results))
    }

    fn build_operands(
        operands: Vec<MixedBitAdderOperands<T>>,
    ) -> Result<Vec<BitAdderOperands<T>>, MixedBitAdderCreateError> {
        let mut adder_operands = Vec::with_capacity(operands.len());
        for operand in operands {
            if operand.right.len() != T::MODULO.bits() {
                return Err(MixedBitAdderCreateError::OperandWrongSize);
            }
            let mut number = operand.left;
            let mut bits = Vec::new();
            let mut products = Vec::new();
            for right in operand.right.shares().iter() {
                if number % &ModularNumber::two() == Ok(ModularNumber::ZERO) {
                    bits.push(ModularNumber::ZERO);
                    products.push(ModularNumber::ZERO);
                } else {
                    bits.push(ModularNumber::ONE);
                    products.push(*right.value());
                }
                number = (number >> ModularNumber::ONE).expect("Nonzero");
            }
            adder_operands.push(BitAdderOperands::new(bits.into(), operand.right, products.into()));
        }
        Ok(adder_operands)
    }

    fn handle_message(
        mut state: Self,
        message: PartyMessage<MixedBitAdderStateMessage>,
    ) -> StateMachineStateResult<Self> {
        use MixedBitAdderStateMessage::*;
        let (party_id, message) = message.into_parts();
        match (message, &mut state) {
            (Adder(message), WaitingAdder(inner)) => {
                match inner.state_machine.handle_message(PartyMessage::new(party_id, message))? {
                    StateMachineOutput::Final(values) => {
                        inner.results = values;
                        state.try_next()
                    }
                    output => state.wrap_message(output, MixedBitAdderStateMessage::Adder),
                }
            }
        }
    }
}

/// A message for the MIXED-BIT-ADDER protocol.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[repr(u8)]
pub enum MixedBitAdderStateMessage {
    /// A message for the BIT-ADDER state machine.
    Adder(BitAdderStateMessage) = 0,
}

/// An error during the MIXED-BIT-ADDER state creation.
#[derive(Debug, thiserror::Error)]
pub enum MixedBitAdderCreateError {
    /// Given operands are wrong sized.
    #[error("Operand wrong size")]
    OperandWrongSize,

    /// Adder creation failed.
    #[error(transparent)]
    BitAdderCreateError(#[from] BitAdderCreateError),
}

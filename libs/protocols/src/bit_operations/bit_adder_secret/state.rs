//! The SECRET-BIT-ADDER protocol state machine.

use crate::{
    bit_operations::bit_adder::{BitAdderOperands, BitAdderState, BitAdderStateMessage},
    multiplication::multiplication_shares::{
        state::{MultCreateError, MultState, MultStateMessage},
        OperandShares,
    },
    random::random_bitwise::BitwiseNumberShares,
};
use anyhow::anyhow;
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
    use crate::{
        bit_operations::{bit_adder::BitAdderStateMachine, bit_adder_secret::SecretBitAdderOperands},
        multiplication::multiplication_shares::MultStateMachine,
        random::random_bitwise::BitwiseNumberShares,
    };
    use math_lib::modular::{ModularNumber, SafePrime};
    use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer};
    use std::sync::Arc;

    /// We are waiting for MULT.
    pub struct WaitingMult<T>
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        /// Secret Sharer.
        pub(crate) secret_sharer: Arc<ShamirSecretSharer<T>>,

        /// The MULT state machine.
        pub(crate) mult_state_machine: MultStateMachine<T>,

        /// The results of the adder.
        pub(crate) mult_results: Vec<ModularNumber<T>>,

        /// Operands.
        pub(crate) operands: Vec<SecretBitAdderOperands<T>>,
    }

    /// We are waiting for BIT-ADDER.
    pub struct WaitingAdder<T>
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        /// The RIPPLE-CARRY-BIT-ADDER state machine.
        pub(crate) state_machine: BitAdderStateMachine<T>,

        /// The results of the adder.
        pub(crate) results: Vec<BitwiseNumberShares<T>>,
    }
}

/// The operands for the mixed bit adder.
#[derive(Clone, Debug)]
pub struct SecretBitAdderOperands<T: Modular> {
    /// The first operand.
    pub left: BitwiseNumberShares<T>,

    /// The second operand.
    pub right: BitwiseNumberShares<T>,
}

impl<T: Modular> SecretBitAdderOperands<T> {
    /// Constructs a new operand shares.
    pub fn new(left: BitwiseNumberShares<T>, right: BitwiseNumberShares<T>) -> Self {
        Self { left, right }
    }
}

/// The Secret Bit Adder protocol state.
#[derive(StateMachineState)]
#[state_machine(
    recipient_id = "PartyId",
    input_message = "PartyMessage<SecretBitAdderStateMessage>",
    output_message = "SecretBitAdderStateMessage",
    final_result = "Vec<BitwiseNumberShares<T>>",
    handle_message_fn = "Self::handle_message"
)]
pub enum SecretBitAdderState<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// We are waiting for the multiplications to finish.
    #[state_machine(submachine = "state.mult_state_machine", transition_fn = "Self::transition_waiting_mult")]
    WaitingMult(states::WaitingMult<T>),

    /// We are waiting for the adder to finish.
    #[state_machine(submachine = "state.state_machine", transition_fn = "Self::transition_waiting_adder")]
    WaitingAdder(states::WaitingAdder<T>),
}

use SecretBitAdderState::*;

#[allow(clippy::expect_used)]
impl<T> SecretBitAdderState<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Construct a new SECRET-BIT-ADDER state.
    pub fn new(
        operands: Vec<SecretBitAdderOperands<T>>,
        secret_sharer: Arc<ShamirSecretSharer<T>>,
    ) -> Result<(Self, Vec<StateMachineMessage<Self>>), SecretBitAdderCreateError> {
        let mut mult_ops = Vec::new();
        for operand in operands.iter() {
            if operand.right.len() != T::MODULO.bits() {
                return Err(SecretBitAdderCreateError::OperandWrongSize);
            }
            if operand.left.len() != T::MODULO.bits() {
                return Err(SecretBitAdderCreateError::OperandWrongSize);
            }
            for (left, right) in operand.left.shares().iter().zip(operand.right.shares().iter()) {
                mult_ops.push(OperandShares::single(*left.value(), *right.value()));
            }
        }

        let (mult_state, messages) = MultState::new(mult_ops, secret_sharer.clone())?;
        let messages = messages.into_iter().map(|message| message.wrap(&SecretBitAdderStateMessage::Mult)).collect();

        let next_state = states::WaitingMult {
            secret_sharer,
            mult_state_machine: StateMachine::new(mult_state),
            mult_results: vec![],
            operands,
        };
        Ok((Self::WaitingMult(next_state), messages))
    }

    /// After the multiplications are finished run the adder.
    fn transition_waiting_mult(state: states::WaitingMult<T>) -> StateMachineStateResult<Self> {
        let adder_operands = Self::build_bit_adder_operands(state.mult_results, state.operands);

        let (state, messages) = BitAdderState::new(adder_operands, state.secret_sharer)
            .map_err(|e| anyhow!("failed to create RIPPLE-CARRY-BIT-ADDER state: {e}"))?;
        let messages = messages.into_iter().map(|message| message.wrap(&SecretBitAdderStateMessage::Adder)).collect();

        let next_state = states::WaitingAdder { state_machine: StateMachine::new(state), results: vec![] };
        Ok(StateMachineStateOutput::Messages(Self::WaitingAdder(next_state), messages))
    }

    fn transition_waiting_adder(state: states::WaitingAdder<T>) -> StateMachineStateResult<Self> {
        Ok(StateMachineStateOutput::Final(state.results))
    }

    /// Build adder operands.
    fn build_bit_adder_operands(
        results: Vec<ModularNumber<T>>,
        operands: Vec<SecretBitAdderOperands<T>>,
    ) -> Vec<BitAdderOperands<T>> {
        let products: Vec<_> = results.chunks(T::MODULO.bits()).map(|chunk| chunk.to_vec()).collect();
        operands
            .into_iter()
            .zip(products)
            .map(|(operand, product)| BitAdderOperands::new(operand.left, operand.right, product.into()))
            .collect()
    }

    fn handle_message(
        mut state: Self,
        message: PartyMessage<SecretBitAdderStateMessage>,
    ) -> StateMachineStateResult<Self> {
        use SecretBitAdderStateMessage::*;
        let (party_id, message) = message.into_parts();
        match (message, &mut state) {
            (Mult(message), WaitingMult(inner)) => {
                match inner.mult_state_machine.handle_message(PartyMessage::new(party_id, message))? {
                    StateMachineOutput::Final(values) => {
                        inner.mult_results = values;
                        state.try_next()
                    }
                    output => state.wrap_message(output, SecretBitAdderStateMessage::Mult),
                }
            }
            (Adder(message), WaitingAdder(inner)) => {
                match inner.state_machine.handle_message(PartyMessage::new(party_id, message))? {
                    StateMachineOutput::Final(values) => {
                        inner.results = values;
                        state.try_next()
                    }
                    output => state.wrap_message(output, SecretBitAdderStateMessage::Adder),
                }
            }
            (message, _) => Ok(StateMachineStateOutput::OutOfOrder(state, PartyMessage::new(party_id, message))),
        }
    }
}

/// A message for the SECRET-BIT-ADDER protocol.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[repr(u8)]
pub enum SecretBitAdderStateMessage {
    /// A message for the MULT state machine.
    Mult(MultStateMessage) = 0,

    /// A message for the RIPPLE-CARRY-BIT-ADDER state machine.
    Adder(BitAdderStateMessage) = 1,
}

/// An error during the SECRET-BIT-ADDER state creation.
#[derive(Debug, thiserror::Error)]
pub enum SecretBitAdderCreateError {
    /// Given operands are wrong sized.
    #[error("Operand wrong size")]
    OperandWrongSize,

    /// Adder creation failed.
    #[error(transparent)]
    MultCreateError(#[from] MultCreateError),
}

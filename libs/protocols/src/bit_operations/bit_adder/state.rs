//! The BIT-ADDER protocol state machine.

use crate::{
    bit_operations::util::is_final_round,
    multiplication::multiplication_shares::{
        state::{MultCreateError, MultState, MultStateMessage},
        OperandShares,
    },
    random::random_bitwise::BitwiseNumberShares,
};
use anyhow::{anyhow, Error};
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
    use crate::{multiplication::multiplication_shares::MultStateMachine, random::random_bitwise::BitwiseNumberShares};
    use math_lib::modular::{ModularNumber, SafePrime};
    use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer};
    use std::sync::Arc;

    use super::BitAdderOperands;

    /// We are waiting for MULT.
    pub struct WaitingMult<T>
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        /// The secret sharer
        pub(crate) secret_sharer: Arc<ShamirSecretSharer<T>>,

        /// The MULT state machine.
        pub(crate) mult_state_machine: MultStateMachine<T>,

        /// The results of the multiplication.
        pub(crate) mult_results: Vec<ModularNumber<T>>,

        /// The loop index.
        pub(crate) round_id: usize,

        /// Generates.
        pub(crate) generate_bits: Vec<BitwiseNumberShares<T>>,

        /// Propagates.
        pub(crate) propagate_bits: Vec<BitwiseNumberShares<T>>,

        /// Operands.
        pub(crate) operands: Vec<BitAdderOperands<T>>,
    }
}

/// The operands for the carry bit adder.
#[derive(Clone, Debug)]
pub struct BitAdderOperands<T: Modular> {
    /// The first operand.
    pub left: BitwiseNumberShares<T>,

    /// The second operand.
    pub right: BitwiseNumberShares<T>,

    /// The multiplication of operands.
    pub product: BitwiseNumberShares<T>,
}

impl<T: Modular> BitAdderOperands<T> {
    /// Constructs a new operand shares.
    pub fn new(left: BitwiseNumberShares<T>, right: BitwiseNumberShares<T>, product: BitwiseNumberShares<T>) -> Self {
        Self { left, right, product }
    }
}

/// The Carry Bit Adder protocol state.
#[derive(StateMachineState)]
#[state_machine(
    recipient_id = "PartyId",
    input_message = "PartyMessage<BitAdderStateMessage>",
    output_message = "BitAdderStateMessage",
    final_result = "Vec<BitwiseNumberShares<T>>",
    handle_message_fn = "Self::handle_message"
)]
pub enum BitAdderState<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// We are waiting for the multiplications to finish.
    #[state_machine(submachine = "state.mult_state_machine", transition_fn = "Self::transition_waiting_mult")]
    WaitingMult(states::WaitingMult<T>),
}

use BitAdderState::*;

impl<T> BitAdderState<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Construct a new BIT-ADDER state.
    pub fn new(
        operands: Vec<BitAdderOperands<T>>,
        secret_sharer: Arc<ShamirSecretSharer<T>>,
    ) -> Result<(Self, Vec<StateMachineMessage<Self>>), BitAdderCreateError> {
        let round_id = 0;

        // Carry Look Ahead Setup
        let (generate_bits, propagate_bits) = Self::build_generate_propagate(&operands);
        let mult_operands = Self::build_operands(round_id, &generate_bits, &propagate_bits)?;

        let (mult_state, messages) = MultState::new(mult_operands, secret_sharer.clone())?;
        let messages = messages
            .into_iter()
            .map(|message| message.wrap(&|message| BitAdderStateMessage::Mult(message, round_id)))
            .collect();

        let next_state = states::WaitingMult {
            secret_sharer,
            mult_state_machine: StateMachine::new(mult_state),
            mult_results: vec![],
            round_id,
            generate_bits,
            propagate_bits,
            operands,
        };
        Ok((Self::WaitingMult(next_state), messages))
    }

    /// After the multiplications are finished, recursively multiply.
    fn transition_waiting_mult(state: states::WaitingMult<T>) -> StateMachineStateResult<Self> {
        // Carry Look Ahead Update
        let (generate_bits, propagate_bits) = Self::update_generate_propagate(
            state.round_id,
            state.generate_bits,
            state.propagate_bits,
            state.mult_results,
        )?;

        let round_id = state.round_id + 1;
        if is_final_round::<T>(round_id) {
            let results = Self::build_final_bits(generate_bits, state.operands)?;
            return Ok(StateMachineStateOutput::Final(results));
        }
        let operands = Self::build_operands(round_id, &generate_bits, &propagate_bits)
            .map_err(|e| anyhow!("could not build operands: {e}"))?;
        // Call multiplication.
        let (mult_state, messages) = MultState::new(operands, state.secret_sharer.clone())
            .map_err(|e| anyhow!("failed to create MULT state: {e}"))?;
        let messages = messages
            .into_iter()
            .map(|message| message.wrap(&|message| BitAdderStateMessage::Mult(message, round_id)))
            .collect();

        let next_state = states::WaitingMult {
            secret_sharer: state.secret_sharer,
            mult_state_machine: StateMachine::new(mult_state),
            mult_results: vec![],
            round_id,
            generate_bits,
            propagate_bits,
            operands: state.operands,
        };
        Ok(StateMachineStateOutput::Messages(Self::WaitingMult(next_state), messages))
    }

    /// Using the generates and the original bits, construct the final addition results.
    fn build_final_bits(
        generate_bits: Vec<BitwiseNumberShares<T>>,
        operands: Vec<BitAdderOperands<T>>,
    ) -> Result<Vec<BitwiseNumberShares<T>>, Error> {
        let mut all_bits = Vec::with_capacity(generate_bits.len());
        for (operand, generate) in operands.into_iter().zip(generate_bits.into_iter()) {
            let mut bits = Vec::with_capacity(generate.len());
            for (i, (left, right)) in operand.left.shares().iter().zip(operand.right.shares().iter()).enumerate() {
                let carry_out = generate.shares().get(i).ok_or(anyhow!("generate not found"))?;
                let mut bit = left.value() + right.value() - carry_out.value() - carry_out.value();
                if i != 0 {
                    let carry_in = generate.shares().get(i - 1).ok_or(anyhow!("generate not found"))?;
                    bit = bit + carry_in.value();
                }
                bits.push(bit);
            }
            all_bits.push(bits.into());
        }
        Ok(all_bits)
    }

    /// Build initial generate propagate bits. We don't need kills as they are implied.
    fn build_generate_propagate(
        operands: &[BitAdderOperands<T>],
    ) -> (Vec<BitwiseNumberShares<T>>, Vec<BitwiseNumberShares<T>>) {
        let mut generate_bits = Vec::with_capacity(operands.len());
        let mut propagate_bits = Vec::with_capacity(operands.len());
        for operand in operands.iter() {
            let mut generate = Vec::with_capacity(operand.left.len());
            let mut propagate = Vec::with_capacity(operand.left.len());
            for ((left_bit, right_bit), product_bit) in
                operand.left.shares().iter().zip(operand.right.shares().iter()).zip(operand.product.shares().iter())
            {
                // left_bit & right_bit
                generate.push(product_bit.clone());
                // left_bit ^ right_bit
                propagate.push(*left_bit.value() + right_bit.value() - product_bit.value() - product_bit.value());
            }
            generate_bits.push(generate.into());
            propagate_bits.push(propagate.into());
        }
        (generate_bits, propagate_bits)
    }

    /// Each round, update generate and propagates using the multiplication results.
    #[allow(clippy::type_complexity)]
    fn update_generate_propagate(
        round_id: usize,
        prev_generate_bits: Vec<BitwiseNumberShares<T>>,
        prev_propagate_bits: Vec<BitwiseNumberShares<T>>,
        mult_results: Vec<ModularNumber<T>>,
    ) -> Result<(Vec<BitwiseNumberShares<T>>, Vec<BitwiseNumberShares<T>>), Error> {
        let power = 1 << round_id;
        let mut generate_bits = Vec::with_capacity(prev_generate_bits.len());
        let mut propagate_bits = Vec::with_capacity(prev_propagate_bits.len());
        let mut products = mult_results.into_iter();
        for (prev_generate, prev_propagate) in prev_generate_bits.iter().zip(prev_propagate_bits.iter()) {
            let mut generate = Vec::with_capacity(prev_generate.len());
            let mut propagate = Vec::with_capacity(prev_propagate.len());
            for (i, (prev_generate_i, prev_propagate_i)) in
                prev_generate.shares().iter().zip(prev_propagate.shares().iter()).enumerate()
            {
                let level = i / power;
                if level % 2 == 0 {
                    generate.push(*prev_generate_i.value());
                    propagate.push(*prev_propagate_i.value());
                } else {
                    generate.push(prev_generate_i.value() + &products.next().ok_or(anyhow!("product not found"))?);
                    propagate.push(products.next().ok_or(anyhow!("product not found"))?);
                }
            }
            generate_bits.push(generate.into());
            propagate_bits.push(propagate.into());
        }
        Ok((generate_bits, propagate_bits))
    }

    /// Build the multiplication operands from generates and propagates.
    fn build_operands(
        round_id: usize,
        generate_bits: &[BitwiseNumberShares<T>],
        propagate_bits: &[BitwiseNumberShares<T>],
    ) -> Result<Vec<OperandShares<T>>, BitAdderCreateError> {
        let power = 1 << round_id;
        let mut operands = Vec::with_capacity(generate_bits.len() * T::MODULO.bits());
        for (generate, propagate) in generate_bits.iter().zip(propagate_bits.iter()) {
            for i in 0..generate.len() {
                let level = i / power;
                // We only multiply the terms if level is odd and carry forward if level is even.
                if level % 2 == 1 {
                    let left = generate.shares().get(power * level - 1).ok_or(BitAdderCreateError::Empty)?;
                    let right = propagate.shares().get(i).ok_or(BitAdderCreateError::Empty)?;
                    operands.push(OperandShares::single(*left.value(), *right.value()));
                    let left = propagate.shares().get(power * level - 1).ok_or(BitAdderCreateError::Empty)?;
                    operands.push(OperandShares::single(*left.value(), *right.value()));
                }
            }
        }
        Ok(operands)
    }

    fn handle_message(mut state: Self, message: PartyMessage<BitAdderStateMessage>) -> StateMachineStateResult<Self> {
        use BitAdderStateMessage::*;
        let (party_id, message) = message.into_parts();
        match (message, &mut state) {
            (Mult(message, round_id), WaitingMult(inner)) if inner.round_id == round_id => {
                match inner.mult_state_machine.handle_message(PartyMessage::new(party_id, message))? {
                    StateMachineOutput::Final(values) => {
                        inner.mult_results = values;
                        state.try_next()
                    }
                    output => state.wrap_message(output, |message| BitAdderStateMessage::Mult(message, round_id)),
                }
            }
            (message, _) => Ok(StateMachineStateOutput::OutOfOrder(state, PartyMessage::new(party_id, message))),
        }
    }
}

/// A message for the BIT-ADDER protocol.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[repr(u8)]
pub enum BitAdderStateMessage {
    /// A message for the MULT state machine.
    Mult(MultStateMessage, usize) = 0,
}

/// An error during the BIT-ADDER state creation.
#[derive(Debug, thiserror::Error)]
pub enum BitAdderCreateError {
    /// Given operands are empty.
    #[error("Empty operand")]
    Empty,

    /// Mult creation failed.
    #[error(transparent)]
    MultCreateError(#[from] MultCreateError),
}

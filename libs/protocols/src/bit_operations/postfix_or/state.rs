//! The POSTFIX-OR protocol state machine.

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
use math_lib::modular::{AsBits, ModularNumber, SafePrime};
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

        /// Postfix.
        pub(crate) postfix: Vec<BitwiseNumberShares<T>>,
    }
}

/// The Carry Bit Adder protocol state.
#[derive(StateMachineState)]
#[state_machine(
    recipient_id = "PartyId",
    input_message = "PartyMessage<PostfixOrStateMessage>",
    output_message = "PostfixOrStateMessage",
    final_result = "Vec<BitwiseNumberShares<T>>",
    handle_message_fn = "Self::handle_message"
)]
pub enum PostfixOrState<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// We are waiting for the multiplications to finish.
    #[state_machine(submachine = "state.mult_state_machine", transition_fn = "Self::transition_waiting_mult")]
    WaitingMult(states::WaitingMult<T>),
}

use PostfixOrState::*;

impl<T> PostfixOrState<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Construct a new POSTFIX-OR state.
    pub fn new(
        inputs: Vec<BitwiseNumberShares<T>>,
        secret_sharer: Arc<ShamirSecretSharer<T>>,
    ) -> Result<(Self, Vec<StateMachineMessage<Self>>), PostfixOrCreateError> {
        let round_id = 0;

        let mult_operands = Self::build_operands(round_id, &inputs)?;

        let (mult_state, messages) = MultState::new(mult_operands, secret_sharer.clone())?;
        let messages = messages
            .into_iter()
            .map(|message| message.wrap(&|message| PostfixOrStateMessage::Mult(message, round_id)))
            .collect();

        let next_state = states::WaitingMult {
            secret_sharer,
            mult_state_machine: StateMachine::new(mult_state),
            mult_results: vec![],
            round_id,
            postfix: inputs,
        };
        Ok((Self::WaitingMult(next_state), messages))
    }

    /// After the multiplications are finished, recursively multiply.
    fn transition_waiting_mult(state: states::WaitingMult<T>) -> StateMachineStateResult<Self> {
        let postfix = Self::update_postfix(state.round_id, state.postfix, state.mult_results)?;

        let round_id = state.round_id + 1;
        if is_final_round::<T>(round_id) {
            return Ok(StateMachineStateOutput::Final(postfix));
        }

        let operands =
            Self::build_operands(round_id, &postfix).map_err(|e| anyhow!("could not build operands: {e}"))?;

        // Call multiplication.
        let (mult_state, messages) = MultState::new(operands, state.secret_sharer.clone())
            .map_err(|e| anyhow!("failed to create MULT state: {e}"))?;
        let messages = messages
            .into_iter()
            .map(|message| message.wrap(&|message| PostfixOrStateMessage::Mult(message, round_id)))
            .collect();

        let next_state = states::WaitingMult {
            secret_sharer: state.secret_sharer,
            mult_state_machine: StateMachine::new(mult_state),
            mult_results: vec![],
            round_id,
            postfix,
        };
        Ok(StateMachineStateOutput::Messages(Self::WaitingMult(next_state), messages))
    }

    /// Build multiplication operands.
    fn build_operands(
        round_id: usize,
        postfix: &[BitwiseNumberShares<T>],
    ) -> Result<Vec<OperandShares<T>>, PostfixOrCreateError> {
        let power = 1 << round_id;
        let mut operands = Vec::with_capacity(postfix.len() * T::MODULO.bits() / 2);
        for bitwise in postfix.iter() {
            for (i, bit) in bitwise.shares().iter().enumerate() {
                let level = i / power;
                if level % 2 == 0 {
                    let right = bitwise.shares().get(power + power * level).ok_or(PostfixOrCreateError::Empty)?;
                    operands.push(OperandShares::single(*bit.value(), *right.value()));
                }
            }
        }
        Ok(operands)
    }

    /// Update postfix ors.
    fn update_postfix(
        round_id: usize,
        previous: Vec<BitwiseNumberShares<T>>,
        products: Vec<ModularNumber<T>>,
    ) -> Result<Vec<BitwiseNumberShares<T>>, Error> {
        let power = 1 << round_id;
        let mut products = products.into_iter();
        let postfix: Result<Vec<_>, Error> = previous
            .iter()
            .map(|prev_bitwise| {
                let bitwise: Result<Vec<_>, Error> = prev_bitwise
                    .shares()
                    .iter()
                    .enumerate()
                    .map(|(i, prev_bit)| {
                        let level = i / power;
                        if level % 2 == 1 {
                            Ok(prev_bit.clone())
                        } else {
                            let other_bit =
                                prev_bitwise.shares().get(power + power * level).ok_or(anyhow!("bit not found"))?;
                            let or = *prev_bit.value() + other_bit.value()
                                - &products.next().ok_or(anyhow!("product not found"))?;
                            Ok(or.into())
                        }
                    })
                    .collect();
                Ok(bitwise?.into())
            })
            .collect();
        postfix
    }

    fn handle_message(mut state: Self, message: PartyMessage<PostfixOrStateMessage>) -> StateMachineStateResult<Self> {
        use PostfixOrStateMessage::*;
        let (party_id, message) = message.into_parts();
        match (message, &mut state) {
            (Mult(message, round_id), WaitingMult(inner)) if inner.round_id == round_id => {
                match inner.mult_state_machine.handle_message(PartyMessage::new(party_id, message))? {
                    StateMachineOutput::Final(values) => {
                        inner.mult_results = values;
                        state.try_next()
                    }
                    output => state.wrap_message(output, |message| PostfixOrStateMessage::Mult(message, round_id)),
                }
            }
            (message, _) => Ok(StateMachineStateOutput::OutOfOrder(state, PartyMessage::new(party_id, message))),
        }
    }
}

/// A message for the POSTFIX-OR protocol.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[repr(u8)]
pub enum PostfixOrStateMessage {
    /// A message for the MULT state machine.
    Mult(MultStateMessage, usize) = 0,
}

/// An error during the POSTFIX-OR state creation.
#[derive(Debug, thiserror::Error)]
pub enum PostfixOrCreateError {
    /// Given operands are empty.
    #[error("Empty operand")]
    Empty,

    /// Mult creation failed.
    #[error(transparent)]
    MultCreateError(#[from] MultCreateError),
}

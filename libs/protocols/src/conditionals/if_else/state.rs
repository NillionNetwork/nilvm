//! The IF-ELSE protocol state machine.

use crate::multiplication::multiplication_shares::{
    state::{MultCreateError, MultState, MultStateMessage},
    OperandShares,
};
use basic_types::{PartyId, PartyMessage};
use math_lib::modular::{Modular, ModularNumber, SafePrime};
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
    use crate::multiplication::multiplication_shares::MultStateMachine;
    use math_lib::modular::{ModularNumber, SafePrime};
    use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer};

    /// We are waiting for MULT.
    pub struct WaitingMult<T>
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        /// The MULT state machine.
        pub(crate) mult_state_machine: MultStateMachine<T>,

        /// The results of the first multiplication.
        pub(crate) mult_results: Vec<ModularNumber<T>>,
    }
}

/// The operands for the if else statement.
/// result = if cond { return left } else { right }
/// result = cond * left + (1 - cond) * right
#[derive(Clone, Debug)]
pub struct IfElseOperands<T: Modular> {
    /// The conditional value.
    pub cond: ModularNumber<T>,

    /// The first operand (i.e., the assignment for the "if" case).
    pub left: ModularNumber<T>,

    /// The second operand (i.e., the assignment for the "else" case).
    pub right: ModularNumber<T>,
}

impl<T: Modular> IfElseOperands<T> {
    /// Constructs a new operand shares.
    pub fn new(cond: ModularNumber<T>, left: ModularNumber<T>, right: ModularNumber<T>) -> Self {
        Self { cond, left, right }
    }
}

/// The If Else protocol state.
#[derive(StateMachineState)]
#[state_machine(
    recipient_id = "PartyId",
    input_message = "PartyMessage<IfElseStateMessage>",
    output_message = "IfElseStateMessage",
    final_result = "Vec<ModularNumber<T>>",
    handle_message_fn = "Self::handle_message"
)]
pub enum IfElseState<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// We are waiting for the multiplications to finish.
    #[state_machine(submachine = "state.mult_state_machine", transition_fn = "Self::transition_waiting_mult")]
    WaitingMult(states::WaitingMult<T>),
}

use IfElseState::*;

impl<T> IfElseState<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Construct a new IF-ELSE state.
    /// This state performs the following computation:
    /// 1. Locally compute not_cond = 1 - operands.cond
    /// 2. Initiate operands.cond * operands.left and not_cond * operands.right
    /// 3. Transition to the next state (transition_waiting_mult)
    pub fn new(
        operands: Vec<IfElseOperands<T>>,
        secret_sharer: Arc<ShamirSecretSharer<T>>,
    ) -> Result<(Self, Vec<StateMachineMessage<Self>>), IfElseCreateError> {
        let both_mult_operands = Self::build_multiplication_operands(&operands);

        // Call both multiplications.
        let (mult_state, messages) = MultState::new(both_mult_operands, secret_sharer.clone())?;
        let messages = messages.into_iter().map(|message| message.wrap(&IfElseStateMessage::Mult)).collect();

        let next_state =
            states::WaitingMult { mult_state_machine: StateMachine::new(mult_state), mult_results: vec![] };

        Ok((Self::WaitingMult(next_state), messages))
    }

    /// Build the operands required for the multiplications. Namely:
    /// mult_ops = [ condition, 1 - condition ] * [ left_operand , right_operand ]
    fn build_multiplication_operands(if_else_operands: &[IfElseOperands<T>]) -> Vec<OperandShares<T>> {
        let mut mult_ops = Vec::new();
        for operand in if_else_operands {
            // operands.cond * operands.left + (1 - operands.cond) * operands.right
            let inv_cond = ModularNumber::ONE - &operand.cond;
            mult_ops.push(OperandShares::new(vec![operand.cond, inv_cond], vec![operand.left, operand.right]));
        }
        mult_ops
    }

    /// After the multiplications are finished, return the results:
    /// result = operands.cond * operands.left + not_cond * operands.right
    fn transition_waiting_mult(state: states::WaitingMult<T>) -> StateMachineStateResult<Self> {
        Ok(StateMachineStateOutput::Final(state.mult_results))
    }

    fn handle_message(mut state: Self, message: PartyMessage<IfElseStateMessage>) -> StateMachineStateResult<Self> {
        use IfElseStateMessage::*;
        let (party_id, message) = message.into_parts();
        match (message, &mut state) {
            (Mult(message), WaitingMult(inner)) => {
                match inner.mult_state_machine.handle_message(PartyMessage::new(party_id, message))? {
                    StateMachineOutput::Final(values) => {
                        inner.mult_results = values;
                        state.try_next()
                    }
                    output => state.wrap_message(output, IfElseStateMessage::Mult),
                }
            }
        }
    }
}

/// A message for the IF-ELSE protocol.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[repr(u8)]
pub enum IfElseStateMessage {
    /// A message for the MULT state machine.
    Mult(MultStateMessage) = 0,
}

/// An error during the IF-ELSE state creation.
#[derive(Debug, thiserror::Error)]
pub enum IfElseCreateError {
    /// Mult creation failed.
    #[error(transparent)]
    MultCreateError(#[from] MultCreateError),
}

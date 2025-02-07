//! Preprocessing for Evaluate Polynomial State Machine
//!
//! This is the Evaluate Polynomial functionality. It is used to obtain evaluate privately a polynomial.

use serde::{Deserialize, Serialize};
use std::sync::Arc;

use basic_types::PartyMessage;
use itertools::Itertools;

use state_machine::{
    sm::StateMachineOutput, state::StateMachineMessage, StateMachine, StateMachineState, StateMachineStateExt,
    StateMachineStateOutput, StateMachineStateResult,
};
use state_machine_derive::StateMachineState;

use math_lib::{
    fields::PrimeField,
    modular::{ModularNumber, SafePrime},
    polynomial::Polynomial,
};

use shamir_sharing::{
    party::PartyId,
    secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer},
};

use crate::multiplication::multiplication_public_output::{
    state::{PubMultCreateError, PubMultState, PubMultStateMessage},
    PubOperandShares,
};

use super::{
    super::offline::output::PrepPolyEvalShares,
    output::{PolyEvalShares, PolyEvalStateOutput},
};

/// The protocol states.
pub mod states {
    use crate::multiplication::multiplication_public_output::PubMultStateMachine;
    use math_lib::{
        fields::PrimeField,
        modular::{ModularNumber, SafePrime},
        polynomial::Polynomial,
    };
    use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer};
    use std::sync::Arc;

    /// Waiting for a share of a random number and its inverse
    pub struct WaitingPubMult<T>
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        /// The secret sharer
        pub secret_sharer: Arc<ShamirSecretSharer<T>>,

        /// The preprocessing elements
        pub(crate) powers: Vec<Vec<ModularNumber<T>>>,

        /// The polynomials to evaluate
        pub(crate) polynomials: Vec<Polynomial<PrimeField<T>>>,

        /// The PUB MULT state machine
        pub(crate) pub_mult_state_machine: PubMultStateMachine<T>,

        /// Polynomial clear_masked_value
        pub(crate) clear_masked_value: Vec<ModularNumber<T>>,
    }
}

/// The POLY EVAL protocol state.
#[derive(StateMachineState)]
#[state_machine(
    recipient_id = "PartyId",
    input_message = "PartyMessage<PolyEvalStateMessage>",
    output_message = "PolyEvalStateMessage",
    final_result = "PolyEvalStateOutput<PolyEvalShares<T>>",
    handle_message_fn = "Self::handle_message"
)]
pub enum PolyEvalState<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// We are waiting for the preprocessing material.
    #[state_machine(submachine = "state.pub_mult_state_machine", transition_fn = "Self::transition_waiting_pub_mult")]
    WaitingPubMult(states::WaitingPubMult<T>),
}

use PolyEvalState::*;

impl<T> PolyEvalState<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Construct a new POLY EVAL state.
    pub fn new(
        x: Vec<ModularNumber<T>>,                          // The x values
        polynomials: Vec<Polynomial<PrimeField<T>>>,       // The polynomials to evaluate on the x values
        prep_poly_eval_output: Vec<PrepPolyEvalShares<T>>, // The preprocessing for each polynomial
        secret_sharer: Arc<ShamirSecretSharer<T>>,         // The secret sharer
    ) -> Result<(Self, Vec<StateMachineMessage<Self>>), PolyEvalError> {
        // The invertible numbers are the invereses extracted from the INV RAN protocol.
        // The powers are the powers of the inverse of the invertible numbers up to a degree of the polynomial.
        // The zero shares are used for the PUB MULT protocol.
        let (invertible_numbers, powers, zero_shares) = prep_poly_eval_output
            .into_iter()
            .map(|output| (output.invertible_number, output.powers, output.zero_share))
            .multiunzip();

        // The operands are the values to be multiplied in the PUB MULT protocol.
        // These are the x values and the invertible numbers.
        // We also add the zero_shares for the PUB MULT protocol.
        let operands = Self::build_operands(x, invertible_numbers, zero_shares);
        let (pub_mult_state, messages) =
            PubMultState::new(operands, secret_sharer.clone()).map_err(PolyEvalError::PubMult)?;
        let messages = messages.into_iter().map(|message| message.wrap(&PolyEvalStateMessage::PubMult)).collect();
        let next_state = states::WaitingPubMult {
            secret_sharer,
            powers,
            polynomials,
            pub_mult_state_machine: StateMachine::new(pub_mult_state),
            clear_masked_value: Vec::new(),
        };
        Ok((WaitingPubMult(next_state), messages))
    }

    fn build_operands(
        x: Vec<ModularNumber<T>>,
        invertible_numbers: Vec<ModularNumber<T>>,
        zero_shares: Vec<ModularNumber<T>>,
    ) -> Vec<PubOperandShares<T>> {
        x.into_iter()
            .zip(invertible_numbers.into_iter().zip(zero_shares))
            .map(|(x_i, (invertible_number, zero_share))| PubOperandShares::single(x_i, invertible_number, zero_share))
            .collect()
    }

    fn transition_waiting_pub_mult(state: states::WaitingPubMult<T>) -> StateMachineStateResult<Self> {
        // Initialize pow_c and poly_x
        let mut pows_c = vec![ModularNumber::ONE; state.polynomials.len()];
        let mut polys_x = vec![ModularNumber::ZERO; state.polynomials.len()];

        for ((((pow_c, poly_x), polynomial), cmv), r_powers) in pows_c
            .iter_mut()
            .zip(polys_x.iter_mut())
            .zip(state.polynomials.iter())
            .zip(state.clear_masked_value.iter())
            .zip(state.powers.iter())
        {
            for (coeff, r_power) in polynomial.coefficients().iter().zip(r_powers.iter()) {
                let xs = *pow_c * r_power;
                *poly_x = *poly_x + &(coeff * &xs);
                *pow_c = *pow_c * cmv;
            }
        }

        let outputs = polys_x.into_iter().map(|poly_x| PolyEvalShares { poly_x }).collect::<Vec<_>>();
        let output = PolyEvalStateOutput::Success { outputs };
        Ok(StateMachineStateOutput::Final(output))
    }

    fn handle_message(mut state: Self, message: PartyMessage<PolyEvalStateMessage>) -> StateMachineStateResult<Self> {
        use PolyEvalStateMessage::*;
        let (party_id, message) = message.into_parts();
        match (message, &mut state) {
            (PubMult(message), WaitingPubMult(inner)) => {
                match inner.pub_mult_state_machine.handle_message(PartyMessage::new(party_id, message))? {
                    StateMachineOutput::Final(shares) => {
                        inner.clear_masked_value = shares;
                        state.try_next()
                    }
                    output => state.wrap_message(output, PolyEvalStateMessage::PubMult),
                }
            }
        }
    }
}

/// A message for the POLY EVAL protocol.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[repr(u8)]
pub enum PolyEvalStateMessage {
    /// A message for the underlying PUB MULT state machine.
    PubMult(PubMultStateMessage) = 0,
}

/// An error during the creation of the POLY EVAL state.
#[derive(Debug, thiserror::Error)]
pub enum PolyEvalError {
    /// An error during the creation of the PUB MULT state.
    #[error("PUB-MULT: {0}")]
    PubMult(#[from] PubMultCreateError),
}

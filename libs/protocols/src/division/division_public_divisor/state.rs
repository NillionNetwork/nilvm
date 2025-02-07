//! Integer division by public divisor protocol.
use crate::division::modulo_public_divisor::{
    offline::PrepModuloShares,
    online::state::{ModuloCreateError, ModuloShares, ModuloState, ModuloStateMessage},
};
use anyhow::anyhow;
use basic_types::PartyMessage;
use math_lib::{
    errors::DivByZero,
    modular::{ModularNumber, SafePrime},
};
use serde::{Deserialize, Serialize};
use shamir_sharing::{
    party::PartyId,
    secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer},
};
use state_machine::{
    state::StateMachineMessage, StateMachine, StateMachineOutput, StateMachineState, StateMachineStateExt,
    StateMachineStateOutput, StateMachineStateResult,
};
use state_machine_derive::StateMachineState;
use std::sync::Arc;

/// The division protocol state definitions.
pub mod states {
    use crate::division::modulo_public_divisor::ModuloStateMachine;
    use math_lib::modular::{ModularNumber, SafePrime};
    use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer};

    /// The protocol is waiting for Modulo operation.
    pub struct WaitingModulo<T>
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        /// The Modulo state machine.
        pub(crate) modulo_state_machine: ModuloStateMachine<T>,

        /// The dividends.
        pub(crate) dividends: Vec<ModularNumber<T>>,

        /// The divisors.
        pub(crate) divisors: Vec<ModularNumber<T>>,

        /// The result of the modulo protocol.
        pub(crate) remainders: Vec<ModularNumber<T>>,
    }
}

/// The input shared dividend and public divisor involved in the integer division operation.
#[derive(Clone, Debug)]
pub struct DivisionIntegerPublicDivisorShares<T>
where
    T: SafePrime,
{
    /// The shared dividend.
    pub dividend: ModularNumber<T>,

    /// The public divisor.
    pub divisor: ModularNumber<T>,

    /// The preprocessing elements need for this integer division.
    pub prep_elements: PrepModuloShares<T>,
}

/// The state machine for the division public divisor protocol.
#[derive(StateMachineState)]
#[state_machine(
    recipient_id = "PartyId",
    input_message = "PartyMessage<DivisionIntegerPublicDivisorStateMessage>",
    output_message = "DivisionIntegerPublicDivisorStateMessage",
    final_result = "Vec<ModularNumber<T>>",
    handle_message_fn = "Self::handle_message"
)]
pub enum DivisionIntegerPublicDivisorState<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// We are waiting for the Modulo operation.
    #[state_machine(submachine = "state.modulo_state_machine", transition_fn = "Self::transition_waiting_modulo")]
    WaitingModulo(states::WaitingModulo<T>),
}

use DivisionIntegerPublicDivisorState::*;

impl<T> DivisionIntegerPublicDivisorState<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Construct a new DIVISION state.
    pub fn new(
        division_elements: Vec<DivisionIntegerPublicDivisorShares<T>>,
        secret_sharer: Arc<ShamirSecretSharer<T>>,
        kappa: usize,
        k: usize,
    ) -> Result<(Self, Vec<StateMachineMessage<Self>>), DivisionCreateError> {
        // Step 1 - Calculate remainder of division using Modulo
        let modulo_shares = Self::calculate_modulo_shares(&division_elements);
        let (dividends, divisors): (Vec<ModularNumber<T>>, Vec<ModularNumber<T>>) =
            division_elements.iter().map(|div_element| (div_element.dividend, div_element.divisor)).unzip();

        let (reveal_state, messages) = ModuloState::new(modulo_shares, secret_sharer, kappa, k)?;

        let next_state = states::WaitingModulo {
            modulo_state_machine: StateMachine::new(reveal_state),
            dividends,
            divisors,
            remainders: Vec::new(),
        };
        let messages = messages
            .into_iter()
            .map(|message| message.wrap(&DivisionIntegerPublicDivisorStateMessage::Modulo))
            .collect();
        Ok((WaitingModulo(next_state), messages))
    }

    fn calculate_modulo_shares(division_elements: &[DivisionIntegerPublicDivisorShares<T>]) -> Vec<ModuloShares<T>> {
        division_elements
            .iter()
            .map(|element| ModuloShares {
                dividend: element.dividend,
                divisor: element.divisor,
                prep_elements: element.prep_elements.clone(),
            })
            .collect()
    }
    fn transition_waiting_modulo(state: states::WaitingModulo<T>) -> StateMachineStateResult<Self> {
        // Step 2 - substract remainder from dividend
        let divs_minus_remainders = Self::build_divs_minus_remainders(state.dividends, state.remainders)
            .map_err(|e| anyhow!("build dividend minus remainder failed: {e}"))?;

        // Step 3 - Perform field division
        let quotients = Self::field_division(divs_minus_remainders, state.divisors)
            .map_err(|e| anyhow!("build field division by divisior failed: {e}"))?;
        Ok(StateMachineStateOutput::Final(quotients))
    }

    fn handle_message(
        mut state: Self,
        message: PartyMessage<DivisionIntegerPublicDivisorStateMessage>,
    ) -> StateMachineStateResult<Self> {
        use DivisionIntegerPublicDivisorStateMessage::*;
        let (party_id, message) = message.into_parts();
        match (message, &mut state) {
            (Modulo(message), WaitingModulo(inner)) => {
                match inner.modulo_state_machine.handle_message(PartyMessage::new(party_id, message))? {
                    StateMachineOutput::Final(values) => {
                        inner.remainders = values;
                        state.try_next()
                    }
                    output => state.wrap_message(output, DivisionIntegerPublicDivisorStateMessage::Modulo),
                }
            }
        }
    }

    fn build_divs_minus_remainders(
        dividends: Vec<ModularNumber<T>>,
        remainders: Vec<ModularNumber<T>>,
    ) -> Result<Vec<ModularNumber<T>>, ModuloCreateError> {
        let dividend_shares = dividends.iter().zip(remainders.iter()).map(|(d, r)| d - r).collect();
        Ok(dividend_shares)
    }

    fn field_division(
        divs_minurs_remainders: Vec<ModularNumber<T>>,
        divisors: Vec<ModularNumber<T>>,
    ) -> Result<Vec<ModularNumber<T>>, DivByZero> {
        let dividend_shares = divs_minurs_remainders
            .into_iter()
            .zip(divisors.iter())
            .map(|(d, r)| d / r)
            .collect::<Result<_, DivByZero>>();
        dividend_shares
    }
}

/// A message for this state machine.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[repr(u8)]
pub enum DivisionIntegerPublicDivisorStateMessage {
    /// A message for the MODULO state machine.
    Modulo(ModuloStateMessage) = 0,
}

/// An error during the DIVISION state construction.
#[derive(thiserror::Error, Debug)]
pub enum DivisionCreateError {
    /// An error during the MODULO operation.
    #[error("MODULO: {0}")]
    Modulo(#[from] ModuloCreateError),

    /// An arithmetic error.
    #[error("arithmetic: {0}")]
    Arithmetic(#[from] DivByZero),
}

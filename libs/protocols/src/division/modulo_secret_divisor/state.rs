//! Integer Modulo by secret divisor protocol.
use crate::{
    division::division_secret_divisor::{
        offline::PrepDivisionIntegerSecretShares,
        online::state::{
            DivisionCreateError, DivisionIntegerSecretDivisorShares, DivisionIntegerSecretDivisorState,
            DivisionIntegerSecretDivisorStateMessage,
        },
    },
    multiplication::multiplication_shares::{
        state::{MultState, MultStateMessage},
        OperandShares,
    },
};
use anyhow::anyhow;
use basic_types::PartyMessage;
use itertools::Itertools;
use math_lib::modular::{ModularNumber, SafePrime};
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

/// The modulo with secret divisor protocol state definitions.
pub mod states {
    use crate::{
        division::division_secret_divisor::DivisonIntegerSecretDivisorStateMachine,
        multiplication::multiplication_shares::MultStateMachine,
    };
    use math_lib::modular::{ModularNumber, SafePrime};
    use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer};
    use std::sync::Arc;

    /// The protocol is waiting for Division operation.
    pub struct WaitingDivision<T>
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        /// The Division with secret divisor state machine.
        pub(crate) division_state_machine: DivisonIntegerSecretDivisorStateMachine<T>,

        /// The secret sharer we're using.
        pub(crate) secret_sharer: Arc<ShamirSecretSharer<T>>,

        /// The dividends.
        pub(crate) dividends: Vec<ModularNumber<T>>,

        /// The divisors.
        pub(crate) divisors: Vec<ModularNumber<T>>,

        /// The result of the division with secret protocol (quotients).
        pub(crate) quotients: Vec<ModularNumber<T>>,
    }

    /// The protocol is waiting for Mult operation.
    pub struct WaitingMult<T>
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        /// The MULT state machine.
        pub(crate) mult_state_machine: MultStateMachine<T>,

        /// The dividends.
        pub(crate) dividends: Vec<ModularNumber<T>>,

        /// The result of the multiplication between the divisor and quotionet.
        pub(crate) divisor_times_quotients: Vec<ModularNumber<T>>,
    }
}

/// The input shared dividend and shared divisor involved in the modulo operation.
#[derive(Clone, Debug)]
pub struct ModuloIntegerSecretDivisorShares<T>
where
    T: SafePrime,
{
    /// The shared dividend.
    pub dividend: ModularNumber<T>,

    /// The shared divisor.
    pub divisor: ModularNumber<T>,

    /// The preprocessing elements need for this integer secret modulo.
    pub prep_elements: PrepDivisionIntegerSecretShares<T>,
}

/// The state machine for the modulo integer secret divisor state protocol.
#[derive(StateMachineState)]
#[state_machine(
    recipient_id = "PartyId",
    input_message = "PartyMessage<ModuloIntegerSecretDivisorStateMessage>",
    output_message = "ModuloIntegerSecretDivisorStateMessage",
    final_result = "Vec<ModularNumber<T>>",
    handle_message_fn = "Self::handle_message"
)]
pub enum ModuloIntegerSecretDivisorState<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// We are waiting for the Division operation.
    #[state_machine(submachine = "state.division_state_machine", transition_fn = "Self::transition_waiting_division")]
    WaitingDivision(states::WaitingDivision<T>),

    /// We are waiting for the Mult operation.
    #[state_machine(submachine = "state.mult_state_machine", transition_fn = "Self::transition_waiting_mult")]
    WaitingMult(states::WaitingMult<T>),
}

use ModuloIntegerSecretDivisorState::*;

impl<T> ModuloIntegerSecretDivisorState<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Construct a new Modulo with secret divisor state.
    /// This state performs the following computation:
    /// 1. [WaitingDivision] Calculate integer division using [`div-int-secret`] crate: floor(a/d), where a is the dividend and d is the divisor.
    /// 2. [WaitingMult] Multiply the quotient by the divisor d using the [`multiplication`] crate: d x floor(a/d).
    /// 3. Subtract the result to dividend, a: a - d x floor(a/d).
    pub fn new(
        modulo_elements: Vec<ModuloIntegerSecretDivisorShares<T>>,
        secret_sharer: Arc<ShamirSecretSharer<T>>,
        kappa: usize,
        k: usize,
    ) -> Result<(Self, Vec<StateMachineMessage<Self>>), ModuloIntegerSecretDivisorCreateError> {
        // Step 1 - Calculate integer division using div-int-secret crate: floor(a/d)$.
        let (dividends, divisors): (Vec<ModularNumber<T>>, Vec<ModularNumber<T>>) =
            modulo_elements.iter().map(|div_element| (div_element.dividend, div_element.divisor)).unzip();
        let division_shares = Self::build_division_shares(modulo_elements);
        let (reveal_state, messages) =
            DivisionIntegerSecretDivisorState::new(division_shares, secret_sharer.clone(), kappa, k)?;
        let next_state = states::WaitingDivision {
            division_state_machine: StateMachine::new(reveal_state),
            secret_sharer,
            dividends,
            divisors,
            quotients: Vec::new(),
        };
        let messages = messages
            .into_iter()
            .map(|message| message.wrap(&ModuloIntegerSecretDivisorStateMessage::DivisionSecret))
            .collect();
        Ok((WaitingDivision(next_state), messages))
    }

    /// After the integer division by a secret divisor is finished,
    /// compute the product between the resulting quotient and the divisor.
    fn transition_waiting_division(state: states::WaitingDivision<T>) -> StateMachineStateResult<Self> {
        // Step 2 - Multiply by the divisor $d$
        let operands_divisor_times_quotient =
            Self::build_multiplication_operands(state.divisors.clone(), state.quotients.clone());
        let (mult_state, messages) = MultState::new(operands_divisor_times_quotient, state.secret_sharer.clone())
            .map_err(|e| anyhow!("failed to create MULT state: {e}"))?;
        let messages =
            messages.into_iter().map(|message| message.wrap(&ModuloIntegerSecretDivisorStateMessage::Mult)).collect();
        let next_state = states::WaitingMult {
            mult_state_machine: StateMachine::new(mult_state),
            dividends: state.dividends,
            divisor_times_quotients: vec![],
        };

        Ok(StateMachineStateOutput::Messages(WaitingMult(next_state), messages))
    }

    /// After the multiplication is finished, subtract the result to the
    /// dividend and return the result:
    /// modulos = a - d x floor(a/d)
    fn transition_waiting_mult(state: states::WaitingMult<T>) -> StateMachineStateResult<Self> {
        // Step 3 - Subtract the result to dividend $a$
        let modulos = Self::compute_subtract(state.divisor_times_quotients, state.dividends)
            .map_err(|e| anyhow!("build subtract to dividend failed: {e}"))?;

        Ok(StateMachineStateOutput::Final(modulos))
    }

    fn handle_message(
        mut state: Self,
        message: PartyMessage<ModuloIntegerSecretDivisorStateMessage>,
    ) -> StateMachineStateResult<Self> {
        use ModuloIntegerSecretDivisorStateMessage::*;
        let (party_id, message) = message.into_parts();
        match (message, &mut state) {
            (DivisionSecret(message), WaitingDivision(inner)) => {
                match inner.division_state_machine.handle_message(PartyMessage::new(party_id, message))? {
                    StateMachineOutput::Final(values) => {
                        inner.quotients = values;
                        state.try_next()
                    }
                    output => state.wrap_message(output, ModuloIntegerSecretDivisorStateMessage::DivisionSecret),
                }
            }
            (Mult(message), WaitingMult(inner)) => {
                match inner.mult_state_machine.handle_message(PartyMessage::new(party_id, message))? {
                    StateMachineOutput::Final(values) => {
                        inner.divisor_times_quotients = values;
                        state.try_next()
                    }
                    output => state.wrap_message(output, ModuloIntegerSecretDivisorStateMessage::Mult),
                }
            }
            (message, _) => Ok(StateMachineStateOutput::OutOfOrder(state, PartyMessage::new(party_id, message))),
        }
    }

    /// Builds the division shares required for the integer division with
    /// secret divisor protocol from the modulo shares.
    fn build_division_shares(
        modulo_elements: Vec<ModuloIntegerSecretDivisorShares<T>>,
    ) -> Vec<DivisionIntegerSecretDivisorShares<T>> {
        modulo_elements
            .into_iter()
            .map(|element| DivisionIntegerSecretDivisorShares {
                dividend: element.dividend,
                divisor: element.divisor,
                prep_elements: element.prep_elements,
            })
            .collect()
    }

    /// Builds the operands required for the multiplication. Namely:
    /// mult_ops = [divisor] * [quotient]
    fn build_multiplication_operands(
        divisors: Vec<ModularNumber<T>>,
        quotients: Vec<ModularNumber<T>>,
    ) -> Vec<OperandShares<T>> {
        let mut mult_ops = Vec::new();
        for (divisor, quotient) in divisors.into_iter().zip_eq(quotients.into_iter()) {
            mult_ops.push(OperandShares::single(divisor, quotient));
        }
        mult_ops
    }

    /// Computes subtraction between dividends and quotients. The output
    /// is the result of modulo with secret divisor.
    fn compute_subtract(
        divisor_times_quotients: Vec<ModularNumber<T>>,
        dividends: Vec<ModularNumber<T>>,
    ) -> Result<Vec<ModularNumber<T>>, DivisionCreateError> {
        let dividend_shares = divisor_times_quotients
            .iter()
            .zip_eq(dividends.iter())
            .map(|(divisor_times_quotient, dividend)| dividend - divisor_times_quotient)
            .collect();
        Ok(dividend_shares)
    }
}

/// A message for this state machine.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[repr(u8)]
pub enum ModuloIntegerSecretDivisorStateMessage {
    /// A message for the Division state machine.
    DivisionSecret(DivisionIntegerSecretDivisorStateMessage) = 0,
    /// A message for the MULT state machine.
    Mult(MultStateMessage) = 1,
}

/// An error during the Modulo with secret divisor state construction.
#[derive(thiserror::Error, Debug)]
pub enum ModuloIntegerSecretDivisorCreateError {
    /// An error during the Division operation.
    #[error("division with secret divisor: {0}")]
    Division(#[from] DivisionCreateError),
}

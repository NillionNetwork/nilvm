//! Share multiplication protocol for multiple shares.

use crate::{
    division::truncation_probabilistic::{
        offline::PrepTruncPrShares,
        online::state::{TruncPrShares, TruncPrState, TruncPrStateMessage},
    },
    multiplication::multiplication_shares::{
        state::{MultCreateError, MultState, MultStateMessage},
        OperandShares,
    },
};
use anyhow::anyhow;
use basic_types::PartyMessage;
use itertools::{multiunzip, multizip};
use math_lib::{
    errors::DivByZero,
    modular::{ModularNumber, SafePrime},
};
use serde::{Deserialize, Serialize};
use shamir_sharing::{
    party::PartyId,
    secret_sharer::{GenerateSharesError, SafePrimeSecretSharer, ShamirSecretSharer},
};
use state_machine::{
    state::StateMachineMessage, StateMachine, StateMachineOutput, StateMachineState, StateMachineStateExt,
    StateMachineStateOutput, StateMachineStateResult,
};
use state_machine_derive::StateMachineState;
use std::sync::Arc;

use super::state::states::WaitingTruncPr;

/// The multiplication protocol state definitions.
pub mod states {
    use crate::{
        division::truncation_probabilistic::{offline::output::PrepTruncPrShares, TruncPrStateMachine},
        multiplication::multiplication_shares::MultStateMachine,
    };
    use math_lib::modular::{ModularNumber, SafePrime};
    use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer};
    use std::sync::Arc;

    /// The protocol is waiting for MULT
    pub struct WaitingMult<T>
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        /// The secret sharer we're using.
        pub(crate) secret_sharer: Arc<ShamirSecretSharer<T>>,

        /// The MULT state machine
        pub(crate) mult_state_machine: MultStateMachine<T>,

        /// The exponent of the truncation (division by 2**exponent).
        pub(crate) exponents: Vec<ModularNumber<T>>,

        /// The products of the multiplication.
        pub(crate) products: Vec<ModularNumber<T>>,

        /// The preprocessing elements needed for TRUNCPR.
        pub prep_elements: Vec<PrepTruncPrShares<T>>,

        /// The statistical security parameter KAPPA
        pub kappa: usize,
        /// The statistical k security parameter
        pub k: usize,
    }

    /// The protocol is waiting for TRUNCPR
    pub struct WaitingTruncPr<T>
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        /// The TRUNCPR state machine
        pub(crate) truncpr_state_machine: TruncPrStateMachine<T>,

        /// The truncated values
        pub(crate) results: Vec<ModularNumber<T>>,
    }
}

/// The input shared dividend and public divisor involved in the share multiplication.
#[derive(Clone, Debug)]
pub struct MultTruncShares<T>
where
    T: SafePrime,
{
    /// The left operand.
    pub left: ModularNumber<T>,

    /// The right operand.
    pub right: ModularNumber<T>,

    /// The truncation exponent
    pub trunc_exponent: ModularNumber<T>,

    /// The preprocessing elements needed for TRUNCPR.
    pub prep_elements: PrepTruncPrShares<T>,
}

/// The state machine for the multiplication protocol.
#[derive(StateMachineState)]
#[state_machine(
    recipient_id = "PartyId",
    input_message = "PartyMessage<MultTruncStateMessage>",
    output_message = "MultTruncStateMessage",
    final_result = "Vec<ModularNumber<T>>",
    handle_message_fn = "Self::handle_message"
)]
pub enum MultTruncState<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// We are waiting for MULT state machine.
    #[state_machine(submachine = "state.mult_state_machine", transition_fn = "Self::transition_waiting_mult")]
    WaitingMult(states::WaitingMult<T>),

    /// We are waiting for TRUNCPR state machine.
    #[state_machine(submachine = "state.truncpr_state_machine", transition_fn = "Self::transition_waiting_trunc")]
    WaitingTrunc(states::WaitingTruncPr<T>),
}

use MultTruncState::*;

impl<T> MultTruncState<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Construct a new MULTIPLICATION-AND-TRUNCATION protocol state.
    pub fn new(
        mult_trunc_shares: Vec<MultTruncShares<T>>,
        secret_sharer: Arc<ShamirSecretSharer<T>>,
        kappa: usize,
        k: usize,
    ) -> Result<(Self, Vec<StateMachineMessage<Self>>), MultTruncCreateError> {
        let (left, right, prep_elements, trunc_exponents) = multiunzip(
            mult_trunc_shares
                .into_iter()
                .map(|share| (share.left, share.right, share.prep_elements, share.trunc_exponent)),
        );

        let (reveal_state, messages) = MultState::new(Self::create_operand_shares(left, right), secret_sharer.clone())?;

        let next_state = states::WaitingMult {
            secret_sharer,
            mult_state_machine: StateMachine::new(reveal_state),
            exponents: trunc_exponents,
            products: Vec::new(),
            prep_elements,
            kappa,
            k,
        };
        let messages = messages.into_iter().map(|message| message.wrap(&MultTruncStateMessage::Mult)).collect();
        Ok((WaitingMult(next_state), messages))
    }

    fn transition_waiting_mult(state: states::WaitingMult<T>) -> StateMachineStateResult<Self> {
        let (reveal_state, messages) = TruncPrState::new(
            Self::create_modulo2m_shares(state.products, state.exponents, state.prep_elements),
            state.secret_sharer,
            state.kappa,
            state.k,
        )
        .map_err(|e| anyhow!("Failed to create TRUNCPR state {e}"))?;
        let messages = messages.into_iter().map(|message| message.wrap(&MultTruncStateMessage::Trunc)).collect();
        Ok(StateMachineStateOutput::Messages(
            WaitingTrunc(WaitingTruncPr { truncpr_state_machine: StateMachine::new(reveal_state), results: vec![] }),
            messages,
        ))
    }

    fn transition_waiting_trunc(state: states::WaitingTruncPr<T>) -> StateMachineStateResult<Self> {
        Ok(StateMachineStateOutput::Final(state.results))
    }

    fn handle_message(mut state: Self, message: PartyMessage<MultTruncStateMessage>) -> StateMachineStateResult<Self> {
        use MultTruncStateMessage::*;
        let (party_id, message) = message.into_parts();
        match (message, &mut state) {
            (Mult(message), WaitingMult(inner)) => {
                match inner.mult_state_machine.handle_message(PartyMessage::new(party_id, message))? {
                    StateMachineOutput::Final(values) => {
                        inner.products = values;
                        state.try_next()
                    }
                    output => state.wrap_message(output, MultTruncStateMessage::Mult),
                }
            }
            (Trunc(message), WaitingTrunc(inner)) => {
                match inner.truncpr_state_machine.handle_message(PartyMessage::new(party_id, message))? {
                    StateMachineOutput::Final(values) => {
                        inner.results = values;
                        state.try_next()
                    }
                    output => state.wrap_message(output, MultTruncStateMessage::Trunc),
                }
            }
            (message, _) => Ok(StateMachineStateOutput::OutOfOrder(state, PartyMessage::new(party_id, message))),
        }
    }

    fn create_operand_shares(left: Vec<ModularNumber<T>>, right: Vec<ModularNumber<T>>) -> Vec<OperandShares<T>> {
        left.into_iter().zip(right).map(|(left, right)| OperandShares::new(vec![left], vec![right])).collect()
    }

    fn create_modulo2m_shares(
        products: Vec<ModularNumber<T>>,
        exponents: Vec<ModularNumber<T>>,
        prep_elements: Vec<PrepTruncPrShares<T>>,
    ) -> Vec<TruncPrShares<T>> {
        multizip((products, exponents, prep_elements))
            .map(|(product, exponent, prep_elements)| TruncPrShares {
                dividend: product,
                divisors_exp_m: exponent,
                prep_elements,
            })
            .collect()
    }
}

/// A message for this state machine.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[repr(u8)]
pub enum MultTruncStateMessage {
    /// A message for the MULT state machine
    Mult(MultStateMessage) = 0,
    /// A message for the TRUNCPR state machine.
    Trunc(TruncPrStateMessage) = 1,
}

/// An error during the MULTIPLICATION-AND-TRUNCATION state construction.
#[derive(thiserror::Error, Debug)]
pub enum MultTruncCreateError {
    /// Multiplying shares failed.
    #[error("share multiplication error: {0}")]
    Operation(#[from] DivByZero),

    /// Share generation failed.
    #[error(transparent)]
    GenerateShares(#[from] GenerateSharesError),

    /// A party id was not found.
    #[error("party id not found")]
    PartyNotFound,

    /// Length of the operands do not match.
    #[error("left.len()={0} is not equal to right.len()={1}")]
    UnequalLengthOperands(usize, usize),

    /// Error in MULT creation
    #[error("error creating the MULT state machine")]
    Mult(#[from] MultCreateError),
}

#[cfg(test)]
mod tests {
    use super::MultTruncState;
    use crate::{
        division::truncation_probabilistic::offline::PrepTruncPrShares,
        multiplication::multiplication_and_truncation::protocol::MultTruncProtocol,
    };
    use basic_types::PartyId;
    use math_lib::modular::{ModularNumber, U64SafePrime};
    use uuid::Uuid;

    #[test]
    fn test_create_truncpr_shares() {
        let one = ModularNumber::<U64SafePrime>::from_u32(1);
        let two = ModularNumber::two();
        let three = ModularNumber::<U64SafePrime>::from_u32(3);
        let four = ModularNumber::from_u32(4);
        let products = vec![one, two];
        let exponents = vec![three, four];
        let parties = vec![PartyId::from(Uuid::new_v4()), PartyId::from(Uuid::new_v4())];
        let protocol = MultTruncProtocol::new(vec![], 1, 40, 23);

        let prep_elements: Vec<PrepTruncPrShares<U64SafePrime>> =
            protocol.create_prep_truncpr_shares(&parties, 2).unwrap().get(&parties[0]).unwrap().clone();

        let mod2m_shares = MultTruncState::create_modulo2m_shares(products, exponents, prep_elements);
        assert_eq!(2, mod2m_shares.len());
    }
}

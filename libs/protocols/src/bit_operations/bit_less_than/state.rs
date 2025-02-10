//! The BIT-LESS-THAN protocol state machine.

use crate::{
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
    use super::{Comparands, Comparators};
    use crate::multiplication::multiplication_shares::MultStateMachine;
    use math_lib::modular::{ModularNumber, SafePrime};
    use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer};
    use std::sync::Arc;

    /// We are waiting for the products of comparands.
    pub struct WaitingMult<T>
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        /// The MULT state machine.
        pub(crate) mult_state_machine: MultStateMachine<T>,

        /// The secret sharer we're using.
        pub(crate) secret_sharer: Arc<ShamirSecretSharer<T>>,

        /// Comparand elements.
        pub(crate) comparands: Vec<Comparands<T>>,

        /// Equality & LessThan check elements.
        pub(crate) comparators: Vec<Comparators<T>>,

        /// The multiplication outputs.
        pub(crate) products: Vec<ModularNumber<T>>,

        /// The id to differentiate between rounds.
        pub(crate) round_id: u32,
    }
}

/// The two comparands that need to be compared.
#[derive(Clone, Debug)]
pub struct Comparands<T: Modular> {
    /// The public comparand.
    pub public: ModularNumber<T>,

    /// The secret comparand.
    pub secret: BitwiseNumberShares<T>,
}

impl<T: Modular> Comparands<T> {
    /// Creates a new comparand.
    pub fn new(public: ModularNumber<T>, secret: BitwiseNumberShares<T>) -> Comparands<T> {
        Comparands { public, secret }
    }
}

/// The equality and less comparators.
#[derive(Clone, Debug)]
pub struct Comparators<T: Modular> {
    /// The equal comparator.
    pub equal: Vec<ModularNumber<T>>,

    /// The less comparator.
    pub less: Vec<ModularNumber<T>>,
}

/// The BIT-LESS-THAN protocol state.
#[derive(StateMachineState)]
#[state_machine(
    recipient_id = "PartyId",
    input_message = "PartyMessage<BitLessThanStateMessage>",
    output_message = "BitLessThanStateMessage",
    final_result = "Vec<ModularNumber<T>>",
    handle_message_fn = "Self::handle_message"
)]
pub enum BitLessThanState<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// We are waiting for the MULT.
    #[state_machine(submachine = "state.mult_state_machine", transition_fn = "Self::transition_waiting_mult")]
    WaitingMult(states::WaitingMult<T>),
}

use BitLessThanState::*;

impl<T> BitLessThanState<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    #[allow(clippy::indexing_slicing)]
    /// Construct a new BIT-LESS-THAN state.
    pub fn new(
        comparands: Vec<Comparands<T>>,
        secret_sharer: Arc<ShamirSecretSharer<T>>,
    ) -> Result<(Self, Vec<StateMachineMessage<Self>>), BitLessError> {
        let mut operands = Vec::new();
        for comparand in comparands.iter() {
            let chunks = comparand.secret.shares().chunks_exact(2);
            for chunk in chunks {
                // SAFETY: We filtered eq to be of size 2.
                let operand = OperandShares::single(*chunk[0].value(), *chunk[1].value());
                operands.push(operand);
            }
        }
        if operands.is_empty() {
            return Err(BitLessError::NoElements);
        }
        let (mult_state, messages) = MultState::new(operands, secret_sharer.clone())?;
        let state = states::WaitingMult {
            mult_state_machine: StateMachine::new(mult_state),
            secret_sharer,
            comparands,
            comparators: Vec::new(),
            products: Vec::new(),
            round_id: 0,
        };
        let messages = messages
            .into_iter()
            .map(|message| message.wrap(&|message| BitLessThanStateMessage::Mult(message, 0)))
            .collect();
        Ok((WaitingMult(state), messages))
    }

    fn transition_waiting_mult(state: states::WaitingMult<T>) -> StateMachineStateResult<Self> {
        let comparators = if state.round_id == 0 {
            Self::build_comparators(state.comparands, state.products)?
        } else {
            Self::update_comparators(state.comparators, state.products)?
        };
        let operands = Self::build_operands(comparators.clone());

        if operands.is_empty() {
            let mut shares = Vec::new();
            for comparator in comparators {
                let share = comparator.less.first().ok_or(anyhow!("missing final element"))?;
                shares.push(*share);
            }
            Ok(StateMachineStateOutput::Final(shares))
        } else {
            let (mult_state, messages) = MultState::new(operands, state.secret_sharer.clone())
                .map_err(|e| anyhow!("failed to create MULT state: {e}"))?;
            let round_id = state.round_id.checked_add(1).ok_or_else(|| anyhow!("too many rounds"))?;
            let next_state = states::WaitingMult {
                mult_state_machine: StateMachine::new(mult_state),
                secret_sharer: state.secret_sharer,
                comparands: Vec::new(),
                comparators,
                products: Vec::new(),
                round_id,
            };
            let messages = messages
                .into_iter()
                .map(|message| message.wrap(&|message| BitLessThanStateMessage::Mult(message, round_id)))
                .collect();
            Ok(StateMachineStateOutput::Messages(WaitingMult(next_state), messages))
        }
    }

    fn handle_message(
        mut state: Self,
        message: PartyMessage<BitLessThanStateMessage>,
    ) -> StateMachineStateResult<Self> {
        use BitLessThanStateMessage::*;
        let (party_id, message) = message.into_parts();
        match (message, &mut state) {
            (Mult(message, round_id), WaitingMult(inner)) if inner.round_id == round_id => {
                match inner.mult_state_machine.handle_message(PartyMessage::new(party_id, message))? {
                    StateMachineOutput::Final(values) => {
                        inner.products = values;
                        state.try_next()
                    }
                    output => state.wrap_message(output, |message| Mult(message, round_id)),
                }
            }
            (message, _) => Ok(StateMachineStateOutput::OutOfOrder(state, PartyMessage::new(party_id, message))),
        }
    }

    #[allow(clippy::indexing_slicing)]
    fn build_comparators(
        comparands: Vec<Comparands<T>>,
        products: Vec<ModularNumber<T>>,
    ) -> Result<Vec<Comparators<T>>, Error> {
        let outer_capacity = comparands.len();
        let mut comparators = Vec::with_capacity(outer_capacity);
        let mut products = products.into_iter();
        for comparand in comparands.into_iter() {
            let inner_capacity = comparand.secret.len() / 2;
            let mut equal = Vec::with_capacity(inner_capacity);
            let mut less = Vec::with_capacity(inner_capacity);
            let public = comparand.public.into_value();
            // Iterate over secret bit pairs.
            for (i, si) in comparand.secret.shares().chunks(2).enumerate() {
                // Find corresponding public bit pair.
                let p0 = public.bit(2 * i);
                let p1 = public.bit(2 * i + 1);
                let (s0, s1, s0ands1) = match si {
                    [s0, s1] => (*s0.value(), *s1.value(), products.next().ok_or(anyhow!("product not found"))?),
                    [s0] => (*s0.value(), ModularNumber::ZERO, ModularNumber::ZERO),
                    _ => return Err(anyhow!("unexpected chunk size")),
                };
                // Whether the public bits (p1, p0) are equal to the secret bits (s1, s0).
                let eq = match (p1, p0) {
                    (false, false) => ModularNumber::ONE - &s0 - &s1 + &s0ands1,
                    (false, true) => s0 - &s0ands1,
                    (true, false) => s1 - &s0ands1,
                    (true, true) => s0ands1,
                };
                // Whether the public bits (p1, p0) are less than the secret bits (s1, s0).
                let lt = match (p1, p0) {
                    (false, false) => s0 + &s1 - &s0ands1,
                    (false, true) => s1,
                    (true, false) => s0ands1,
                    (true, true) => ModularNumber::ZERO,
                };
                equal.push(eq);
                less.push(lt);
            }
            // Push to batch.
            comparators.push(Comparators { equal, less })
        }
        Ok(comparators)
    }

    #[allow(clippy::indexing_slicing)]
    fn build_operands(comparators: Vec<Comparators<T>>) -> Vec<OperandShares<T>> {
        let mut operands = Vec::new();
        for comparator in comparators.iter() {
            let equal = comparator.equal.chunks_exact(2);
            for eq in equal.clone() {
                // SAFETY: We filtered eq to be of size 2.
                let operand = OperandShares::single(eq[0], eq[1]);
                operands.push(operand);
            }
            let less = comparator.less.chunks_exact(2);
            for (eq, lt) in equal.zip(less) {
                // SAFETY: We filtered eq and lt to be of size 2.
                let operand = OperandShares::single(eq[1], lt[0]);
                operands.push(operand);
            }
        }
        operands
    }

    #[allow(clippy::indexing_slicing)]
    fn update_comparators(
        comparators: Vec<Comparators<T>>,
        products: Vec<ModularNumber<T>>,
    ) -> Result<Vec<Comparators<T>>, Error> {
        let mut new_comparators = Vec::new();
        let mut products = products.into_iter();
        for comparator in comparators.iter() {
            let equal = comparator
                .equal
                .chunks(2)
                .map(|eq| {
                    if eq.len() == 2 {
                        products.next().ok_or(anyhow!("product not found"))
                    } else {
                        eq.first().ok_or(anyhow!("equality element not found")).copied()
                    }
                })
                .collect::<Result<Vec<_>, _>>()?;
            let less = comparator
                .less
                .chunks(2)
                .map(|lt| {
                    if lt.len() == 2 {
                        let next = products.next().ok_or(anyhow!("product not found"))?;
                        // SAFETY: We filtered lt to be of size 2.
                        Ok(lt[1] + &next)
                    } else {
                        lt.first().ok_or(anyhow!("less than element not found")).copied()
                    }
                })
                .collect::<Result<Vec<_>, _>>()?;
            new_comparators.push(Comparators { equal, less })
        }
        Ok(new_comparators)
    }
}

/// A message for the BIT-LESS-THAN protocol.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[repr(u8)]
pub enum BitLessThanStateMessage {
    /// A message for the underlying MULT state machine.
    Mult(MultStateMessage, u32) = 0,
}

/// An error during the BIT-LESS-THAN state creation.
#[derive(Debug, thiserror::Error)]
pub enum BitLessError {
    /// An error during the MULT creation.
    #[error("MULT: {0}")]
    Mult(#[from] MultCreateError),

    /// No elements found.
    #[error("no elements found")]
    NoElements,
}

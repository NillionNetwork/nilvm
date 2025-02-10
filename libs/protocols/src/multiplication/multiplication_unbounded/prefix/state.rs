//! PREP-PREFIX-MULT protocol.

use crate::{
    multiplication::multiplication_shares::{
        state::{MultState, MultStateMessage},
        OperandShares,
    },
    random::random_invertible::state::{
        InvRanError, InvRanState, InvRanStateMessage, InvRanStateOutput, InvertibleElement,
    },
};
use anyhow::{anyhow, Error};
use basic_types::{Batches, PartyId, PartyMessage};
use math_lib::modular::{EncodedModularNumber, Modular, ModularNumber, SafePrime};
use serde::{Deserialize, Serialize};
use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer};
use state_machine::{
    sm::StateMachineOutput, state::StateMachineMessage, StateMachine, StateMachineState, StateMachineStateExt,
    StateMachineStateOutput, StateMachineStateResult,
};
use state_machine_derive::StateMachineState;
use std::sync::Arc;

/// The protocol states.
pub mod states {
    use crate::{
        multiplication::multiplication_shares::MultStateMachine,
        random::random_invertible::{state::InvertibleElement, InvRanStateMachine},
    };
    use basic_types::Batches;
    use math_lib::modular::{ModularNumber, SafePrime};
    use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer};
    use std::sync::Arc;

    /// We are waiting for INV-RAN.
    pub struct WaitingInvRan<T: SafePrime>
    where
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        /// The INV-RAN state machine.
        pub(crate) inv_ran_state_machine: InvRanStateMachine<T>,

        /// The secret sharer to be used.
        pub(crate) secret_sharer: Arc<ShamirSecretSharer<T>>,

        /// The batch size.
        pub(crate) batch_size: usize,

        /// The invertibles produced by INV-RAN.
        pub(crate) invertible_shares: Batches<InvertibleElement<T>>,
    }

    /// We are waiting for MULT.
    pub struct WaitingMult<T: SafePrime>
    where
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        /// The MULT state machine.
        pub(crate) mult_state_machine: MultStateMachine<T>,

        /// The invertibles produced by INV-RAN.
        pub(crate) invertible_shares: Batches<InvertibleElement<T>>,

        /// The batch size.
        pub(crate) batch_size: usize,

        /// The product output produced by MULT.
        pub(crate) product_shares: Batches<ModularNumber<T>>,
    }
}

/// The tuple produced by PREFIX-MULT-PRE.
#[derive(Clone, Debug)]
pub struct PrefixMultTuple<T: Modular> {
    /// The mask share component.
    pub mask: ModularNumber<T>,

    /// The rolling mask for prefix elements.
    pub domino: ModularNumber<T>,
}

/// An encoded version of a `PrefixMultTuple`.
#[derive(Clone, Debug)]
pub struct EncodedPrefixMultTuple {
    /// The encoded mask.
    pub mask: EncodedModularNumber,

    /// The encoded domino.
    pub domino: EncodedModularNumber,
}

/// The output of the PREFIX-MULT-PRE protocol.
pub enum PrepPrefixMultStateOutput<T: Modular> {
    /// The protocol was successful.
    Success {
        /// The output shares.
        shares: Batches<PrefixMultTuple<T>>,
    },

    /// INV-RAN aborted.
    InvRanAbort,
}

/// The PREFIX-MULT prepare protocol state.
#[derive(StateMachineState)]
#[state_machine(
    recipient_id = "PartyId",
    input_message = "PartyMessage<PrepPrefixMultStateMessage>",
    output_message = "PrepPrefixMultStateMessage",
    final_result = "PrepPrefixMultStateOutput<T>",
    handle_message_fn = "Self::handle_message"
)]
pub enum PrepPrefixMultState<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// We are waiting for INV-RAN.
    #[state_machine(submachine = "state.inv_ran_state_machine", transition_fn = "Self::transition_waiting_inv_ran")]
    WaitingInvRan(states::WaitingInvRan<T>),

    /// We are waiting for MULT.
    #[state_machine(submachine = "state.mult_state_machine", transition_fn = "Self::transition_waiting_mult")]
    WaitingMult(states::WaitingMult<T>),
}

use PrepPrefixMultState::*;

impl<T> PrepPrefixMultState<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Constructs a new PREFIX-MULT-PRE state.
    pub fn new(
        batch_count: usize,
        batch_size: usize,
        secret_sharer: Arc<ShamirSecretSharer<T>>,
    ) -> Result<(Self, Vec<StateMachineMessage<Self>>), PrepPrefixMultCreateError> {
        if batch_size <= 1 {
            return Err(PrepPrefixMultCreateError::TooFewElements);
        }
        let invertible_count = batch_size.checked_mul(batch_count).ok_or(PrepPrefixMultCreateError::IntegerOverflow)?;
        let (inv_ran_state, messages) = InvRanState::new(invertible_count, secret_sharer.clone())?;
        let messages = messages.into_iter().map(|message| message.wrap(&PrepPrefixMultStateMessage::InvRan)).collect();
        let state = states::WaitingInvRan {
            secret_sharer,
            batch_size,
            inv_ran_state_machine: StateMachine::new(inv_ran_state),
            invertible_shares: Batches::default(),
        };
        Ok((WaitingInvRan(state), messages))
    }

    fn transition_waiting_inv_ran(state: states::WaitingInvRan<T>) -> StateMachineStateResult<Self> {
        let operands = Self::build_operands(&state.invertible_shares);
        let (mult_state, messages) =
            MultState::new(operands, state.secret_sharer).map_err(|e| anyhow!("failed to create MULT state: {e}"))?;
        // We only have batch_size - 1 MULT outputs here.
        let batch_size = state.batch_size.checked_sub(1).ok_or_else(|| anyhow!("integer underflow"))?;
        let messages = messages.into_iter().map(|message| message.wrap(&PrepPrefixMultStateMessage::Mult)).collect();
        let next_state = states::WaitingMult {
            invertible_shares: state.invertible_shares,
            mult_state_machine: StateMachine::new(mult_state),
            product_shares: Batches::default(),
            batch_size,
        };
        Ok(StateMachineStateOutput::Messages(WaitingMult(next_state), messages))
    }

    fn transition_waiting_mult(state: states::WaitingMult<T>) -> StateMachineStateResult<Self> {
        let shares = Self::build_tuples(state.invertible_shares, state.product_shares)?;
        let output = PrepPrefixMultStateOutput::Success { shares };
        Ok(StateMachineStateOutput::Final(output))
    }

    fn handle_message(
        mut state: Self,
        message: PartyMessage<PrepPrefixMultStateMessage>,
    ) -> StateMachineStateResult<Self> {
        use PrepPrefixMultStateMessage::*;
        let (party_id, message) = message.into_parts();
        match (message, &mut state) {
            (InvRan(message), WaitingInvRan(inner)) => {
                match inner.inv_ran_state_machine.handle_message(PartyMessage::new(party_id, message))? {
                    StateMachineOutput::Final(InvRanStateOutput::Success { elements }) => {
                        let elements = Batches::from_flattened_fixed(elements, inner.batch_size)
                            .map_err(|e| anyhow!("batch construction failed: {e}"))?;
                        inner.invertible_shares = elements;
                        state.try_next()
                    }
                    StateMachineOutput::Final(_) => {
                        Ok(StateMachineStateOutput::Final(PrepPrefixMultStateOutput::InvRanAbort))
                    }
                    output => state.wrap_message(output, PrepPrefixMultStateMessage::InvRan),
                }
            }
            (Mult(message), WaitingMult(inner)) => {
                match inner.mult_state_machine.handle_message(PartyMessage::new(party_id, message))? {
                    StateMachineOutput::Final(elements) => {
                        let elements = Batches::from_flattened_fixed(elements, inner.batch_size)
                            .map_err(|e| anyhow!("batch construction failed: {e}"))?;
                        inner.product_shares = elements;
                        state.try_next()
                    }
                    output => state.wrap_message(output, PrepPrefixMultStateMessage::Mult),
                }
            }
            (message, _) => Ok(StateMachineStateOutput::OutOfOrder(state, PartyMessage::new(party_id, message))),
        }
    }

    #[allow(clippy::indexing_slicing)]
    fn build_operands(invertibles: &Batches<InvertibleElement<T>>) -> Vec<OperandShares<T>> {
        let mut operands = Vec::new();
        for invertible_batch in invertibles.iter() {
            for invertibles in invertible_batch.windows(2) {
                // SAFETY: the contract of `windows` ensures there's exactly 2 elements in this slice.
                let left = invertibles[1].inverse;
                let right = invertibles[0].element;
                operands.push(OperandShares::single(left, right));
            }
        }
        operands
    }

    fn build_tuples(
        invertibles: Batches<InvertibleElement<T>>,
        product_shares: Batches<ModularNumber<T>>,
    ) -> Result<Batches<PrefixMultTuple<T>>, Error> {
        let mut batches = Batches::default();
        for (invertibles, product_shares) in invertibles.into_iter().zip(product_shares.into_iter()) {
            let first_domino = invertibles.first().ok_or_else(|| anyhow!("no invertibles provided"))?.inverse;
            let invertibles = invertibles.into_iter();
            let dominos = std::iter::once(first_domino).chain(product_shares.into_iter());
            let mut tuples = Vec::new();
            for (invertible, domino) in invertibles.into_iter().zip(dominos) {
                tuples.push(PrefixMultTuple { mask: invertible.element, domino });
            }
            batches.push(tuples);
        }
        Ok(batches)
    }
}

/// A message for the RANDOM-BITWISE protocol.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[repr(u8)]
pub enum PrepPrefixMultStateMessage {
    /// A message for the INV-RAN state machine.
    InvRan(InvRanStateMessage) = 0,

    /// A message for the Mult state machine.
    Mult(MultStateMessage) = 1,
}

/// An error during the creation of the state.
#[derive(Debug, thiserror::Error)]
pub enum PrepPrefixMultCreateError {
    /// An error during the INV-RAN state machine creation.
    #[error("INV-RAN failed: {0}")]
    InvRan(#[from] InvRanError),

    /// An integer overflow.
    #[error("integer overflow")]
    IntegerOverflow,

    /// Too few elements were requested.
    #[error("too few elements requested")]
    TooFewElements,
}

#[allow(clippy::arithmetic_side_effects, clippy::indexing_slicing)]
#[cfg(test)]
mod test {
    use math_lib::modular::U64SafePrime;

    use super::*;

    type Prime = U64SafePrime;
    type State = PrepPrefixMultState<Prime>;

    #[test]
    fn operand_building() {
        let invertibles = vec![
            InvertibleElement::new(ModularNumber::ONE, ModularNumber::from_u32(10)),
            InvertibleElement::new(ModularNumber::two(), ModularNumber::from_u32(20)),
            InvertibleElement::new(ModularNumber::from_u32(3), ModularNumber::from_u32(30)),
            InvertibleElement::new(ModularNumber::from_u32(4), ModularNumber::from_u32(40)),
        ];
        let operands = State::build_operands(&Batches::single(invertibles.clone()));
        assert_eq!(operands.len(), 3);
        assert_eq!(operands[0].left[0], invertibles[1].inverse);
        assert_eq!(operands[0].right[0], invertibles[0].element);
        assert_eq!(operands[1].left[0], invertibles[2].inverse);
        assert_eq!(operands[1].right[0], invertibles[1].element);
        assert_eq!(operands[2].left[0], invertibles[3].inverse);
        assert_eq!(operands[2].right[0], invertibles[2].element);
    }

    #[test]
    fn tuple_building() {
        let invertibles = vec![
            InvertibleElement::new(ModularNumber::ONE, ModularNumber::from_u32(10)),
            InvertibleElement::new(ModularNumber::two(), ModularNumber::from_u32(20)),
            InvertibleElement::new(ModularNumber::from_u32(3), ModularNumber::from_u32(30)),
        ];
        let products = vec![ModularNumber::from_u32(100), ModularNumber::from_u32(200)];
        let batches = State::build_tuples(Batches::single(invertibles.clone()), Batches::single(products.clone()))
            .expect("building tuples failed");
        assert_eq!(batches.len(), 1);

        let tuples = &batches[0];
        assert_eq!(tuples.len(), 3);
        assert_eq!(tuples[0].mask, invertibles[0].element);
        assert_eq!(tuples[0].domino, invertibles[0].inverse);
        assert_eq!(tuples[1].mask, invertibles[1].element);
        assert_eq!(tuples[1].domino, products[0]);
        assert_eq!(tuples[2].mask, invertibles[2].element);
        assert_eq!(tuples[2].domino, products[1]);
    }
}

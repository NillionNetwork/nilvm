//! The QUATERNARY-LESS-THAN protocol state machine.

use crate::{
    multiplication::multiplication_shares::{
        state::{MultCreateError, MultState, MultStateMessage},
        OperandShares,
    },
    random::random_quaternary::QuaternaryShares,
};
use anyhow::anyhow;
use basic_types::{Batches, PartyId, PartyMessage};
use math_lib::{
    errors::DivByZero,
    modular::{AsBits, Modular, ModularNumber, SafePrime},
};
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
    use super::QuatComparators;
    use crate::multiplication::multiplication_shares::MultStateMachine;
    use math_lib::modular::{ModularNumber, SafePrime};
    use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer};
    use std::sync::Arc;

    /// We are waiting for the products of quaternary comparands.
    pub struct WaitingMult<T>
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        /// The MULT state machine.
        pub(crate) mult_state_machine: MultStateMachine<T>,

        /// The secret sharer we're using.
        pub(crate) secret_sharer: Arc<ShamirSecretSharer<T>>,

        /// Equality & LessThan check elements.
        pub(crate) comparators: QuatComparators<T>,

        /// The multiplication outputs.
        pub(crate) products: Vec<ModularNumber<T>>,

        /// The id to differentiate between rounds.
        pub(crate) round_id: u32,
    }
}

/// The two comparands that need to be compared.
#[derive(Clone)]
pub struct QuatComparands<T: Modular> {
    /// The public comparand.
    pub public: ModularNumber<T>,

    /// The secret comparand.
    pub secret: QuaternaryShares<T>,
}

impl<T: Modular> QuatComparands<T> {
    /// Creates a new quaternary comparand.
    pub fn new(public: ModularNumber<T>, secret: QuaternaryShares<T>) -> QuatComparands<T> {
        QuatComparands { public, secret }
    }
}

/// The equality and less comparators.
#[derive(Clone, Debug)]
pub struct QuatComparators<T: Modular> {
    /// The equal comparator.
    pub equal: Batches<ModularNumber<T>>,

    /// The less comparator.
    pub less: Batches<ModularNumber<T>>,
}

/// The QUATERNARY-LESS-THAN protocol state.
#[derive(StateMachineState)]
#[state_machine(
    recipient_id = "PartyId",
    input_message = "PartyMessage<QuatLessStateMessage>",
    output_message = "QuatLessStateMessage",
    final_result = "Vec<ModularNumber<T>>",
    handle_message_fn = "Self::handle_message"
)]
pub enum QuatLessState<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// We are waiting for the MULT.
    #[state_machine(submachine = "state.mult_state_machine", transition_fn = "Self::transition_waiting_mult")]
    WaitingMult(states::WaitingMult<T>),
}

use QuatLessState::*;

impl<T> QuatLessState<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Construct a new QUATERNARY-LESS-THAN state.
    pub fn new(
        comparands: Vec<QuatComparands<T>>,
        secret_sharer: Arc<ShamirSecretSharer<T>>,
    ) -> Result<(Self, Vec<StateMachineMessage<Self>>), QuatLessError> {
        let comparators = Self::build_comparators(&comparands)?;
        let operands = Self::build_operands(&comparators);
        if operands.is_empty() {
            return Err(QuatLessError::NoElements);
        }
        let (mult_state, messages) = MultState::new(operands, secret_sharer.clone())?;
        let state = states::WaitingMult {
            mult_state_machine: StateMachine::new(mult_state),
            secret_sharer,
            comparators,
            products: Vec::new(),
            round_id: 0,
        };
        let messages = messages
            .into_iter()
            .map(|message| message.wrap(&|message| QuatLessStateMessage::Mult(message, 0)))
            .collect();
        Ok((WaitingMult(state), messages))
    }

    fn transition_waiting_mult(state: states::WaitingMult<T>) -> StateMachineStateResult<Self> {
        let comparators = Self::update_comparators(&state.comparators, state.products)
            .map_err(|e| anyhow!("couldn't update comparands {e}"))?;
        let operands = Self::build_operands(&comparators);
        if operands.is_empty() {
            let shares = comparators.less.flatten();
            Ok(StateMachineStateOutput::Final(shares))
        } else {
            let (mult_state, messages) = MultState::new(operands, state.secret_sharer.clone())
                .map_err(|e| anyhow!("failed to create MULT state: {e}"))?;
            let round_id = state.round_id.checked_add(1).ok_or_else(|| anyhow!("too many rounds"))?;
            let next_state = states::WaitingMult {
                mult_state_machine: StateMachine::new(mult_state),
                secret_sharer: state.secret_sharer,
                comparators,
                products: Vec::new(),
                round_id,
            };
            let messages = messages
                .into_iter()
                .map(|message| message.wrap(&|message| QuatLessStateMessage::Mult(message, round_id)))
                .collect();
            Ok(StateMachineStateOutput::Messages(WaitingMult(next_state), messages))
        }
    }

    fn handle_message(mut state: Self, message: PartyMessage<QuatLessStateMessage>) -> StateMachineStateResult<Self> {
        use QuatLessStateMessage::*;
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

    fn build_comparators(comparands: &Vec<QuatComparands<T>>) -> Result<QuatComparators<T>, QuatLessError> {
        let mut equals = Vec::new();
        let mut lesses = Vec::new();
        for comparand in comparands {
            let mut equal = Vec::new();
            let mut less = Vec::new();
            let r = comparand.secret.shares();
            let c = comparand.public.into_value();
            for (q, rq) in r.iter().enumerate() {
                let two_q = q.checked_mul(2).ok_or(QuatLessError::IntegerOverflow)?;
                let two_q_plus_1 = two_q.checked_add(1).ok_or(QuatLessError::IntegerOverflow)?;
                let c0 = c.bit(two_q);
                let c1 = c.bit(two_q_plus_1);
                let (r0, r1, rr) = rq.as_parts();
                let eq = match (c1, c0) {
                    (false, false) => ModularNumber::ONE - r0 - r1 + rr,
                    (false, true) => *r0 - rr,
                    (true, false) => *r1 - rr,
                    (true, true) => *rr,
                };
                let lt = match (c1, c0) {
                    (false, false) => *r0 + r1 - rr,
                    (false, true) => *r1,
                    (true, false) => *rr,
                    (true, true) => ModularNumber::ZERO,
                };
                equal.push(eq);
                less.push(lt);
            }
            // Push to batch.
            equals.push(equal);
            lesses.push(less);
        }
        Ok(QuatComparators { equal: equals.into(), less: lesses.into() })
    }

    #[allow(clippy::indexing_slicing)]
    fn build_operands(comparators: &QuatComparators<T>) -> Vec<OperandShares<T>> {
        let mut operands = Vec::new();
        for (equal, less) in comparators.equal.iter().zip(comparators.less.iter()) {
            let eqc = equal.chunks(2).filter(|c| c.len() == 2);
            for eq in eqc {
                // SAFETY: We filtered eq to be of size 2.
                let operand = OperandShares::single(eq[0], eq[1]);
                operands.push(operand);
            }
            let eqc = equal.chunks(2).filter(|c| c.len() == 2);
            let ltc = less.chunks(2).filter(|c| c.len() == 2);
            for (eq, lt) in eqc.zip(ltc) {
                // SAFETY: We filtered eq and lt to be of size 2.
                let operand = OperandShares::single(eq[1], lt[0]);
                operands.push(operand);
            }
        }
        operands
    }

    #[allow(clippy::indexing_slicing)]
    fn update_comparators(
        comparators: &QuatComparators<T>,
        products: Vec<ModularNumber<T>>,
    ) -> Result<QuatComparators<T>, QuatLessError> {
        let mut new_equals = Vec::new();
        let mut new_lesses = Vec::new();
        let mut products = products.into_iter();
        for (equal, less) in comparators.equal.iter().zip(comparators.less.iter()) {
            let eqc = equal.chunks(2);
            let ltc = less.chunks(2);
            let mut new_equal = Vec::new();
            for eq in eqc {
                if eq.len() == 2 {
                    let next = products.next().ok_or(QuatLessError::NoElements)?;
                    new_equal.push(next)
                } else {
                    new_equal.push(*eq.first().ok_or(QuatLessError::NoElements)?);
                }
            }
            let mut new_less = Vec::new();
            for lt in ltc {
                if lt.len() == 2 {
                    let next = products.next().ok_or(QuatLessError::NoElements)?;
                    // SAFETY: We filtered lt to be of size 2.
                    let next = lt[1] + &next;
                    new_less.push(next)
                } else {
                    new_less.push(*lt.first().ok_or(QuatLessError::NoElements)?);
                }
            }
            new_equals.push(new_equal);
            new_lesses.push(new_less);
        }
        Ok(QuatComparators { equal: new_equals.into(), less: new_lesses.into() })
    }
}

/// A message for the QUATERNARY-LESS-THAN protocol.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[repr(u8)]
pub enum QuatLessStateMessage {
    /// A message for the underlying MULT state machine.
    Mult(MultStateMessage, u32) = 0,
}

/// An error during the QUATERNARY-LESS-THAN state creation.
#[derive(Debug, thiserror::Error)]
pub enum QuatLessError {
    /// An error during the MULT creation.
    #[error("MULT: {0}")]
    Mult(#[from] MultCreateError),

    /// No elements found.
    #[error("no elements found")]
    NoElements,

    /// Integer overflow.
    #[error("integer overflow")]
    IntegerOverflow,

    /// An arithmetic error.
    #[error("arithmetic: {0}")]
    Arithmetic(#[from] DivByZero),
}

#[allow(clippy::arithmetic_side_effects, clippy::indexing_slicing)]
#[cfg(test)]
mod test {
    use super::*;
    use crate::random::random_quaternary::QuatShare;
    use math_lib::modular::U64SafePrime;
    use rstest::rstest;

    type Prime = U64SafePrime;
    type State = QuatLessState<Prime>;

    fn quaternary_shares(secret: u64) -> QuaternaryShares<U64SafePrime> {
        let mut shares = Vec::new();
        let mut secret = secret;
        loop {
            let low = ModularNumber::from_u64(secret % 2);
            secret /= 2;
            let high = ModularNumber::from_u64(secret % 2);
            secret /= 2;
            let cross = low * &high;
            let share = QuatShare::new(low, high, cross);
            shares.push(share);
            if secret == 0 {
                break;
            }
        }
        let secret: QuaternaryShares<U64SafePrime> = QuaternaryShares::from(shares);
        secret
    }

    #[rstest]
    #[case(0, 0)]
    #[case(0, 1)]
    #[case(0, 2)]
    #[case(0, 3)]
    #[case(1, 0)]
    #[case(1, 1)]
    #[case(1, 2)]
    #[case(1, 3)]
    #[case(2, 0)]
    #[case(2, 1)]
    #[case(2, 2)]
    #[case(2, 3)]
    #[case(3, 0)]
    #[case(3, 1)]
    #[case(3, 2)]
    #[case(3, 3)]
    fn building_comparators(#[case] left: u64, #[case] right: u64) {
        let secret = quaternary_shares(left);
        let public = ModularNumber::from_u64(right);
        let comparands = vec![QuatComparands { secret, public }];
        let comps = State::build_comparators(&comparands).unwrap();
        let eq = ModularNumber::from_u64((left == right) as u64);
        let lt = ModularNumber::from_u64((left > right) as u64);
        assert_eq!(comps.equal[0][0], eq);
        assert_eq!(comps.less[0][0], lt);
    }

    #[rstest]
    #[case(0, 1, 0)]
    #[case(2, 1, 3)]
    #[case(135132, 63456112, 1231)]
    #[case(3456340, 34561, 2340)]
    fn building_operands(#[case] eq: u64, #[case] r: u64, #[case] lt: u64) {
        let eq = ModularNumber::<U64SafePrime>::from_u64(eq);
        let r = ModularNumber::<U64SafePrime>::from_u64(r);
        let lt = ModularNumber::<U64SafePrime>::from_u64(lt);
        let z = ModularNumber::<U64SafePrime>::ZERO;
        let equal = Batches::from_flattened_fixed(vec![eq, r], 2).unwrap();
        let less = Batches::from_flattened_fixed(vec![lt, z], 2).unwrap();
        let comp = QuatComparators { equal, less };
        let ops = State::build_operands(&comp);
        assert_eq!(ops[0].left[0], eq);
        assert_eq!(ops[0].right[0], r);
        assert_eq!(ops[1].left[0], r);
        assert_eq!(ops[1].right[0], lt);
    }

    #[rstest]
    #[case(0, 1, 0, 2, 3)]
    #[case(5, 7, 3, 9, 13)]
    #[case(513212, 371234123, 31324, 9124524, 1324113)]
    #[case(324052, 112132, 12435234, 785421, 913482)]
    fn updating_comparators(#[case] eq: u64, #[case] r: u64, #[case] lt: u64, #[case] p0: u64, #[case] p1: u64) {
        let eq = ModularNumber::<U64SafePrime>::from_u64(eq);
        let r = ModularNumber::<U64SafePrime>::from_u64(r);
        let lt = ModularNumber::<U64SafePrime>::from_u64(lt);
        let z = ModularNumber::<U64SafePrime>::ZERO;
        let equal = Batches::from_flattened_fixed(vec![eq, r], 2).unwrap();
        let less = Batches::from_flattened_fixed(vec![lt, z], 2).unwrap();
        let comp = QuatComparators { equal, less };
        let p0 = ModularNumber::<U64SafePrime>::from_u64(p0);
        let p1 = ModularNumber::<U64SafePrime>::from_u64(p1);
        let products = vec![p0, p1];
        let comps = State::update_comparators(&comp, products).unwrap();
        let equal = p0;
        let less = z + &p1;
        println!("{:?}", comps);
        assert_eq!(comps.equal[0][0], equal);
        assert_eq!(comps.less[0][0], less);
    }
}

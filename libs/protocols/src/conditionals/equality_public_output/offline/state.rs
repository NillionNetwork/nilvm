//! PREP-PUBLIC-OUTPUT-EQUALITY protocol.

use super::{PrepPublicOutputEqualityShares, PrepPublicOutputEqualityStateOutput};
use crate::random::random_integer::state::{
    RandomIntegerError, RandomIntegerState, RandomIntegerStateMessage, RandomMode,
};
use basic_types::{PartyId, PartyMessage};
use math_lib::modular::{ModularNumber, SafePrime};
use serde::{Deserialize, Serialize};
use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer};
use state_machine::{
    errors::StateMachineError, sm::StateMachineOutput, state::StateMachineMessage, StateMachine, StateMachineState,
    StateMachineStateExt, StateMachineStateOutput, StateMachineStateResult,
};
use state_machine_derive::StateMachineState;
use std::sync::Arc;

/// The protocol states.
pub mod states {
    use crate::random::random_integer::RandomIntegerStateMachine;
    use math_lib::modular::{ModularNumber, SafePrime};
    use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer};
    use std::sync::Arc;

    /// We are waiting for RAN-ZERO protocol.
    pub struct WaitingRanZero<T>
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        /// The RAN_ZERO state machine.
        pub(crate) ran_zero_state_machine: RandomIntegerStateMachine<T>,

        /// The secret sharer to be used.
        pub(crate) secret_sharer: Arc<ShamirSecretSharer<T>>,

        /// The number of elements to be produced.
        pub(crate) element_count: usize,

        /// The bitwise numbers produced by RAN_ZERO.
        pub(crate) zero_two_t_shares: Vec<ModularNumber<T>>,
    }

    /// We are waiting for RAN-ZERO protocol.
    pub struct WaitingRan<T>
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        /// The RAN state machine.
        pub(crate) ran_state_machine: RandomIntegerStateMachine<T>,

        /// The bitwise numbers produced by RAN_ZERO.
        pub(crate) zero_two_t_shares: Vec<ModularNumber<T>>,

        /// The bitwise numbers produced by RAN.
        pub(crate) ran_shares: Vec<ModularNumber<T>>,
    }
}

/// The PREP-PUBLIC-OUTPUT-EQUALITY protocol state.
#[derive(StateMachineState)]
#[state_machine(
    recipient_id = "PartyId",
    input_message = "PartyMessage<PrepPublicOutputEqualityStateMessage>",
    output_message = "PrepPublicOutputEqualityStateMessage",
    final_result = "PrepPublicOutputEqualityStateOutput<PrepPublicOutputEqualityShares<T>>",
    handle_message_fn = "Self::handle_message"
)]
pub enum PrepPublicOutputEqualityState<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// We are waiting for RAN_ZERO.
    #[state_machine(submachine = "state.ran_zero_state_machine", transition_fn = "Self::transition_waiting_ran_zero")]
    WaitingRanZero(states::WaitingRanZero<T>),

    /// We are waiting for RAN.
    #[state_machine(submachine = "state.ran_state_machine", transition_fn = "Self::transition_waiting_ran")]
    WaitingRan(states::WaitingRan<T>),
}

use PrepPublicOutputEqualityState::*;

impl<T> PrepPublicOutputEqualityState<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Construct a new PREP-PUBLIC-OUTPUT-EQUALITY state.
    pub fn new(
        element_count: usize,
        secret_sharer: Arc<ShamirSecretSharer<T>>,
    ) -> Result<(Self, Vec<StateMachineMessage<Self>>), PrepPublicOutputEqualityCreateError> {
        let (ran_state_zero, messages) =
            RandomIntegerState::new(RandomMode::ZerosOfDegree2T, element_count, secret_sharer.clone())
                .map_err(PrepPublicOutputEqualityCreateError::RanZero)?;
        let state = states::WaitingRanZero {
            ran_zero_state_machine: StateMachine::new(ran_state_zero),
            secret_sharer,
            element_count,
            zero_two_t_shares: Vec::new(),
        };
        let messages =
            messages.into_iter().map(|message| message.wrap(&PrepPublicOutputEqualityStateMessage::RanZero)).collect();
        Ok((WaitingRanZero(state), messages))
    }

    fn transition_waiting_ran_zero(state: states::WaitingRanZero<T>) -> StateMachineStateResult<Self> {
        let mut shares = Vec::new();
        for zero_two_t in state.zero_two_t_shares.iter() {
            let share = PrepPublicOutputEqualityShares { zero_two_t: *zero_two_t, ran: ModularNumber::ZERO };
            shares.push(share);
        }

        let (ran_state, messages) =
            RandomIntegerState::new(RandomMode::RandomOfDegreeT, state.element_count, state.secret_sharer.clone())
                .map_err(|err| {
                    StateMachineError::UnexpectedError(PrepPublicOutputEqualityCreateError::RanZero(err).into())
                })?;
        let next_state = states::WaitingRan {
            ran_state_machine: StateMachine::new(ran_state),
            zero_two_t_shares: state.zero_two_t_shares.clone(),
            ran_shares: Vec::new(),
        };

        let messages =
            messages.into_iter().map(|message| message.wrap(&PrepPublicOutputEqualityStateMessage::Ran)).collect();
        Ok(StateMachineStateOutput::Messages(WaitingRan(next_state), messages))
    }

    fn transition_waiting_ran(state: states::WaitingRan<T>) -> StateMachineStateResult<Self> {
        let mut shares = Vec::new();
        for (ran, zero_two_t) in state.ran_shares.into_iter().zip(state.zero_two_t_shares.into_iter()) {
            let share = PrepPublicOutputEqualityShares { ran, zero_two_t };
            shares.push(share);
        }
        let output = PrepPublicOutputEqualityStateOutput::Success { shares };
        Ok(StateMachineStateOutput::Final(output))
    }

    fn handle_message(
        mut state: Self,
        message: PartyMessage<PrepPublicOutputEqualityStateMessage>,
    ) -> StateMachineStateResult<Self> {
        use PrepPublicOutputEqualityStateMessage::*;
        let (party_id, message) = message.into_parts();
        match (message, &mut state) {
            (RanZero(message), WaitingRanZero(inner)) => {
                match inner.ran_zero_state_machine.handle_message(PartyMessage::new(party_id, message))? {
                    StateMachineOutput::Final(shares) => {
                        inner.zero_two_t_shares = shares;
                        state.try_next()
                    }
                    output => state.wrap_message(output, PrepPublicOutputEqualityStateMessage::RanZero),
                }
            }
            (Ran(message), WaitingRan(inner)) => {
                match inner.ran_state_machine.handle_message(PartyMessage::new(party_id, message))? {
                    StateMachineOutput::Final(shares) => {
                        inner.ran_shares = shares;
                        state.try_next()
                    }
                    output => state.wrap_message(output, PrepPublicOutputEqualityStateMessage::Ran),
                }
            }
            (message, _) => Ok(StateMachineStateOutput::OutOfOrder(state, PartyMessage::new(party_id, message))),
        }
    }
}

/// A message for the PREP-PUBLIC-OUTPUT-EQUALITY protocol.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[repr(u8)]
pub enum PrepPublicOutputEqualityStateMessage {
    /// A message for the RAN_ZERO state machine.
    RanZero(RandomIntegerStateMessage) = 0,
    /// A message for the RAN state machine.
    Ran(RandomIntegerStateMessage) = 1,
}

/// An error during the creation of the PREP-PUBLIC-OUTPUT-EQUALITY state.
#[derive(Debug, thiserror::Error)]
pub enum PrepPublicOutputEqualityCreateError {
    /// An error during the RAN_ZERO creation.
    #[error("RAN_ZERO: {0}")]
    RanZero(RandomIntegerError),
    /// An error during the RAN creation.
    #[error("RAN: {0}")]
    Ran(RandomIntegerError),
}

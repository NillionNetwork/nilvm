//! Tests for state machines.

#![allow(clippy::indexing_slicing)]

use crate::{
    errors::StateUnavailableError,
    state::{Recipient, RecipientMessage, StateMachineStateExt, StateMachineStateOutput, StateMachineStateResult},
    StateMachine, StateMachineState,
};
use anyhow::Result;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;

#[derive(Clone, PartialEq, Hash, Eq)]
struct PartyId(u32);

struct Messages {
    party_count: usize,
    party_messages: HashMap<PartyId, u32>,
}

impl Messages {
    fn new(party_count: usize) -> Self {
        Self { party_count, party_messages: HashMap::new() }
    }
}

// This a testing state that transitions:
//
// * `WaitingA` -> `Processing` when enough party messages are stored in the map. The `Processing`'s expected counter
// will be the same as the `Waiting`'s one, having an empty message map.
// * `WaitingB` -> `WaitingC` when the current counter reaches the expected.
// * `WaitingC` -> completion when the current counter reaches the expected.
enum WaiterState {
    WaitingA(Messages),
    WaitingB(Messages),
    WaitingC(Messages),
}

impl std::fmt::Display for WaiterState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WaitingA(_) => write!(f, "WaitingA"),
            WaitingB(_) => write!(f, "WaitingB"),
            WaitingC(_) => write!(f, "WaitingC"),
        }
    }
}

use WaiterState::*;

impl WaiterState {
    fn new(party_count: usize) -> Self {
        WaitingA(Messages::new(party_count))
    }
}

impl StateMachineState for WaiterState {
    type RecipientId = PartyId;
    type InputMessage = StoreMessage;
    type OutputMessage = StoreMessage;
    type FinalResult = CompletedMessage;

    fn is_completed(&self) -> bool {
        match self {
            WaitingA(state) | WaitingB(state) | WaitingC(state) => state.party_messages.len() == state.party_count,
        }
    }

    fn try_next(self) -> StateMachineStateResult<Self> {
        match self {
            WaitingA(state) => {
                // Let's pretend like we're sending an output message to party id 42, from us (party id 1)
                let message = RecipientMessage::new(Recipient::Single(PartyId(42)), StoreMessage::B(PartyId(1), 1337));
                let next_state = WaitingB(Messages::new(state.party_count));
                Ok(StateMachineStateOutput::Messages(next_state, vec![message]))
            }
            WaitingB(state) => {
                // Same as above....
                let message = RecipientMessage::new(Recipient::Single(PartyId(42)), StoreMessage::C(PartyId(1), 1337));
                let next_state = WaitingC(Messages::new(state.party_count));
                Ok(StateMachineStateOutput::Messages(next_state, vec![message]))
            }
            WaitingC(_) => Ok(StateMachineStateOutput::Final(CompletedMessage)),
        }
    }

    fn handle_message(mut self, message: Self::InputMessage) -> StateMachineStateResult<Self> {
        use StoreMessage::*;
        match (message, &mut self) {
            (A(party_id, value), WaitingA(inner))
            | (B(party_id, value), WaitingB(inner))
            | (C(party_id, value), WaitingC(inner)) => {
                inner.party_messages.insert(party_id, value);
                self.advance_if_completed()
            }
            (message, _) => Ok(StateMachineStateOutput::OutOfOrder(self, message)),
        }
    }
}

#[derive(Clone)]
enum StoreMessage {
    A(PartyId, u32),
    B(PartyId, u32),
    C(PartyId, u32),
}

// Adding dummy implementations here as this is unused so there's no point in bringing in serde-derive.
impl Serialize for StoreMessage {
    fn serialize<S>(&self, _: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::Error;
        Err(S::Error::custom("not implemented"))
    }
}

impl<'de> Deserialize<'de> for StoreMessage {
    fn deserialize<D>(_: D) -> Result<StoreMessage, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::Error;
        Err(D::Error::custom("not implemented"))
    }
}

struct CompletedMessage;

#[test]
fn linear_state_transitions() -> Result<()> {
    let mut sm = StateMachine::new(WaiterState::new(2));

    // Two messages should take us to the B state.
    assert!(sm.handle_message(StoreMessage::A(PartyId(1), 10))?.into_empty().is_ok());
    let messages = sm.handle_message(StoreMessage::A(PartyId(2), 20))?.into_messages()?;
    assert_eq!(messages.len(), 1);
    assert!(matches!(messages[0].contents(), StoreMessage::B(..)));

    // Two increments should take us to C.
    assert!(sm.handle_message(StoreMessage::B(PartyId(1), 10))?.into_empty().is_ok());
    assert!(sm.handle_message(StoreMessage::B(PartyId(2), 20))?.into_messages().is_ok());

    // Two increments should produce the final output.
    assert!(sm.handle_message(StoreMessage::C(PartyId(1), 10))?.into_empty().is_ok());
    let output = sm.handle_message(StoreMessage::C(PartyId(2), 20))?;
    assert!(output.into_final().is_ok());

    Ok(())
}

#[test]
fn out_of_order_messages_partial() -> Result<()> {
    let mut sm = StateMachine::new(WaiterState::new(2));

    // First send one message for B.
    assert!(sm.handle_message(StoreMessage::B(PartyId(1), 10))?.into_empty().is_ok());

    // Now send the messages for A. We should have transitioned into B and get the messages for B.
    assert!(sm.handle_message(StoreMessage::A(PartyId(1), 10))?.into_empty().is_ok());
    let messages = sm.handle_message(StoreMessage::A(PartyId(2), 20))?.into_messages()?;
    assert_eq!(messages.len(), 1);
    assert!(matches!(messages[0].contents(), StoreMessage::B(..)));

    // Now send the last message for B, we should be in C
    assert!(sm.handle_message(StoreMessage::B(PartyId(2), 20))?.into_messages().is_ok());
    assert!(matches!(sm.state()?, WaitingC(_)));

    Ok(())
}

#[test]
fn out_of_order_messages_into_final() -> Result<()> {
    // Note that we start in state B to simplify the test
    let mut sm = StateMachine::new(WaitingB(Messages::new(2)));

    // First send the messages for C.
    assert!(sm.handle_message(StoreMessage::C(PartyId(1), 10))?.into_empty().is_ok());
    assert!(sm.handle_message(StoreMessage::C(PartyId(2), 20))?.into_empty().is_ok());

    // Now send the messages for B. We should have transitioned to the final state as we already have the C ones.
    assert!(sm.handle_message(StoreMessage::B(PartyId(1), 10))?.into_empty().is_ok());
    let output = sm.handle_message(StoreMessage::B(PartyId(2), 20))?;
    assert!(output.into_final().is_ok());

    assert!(matches!(sm.state(), Err(StateUnavailableError("state machine reached terminal state"))));
    Ok(())
}

#[test]
fn out_of_order_for_two_states() -> Result<()> {
    let mut sm = StateMachine::new(WaiterState::new(2));

    // First send one message for C.
    assert!(sm.handle_message(StoreMessage::C(PartyId(1), 10))?.into_empty().is_ok());

    // Now send the messages for B. We are still waiting for A's...
    assert!(sm.handle_message(StoreMessage::B(PartyId(1), 10))?.into_empty().is_ok());
    assert!(sm.handle_message(StoreMessage::B(PartyId(2), 20))?.into_empty().is_ok());

    // Finally, send the ones for A. This should spit out 2 messages, one for B and one for C, and we should be in C
    assert!(sm.handle_message(StoreMessage::A(PartyId(1), 10))?.into_empty().is_ok());
    let messages = sm.handle_message(StoreMessage::A(PartyId(2), 20))?.into_messages()?;
    assert_eq!(messages.len(), 2);
    assert!(matches!(messages[0].contents(), StoreMessage::B(..)));
    assert!(matches!(messages[1].contents(), StoreMessage::C(..)));

    Ok(())
}

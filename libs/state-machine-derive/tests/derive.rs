use serde::{Deserialize, Deserializer, Serialize, Serializer};
use state_machine::{
    errors::StateMachineError, state::StateMachineStateOutput, StateMachineState, StateMachineStateResult,
};
use state_machine_derive::StateMachineState;

pub mod states {
    #[derive(Debug)]
    pub struct WaitingSomething {
        pub current: u8,
        pub expected: u8,
    }

    #[derive(Debug)]
    pub struct WaitingSomethingElse;

    #[derive(Debug)]
    pub struct WaitingGeneric<T> {
        pub inner: T,
    }
}

#[derive(Debug, StateMachineState)]
#[state_machine(
    recipient_id = "u32",
    input_message = "Message",
    output_message = "Message",
    final_result = "String",
    handle_message_fn = "handle_message"
)]
#[allow(dead_code)]
enum State1 {
    #[state_machine(completed = "state.current == state.expected", transition_fn = "transition_waiting_something")]
    WaitingSomething(states::WaitingSomething),

    #[state_machine(completed = "true", transition_fn = "transition_waiting_something_else")]
    WaitingSomethingElse(states::WaitingSomethingElse),

    #[state_machine(completed_fn = "always_true", transition_fn = "transition_waiting_generic")]
    WaitingGeneric(states::WaitingGeneric<u8>),
}

#[derive(Clone)]
struct Message;

fn handle_message(_: State1, _: Message) -> Result<StateMachineStateOutput<State1>, StateMachineError> {
    Ok(StateMachineStateOutput::Final("hello".to_string()))
}

// Adding dummy implementations here as this is unused so there's no point in bringing in serde-derive.
impl Serialize for Message {
    fn serialize<S>(&self, _: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::Error;
        Err(S::Error::custom("not implemented"))
    }
}

impl<'de> Deserialize<'de> for Message {
    fn deserialize<D>(_: D) -> Result<Message, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::Error;
        Err(D::Error::custom("not implemented"))
    }
}

fn always_true(_: &states::WaitingGeneric<u8>) -> bool {
    true
}

fn transition_waiting_something(_: states::WaitingSomething) -> StateMachineStateResult<State1> {
    Ok(State1::WaitingSomethingElse(states::WaitingSomethingElse).into())
}

fn transition_waiting_something_else(_: states::WaitingSomethingElse) -> StateMachineStateResult<State1> {
    Ok(State1::WaitingGeneric(states::WaitingGeneric { inner: 42 }).into())
}

fn transition_waiting_generic(_: states::WaitingGeneric<u8>) -> StateMachineStateResult<State1> {
    Ok(StateMachineStateOutput::Final("hello".to_string()))
}

#[test]
fn state_accessors() {
    let mut s = State1::WaitingSomething(states::WaitingSomething { current: 0, expected: 1 });
    assert!(s.waiting_something_state().is_ok());
    assert!(s.waiting_something_state_mut().is_ok());

    assert!(s.waiting_something_else_state().is_err());
    assert!(s.waiting_something_else_state_mut().is_err());

    assert!(s.waiting_generic_state().is_err());
    assert!(s.waiting_generic_state_mut().is_err());
}

#[test]
fn completion() {
    let s = State1::WaitingSomething(states::WaitingSomething { current: 0, expected: 1 });
    assert!(!s.is_completed());

    let s = State1::WaitingSomething(states::WaitingSomething { current: 1, expected: 1 });
    assert!(s.is_completed());
}

#[test]
fn message_handling() {
    let s = State1::WaitingSomething(states::WaitingSomething { current: 0, expected: 1 });
    let output = s.handle_message(Message).unwrap();
    assert!(matches!(output, StateMachineStateOutput::Final(_)));
}

#[test]
fn transition() {
    let s = State1::WaitingSomethingElse(states::WaitingSomethingElse);
    let s = s.try_next().unwrap();
    assert!(matches!(s, StateMachineStateOutput::Empty(State1::WaitingGeneric(_))));
}

#[test]
fn access_inner_refs() {
    let mut s = State1::WaitingSomething(states::WaitingSomething { current: 0, expected: 1 });
    let _: &states::WaitingSomething = s.waiting_something_state().unwrap();
    let _: &mut states::WaitingSomething = s.waiting_something_state_mut().unwrap();
}

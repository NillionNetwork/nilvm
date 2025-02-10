# State machine

This crates implements an abstraction over a state machine and its state. This allows defining new state machines by:
* Implementing the `StateMachineState` trait for the enum representing the different states.
* Defining a state machine using the `StateMachine` type, using the state type as its generic argument.

This lets you easily define state transitions, requirements for each state to be completed, etc. See the tests
and documentation for each type for more information.

For users trying to define state machines, it is highly recommended to use the `state-machine-derive` macro to do so
as this automatically defines a few dozen lines of boilerplate that you'd otherwise have to write yourself.

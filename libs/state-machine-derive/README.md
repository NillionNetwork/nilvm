# State machine state derivation

This crate exports a derive macro that allows automatically defining all the boilerplate code for state machine states.

Currently, it only supports defining the accessors for each state. That is, the code that allows accessing
every inner state like `my_state.waiting_for_something_state()?` and `my_state.waiting_for_something_state_mut()?`.

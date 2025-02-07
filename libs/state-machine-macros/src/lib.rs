/// Allows defining a state machine that can be used behind a `dyn`.
///
/// The state machine's state needs to have a single generic type that impls `SafePrime`.
///
/// # Example
///
/// ```ignore
///
/// struct FooState<T>;
///
/// define_dyn_state_machine!(FooState);
///
/// let sm = StateMachine::new(FooState::new(...));
/// let sm: Box<dyn FooStateMachineDyn> = DefaultFooStateMachine(sm);
/// ```
#[macro_export]
macro_rules! define_dyn_state_machine {
    ($trait_name:ident, $struct_name:ident, $state_name:ident) => {
        /// A trait that allows using a `StateMachine<$state_name>` behind a `dyn`.
        pub trait $trait_name: Send + 'static {
            /// Handle a message.
            ///
            /// See `[state_machine::StateMachine]`.
            fn handle_message(
                &mut self,
                message: <$state_name<math_lib::modular::U64SafePrime> as state_machine::StateMachineState>::InputMessage,
            ) -> Result<
                state_machine::StateMachineOutput<
                    basic_types::PartyId,
                    <$state_name<math_lib::modular::U64SafePrime> as state_machine::StateMachineState>::OutputMessage,
                    <$state_name<math_lib::modular::U64SafePrime> as state_machine::StateMachineState>::FinalResult,
                >,
                state_machine::errors::StateMachineError,
            >;
        }

        /// A state machine wrapper that can be used behind a `dyn`.
        pub struct $struct_name<T>(pub state_machine::StateMachine<$state_name<T>>)
        where
            T: math_lib::modular::SafePrime,
            shamir_sharing::secret_sharer::ShamirSecretSharer<T>: shamir_sharing::secret_sharer::SafePrimeSecretSharer<T>;

        impl<T> $trait_name for $struct_name<T>
        where
            T: math_lib::modular::SafePrime,
            shamir_sharing::secret_sharer::ShamirSecretSharer<T>: shamir_sharing::secret_sharer::SafePrimeSecretSharer<T>,
        {
            fn handle_message(
                &mut self,
                message: <$state_name<T> as state_machine::StateMachineState>::InputMessage,
            ) -> Result<
                state_machine::StateMachineOutput<
                    basic_types::PartyId,
                    <$state_name<T> as state_machine::StateMachineState>::OutputMessage,
                    <$state_name<T> as state_machine::StateMachineState>::FinalResult,
                >,
                state_machine::errors::StateMachineError,
            > {
                self.0.handle_message(message)
            }
        }
    };
    ($state:ident) => {
        $crate::paste! {
            $crate::define_dyn_state_machine!([<$state MachineDyn>], [<Default $state Machine>], $state);
        }
    };
}

/// Allows defining a state machine that can be used behind a `dyn`.
///
/// As opposed to `define_dyn_state_machine`, this macro converts the final output into the
/// provided type, granted the state machine's output type can be converted to this type via
/// `output.encode()`.
///
/// # Example
///
/// ```ignore
///
/// define_encoded_dyn_state_machine!(FooState);
///
/// let sm = StateMachine::new(FooState::new(...));
/// let sm: Box<dyn FooStateMachineDyn> = DefaultFooStateMachine(sm);
/// ```
#[macro_export]
macro_rules! define_encoded_dyn_state_machine {
    ($trait_name:ident, $struct_name:ident, $state_name:ident, $output:ty) => {
        /// A trait that allows using a `StateMachine<ComputeComputeStateMachine>` behind a `dyn`.
        pub trait $trait_name: Send + 'static {
            /// Handle a message.
            ///
            /// See `[state_machine::StateMachine]`.
            fn handle_message(
                &mut self,
                message: <$state_name<math_lib::modular::U64SafePrime> as state_machine::StateMachineState>::InputMessage,
            ) -> Result<
                state_machine::StateMachineOutput<
                    basic_types::PartyId,
                    <$state_name<math_lib::modular::U64SafePrime> as state_machine::StateMachineState>::OutputMessage,
                    $output
                >,
                state_machine::errors::StateMachineError,
            >;
        }

        /// A state machine wrapper that can be used behind a `dyn`.
        pub struct $struct_name<T>(pub state_machine::StateMachine<$state_name<T>>)
        where
            T: math_lib::modular::SafePrime,
            shamir_sharing::secret_sharer::ShamirSecretSharer<T>: shamir_sharing::secret_sharer::SafePrimeSecretSharer<T>;

        impl<T> $trait_name for $struct_name<T>
        where
            T: math_lib::modular::SafePrime,
            shamir_sharing::secret_sharer::ShamirSecretSharer<T>: shamir_sharing::secret_sharer::SafePrimeSecretSharer<T>,
        {
            fn handle_message(
                &mut self,
                message: <$state_name<T> as state_machine::StateMachineState>::InputMessage,
            ) -> Result<
                state_machine::StateMachineOutput<
                    basic_types::PartyId,
                    <$state_name<T> as state_machine::StateMachineState>::OutputMessage,
                    $output
                >,
                state_machine::errors::StateMachineError,
            > {
                use state_machine::StateMachineOutput;
                let output = match self.0.handle_message(message)? {
                    StateMachineOutput::Final(output) => {
                        StateMachineOutput::Final(output.encode().map_err(|e| anyhow!("unimplemented: {e}"))?)
                    },
                    StateMachineOutput::Messages(messages) => StateMachineOutput::Messages(messages),
                    StateMachineOutput::Empty => StateMachineOutput::Empty,
                };
                Ok(output)
            }
        }
    };
    ($state:ident, $output:ty) => {
        $crate::paste! {
            $crate::define_encoded_dyn_state_machine!(
                [<$state MachineDyn>],
                [<Default $state Machine>],
                $state,
                $output
            );
        }
    };
}

pub use paste::paste;

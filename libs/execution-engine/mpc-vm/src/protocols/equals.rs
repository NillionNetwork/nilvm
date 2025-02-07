//! Implementation of the MPC protocols for the equals operation

use crate::{protocols::MPCProtocol, requirements::RuntimeRequirementType, utils::into_mpc_protocol};
use jit_compiler::{
    binary_protocol,
    bytecode2protocol::{errors::Bytecode2ProtocolError, Bytecode2Protocol, Bytecode2ProtocolContext, ProtocolFactory},
    models::{
        bytecode::{Equals as BytecodeEquals, PublicOutputEquality as BytecodePublicOutputEquality},
        protocols::{memory::ProtocolAddress, ExecutionLine},
    },
    public_relational_protocol, share_relational_protocol,
};

pub(crate) struct Equals;

impl Equals {
    /// Transforms a bytecode equals into a protocol
    pub(crate) fn transform<F: ProtocolFactory<MPCProtocol>>(
        context: &mut Bytecode2ProtocolContext<MPCProtocol, F>,
        operation: &BytecodeEquals,
    ) -> Result<MPCProtocol, Bytecode2ProtocolError> {
        let left_type = context.bytecode.memory_element_type(operation.left)?;
        let right_type = context.bytecode.memory_element_type(operation.right)?;

        use nada_value::NadaType::*;
        match (left_type, right_type) {
            // Checking for public-public tuples
            (Integer, Integer) | (UnsignedInteger, UnsignedInteger) | (Boolean, Boolean) => {
                EqualsPublic::public_protocol(context, operation, left_type)
            }
            // Mixed (Public, Secret)
            (Integer, SecretInteger)
            | (UnsignedInteger, SecretUnsignedInteger)
            | (Boolean, SecretBoolean)
            // Mixed (Secret, Public)
            | (SecretInteger, Integer)
            | (SecretUnsignedInteger, UnsignedInteger)
            | (SecretBoolean, Boolean)
            // (Secret, Secret)
            | (SecretInteger, SecretInteger)
            | (SecretUnsignedInteger, SecretUnsignedInteger)
            | (SecretBoolean, SecretBoolean) => {
                EqualsSecret::share_protocol(context, operation, left_type)
            }
            _ => {
                let msg = format!("type {left_type} == {right_type} not supported");
                Err(Bytecode2ProtocolError::OperationNotSupported(msg))
            }
        }
    }
}

binary_protocol!(
    EqualsSecret,
    "EQS",
    ExecutionLine::Online,
    RuntimeRequirementType,
    &[(RuntimeRequirementType::EqualsIntegerSecret, 1)]
);
into_mpc_protocol!(EqualsSecret);
impl EqualsSecret {
    share_relational_protocol!(BytecodeEquals);
}

binary_protocol!(EqualsPublic, "EQP", ExecutionLine::Local, RuntimeRequirementType);
into_mpc_protocol!(EqualsPublic);
impl EqualsPublic {
    public_relational_protocol!(BytecodeEquals);
}

// A protocol that performs equality comparison and returns a public variable.
binary_protocol!(
    PublicOutputEquality,
    "EQC",
    ExecutionLine::Online,
    RuntimeRequirementType,
    &[(RuntimeRequirementType::PublicOutputEquality, 1)]
);
into_mpc_protocol!(PublicOutputEquality);
impl PublicOutputEquality {
    /// Transforms a bytecode public output equality into a protocol
    pub(crate) fn transform<F: ProtocolFactory<MPCProtocol>>(
        context: &mut Bytecode2ProtocolContext<MPCProtocol, F>,
        operation: &BytecodePublicOutputEquality,
    ) -> Result<MPCProtocol, Bytecode2ProtocolError> {
        let left_type = context.bytecode.memory_element_type(operation.left)?;
        let right_type = context.bytecode.memory_element_type(operation.right)?;

        use nada_value::NadaType::*;
        match (left_type, right_type) {
            // Checking for public-public tuples
            (Integer, Integer) | (UnsignedInteger, UnsignedInteger) | (Boolean, Boolean) => {
                let protocol = EqualsPublic {
                    address: ProtocolAddress::default(),
                    left: Bytecode2Protocol::adapted_protocol(context, operation.left, left_type)?,
                    right: Bytecode2Protocol::adapted_protocol(context, operation.right, right_type)?,
                    ty: Boolean,
                    source_ref_index: operation.source_ref_index,
                };
                Ok(protocol.into())
            }
            // Mixed (Public, Secret)
            (Integer, SecretInteger)
            | (UnsignedInteger, SecretUnsignedInteger)
            | (Boolean, SecretBoolean)
            // Mixed (Secret, Public)
            | (SecretInteger, Integer)
            | (SecretUnsignedInteger, UnsignedInteger)
            | (SecretBoolean, Boolean)
            // (Secret, Secret)
            | (SecretInteger, SecretInteger)
            | (SecretUnsignedInteger, SecretUnsignedInteger)
            | (SecretBoolean, SecretBoolean) => {
                let expected_ty = left_type.as_shamir_share()?;
                let protocol = Self {
                    address: ProtocolAddress::default(),
                    left: Bytecode2Protocol::adapted_protocol(context, operation.left, &expected_ty)?,
                    right: Bytecode2Protocol::adapted_protocol(context, operation.right, &expected_ty)?,
                    ty: Boolean,
                    source_ref_index: operation.source_ref_index,
                };
                Ok(protocol.into())
            }
            _ => {
                let msg = format!("type {left_type} == {right_type} not supported");
                Err(Bytecode2ProtocolError::OperationNotSupported(msg))
            }
        }
    }
}

#[cfg(any(test, feature = "vm"))]
pub(crate) mod vm {
    use crate::{
        protocols::{EqualsPublic, EqualsSecret, PublicOutputEquality},
        vm::{plan::MPCProtocolPreprocessingElements, MPCInstructionRouter, MPCMessages},
    };
    use anyhow::{anyhow, Error};
    use execution_engine_vm::vm::{
        instructions::{into_instruction_messages, DefaultInstructionStateMachine, Instruction, InstructionResult},
        memory::MemoryValue,
        sm::ExecutionContext,
    };
    use math_lib::modular::{ModularNumber, SafePrime};
    use nada_value::NadaValue;
    use protocols::conditionals::{
        equality::{PrivateOutputEqualityState, PrivateOutputEqualityStateMessage},
        equality_public_output::{
            PublicOutputEqualityShares, PublicOutputEqualityState, PublicOutputEqualityStateMessage,
        },
    };
    use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer};

    impl<T> Instruction<T> for EqualsPublic
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        type PreprocessingElement = MPCProtocolPreprocessingElements<T>;
        type Router = MPCInstructionRouter<T>;
        type Message = MPCMessages;

        fn run<F>(
            self,
            context: &mut ExecutionContext<F, T>,
            _: Self::PreprocessingElement,
        ) -> Result<InstructionResult<Self::Router, T>, Error>
        where
            F: Instruction<T>,
        {
            use nada_value::NadaType::*;

            let left = context.read(self.left)?;
            let right = context.read(self.right)?;

            let left_type = left.to_type();
            let right_type = right.to_type();

            let left = left.try_into_value()?;
            let right = right.try_into_value()?;

            // Equals in the clear for public variables
            match (left_type, right_type) {
                (Integer, Integer) | (UnsignedInteger, UnsignedInteger) | (Boolean, Boolean) => {
                    let result = if left == right { ModularNumber::ONE } else { ModularNumber::ZERO };
                    Ok(InstructionResult::Value { value: NadaValue::new_boolean(result) })
                }
                (left, right) => Err(anyhow!("unsupported operands for Equals public protocol: {left:?} / {right:?}")),
            }
        }
    }

    impl<T> Instruction<T> for EqualsSecret
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        type PreprocessingElement = MPCProtocolPreprocessingElements<T>;
        type Router = MPCInstructionRouter<T>;
        type Message = MPCMessages;

        fn run<F>(
            self,
            context: &mut ExecutionContext<F, T>,
            mut share_elements: Self::PreprocessingElement,
        ) -> Result<InstructionResult<Self::Router, T>, Error>
        where
            F: Instruction<T>,
        {
            use nada_value::NadaType::*;

            let left = context.read(self.left)?;
            let right = context.read(self.right)?;

            match (left.to_type(), right.to_type()) {
                (ShamirShareInteger, Integer)
                | (ShamirShareUnsignedInteger, UnsignedInteger)
                | (ShamirShareInteger, ShamirShareInteger)
                | (Integer, ShamirShareInteger)
                | (ShamirShareUnsignedInteger, ShamirShareUnsignedInteger)
                | (UnsignedInteger, ShamirShareUnsignedInteger)
                | (ShamirShareBoolean, Boolean)
                | (Boolean, ShamirShareBoolean)
                | (ShamirShareBoolean, ShamirShareBoolean) => {
                    let left = left.try_into_value()?;
                    let right = right.try_into_value()?;

                    // Use Equals with Integer Public divisor protocol for secret dividends.
                    let prep_elements = share_elements
                        .equals_integer_secret
                        .pop()
                        .ok_or_else(|| anyhow!("shares not found for Equals integer with public divisor"))?;
                    let (initial_state, messages) = PrivateOutputEqualityState::new(
                        vec![left],
                        vec![right],
                        vec![prep_elements],
                        context.secret_sharer(),
                    )?;

                    Ok(InstructionResult::StateMachine {
                        state_machine: MPCInstructionRouter::EqualsIntegerSecret(DefaultInstructionStateMachine::new(
                            initial_state,
                            ShamirShareBoolean,
                        )),
                        messages: into_instruction_messages(messages),
                    })
                }
                (left, right) => Err(anyhow!("unsupported operands for secret equals: {left:?} / {right:?}")),
            }
        }
    }

    impl From<PrivateOutputEqualityStateMessage> for MPCMessages {
        fn from(message: PrivateOutputEqualityStateMessage) -> Self {
            MPCMessages::EqualsIntegerSecret(message)
        }
    }

    impl TryFrom<MPCMessages> for PrivateOutputEqualityStateMessage {
        type Error = Error;

        fn try_from(msg: MPCMessages) -> Result<Self, Self::Error> {
            let MPCMessages::EqualsIntegerSecret(msg) = msg else {
                return Err(anyhow!("unknown instruction message"));
            };
            Ok(msg)
        }
    }

    impl<T> Instruction<T> for PublicOutputEquality
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        type PreprocessingElement = MPCProtocolPreprocessingElements<T>;
        type Router = MPCInstructionRouter<T>;
        type Message = MPCMessages;

        fn run<F>(
            self,
            context: &mut ExecutionContext<F, T>,
            mut share_elements: Self::PreprocessingElement,
        ) -> Result<InstructionResult<Self::Router, T>, Error>
        where
            F: Instruction<T>,
        {
            let prep_shares = share_elements.public_output_equality.pop().ok_or_else(|| anyhow!("shares not found"))?;
            let right = context.read(self.right)?;
            let left = context.read(self.left)?;

            use nada_value::NadaType::*;
            match (left.to_type(), right.to_type()) {
                (Integer, Integer)
                | (UnsignedInteger, UnsignedInteger)
                | (ShamirShareInteger, ShamirShareInteger)
                | (ShamirShareInteger, Integer)
                | (Integer, ShamirShareInteger)
                | (ShamirShareUnsignedInteger, ShamirShareUnsignedInteger)
                | (UnsignedInteger, ShamirShareUnsignedInteger)
                | (ShamirShareUnsignedInteger, UnsignedInteger) => {
                    let left = left.try_into_value()?;
                    let right = right.try_into_value()?;
                    let (initial_state, messages) = PublicOutputEqualityState::new(
                        vec![PublicOutputEqualityShares { left, right, prep_shares }],
                        context.secret_sharer(),
                    )?;

                    Ok(InstructionResult::StateMachine {
                        state_machine: MPCInstructionRouter::PublicOutputEquality(DefaultInstructionStateMachine::new(
                            initial_state,
                            Boolean,
                        )),
                        messages: into_instruction_messages(messages),
                    })
                }
                (left, right) => Err(anyhow!("unsupported operands for public equality: {left:?} == {right:?}")),
            }
        }
    }

    impl From<PublicOutputEqualityStateMessage> for MPCMessages {
        fn from(message: PublicOutputEqualityStateMessage) -> Self {
            MPCMessages::PublicOutputEquality(message)
        }
    }

    impl TryFrom<MPCMessages> for PublicOutputEqualityStateMessage {
        type Error = Error;

        fn try_from(msg: MPCMessages) -> Result<Self, Self::Error> {
            let MPCMessages::PublicOutputEquality(msg) = msg else {
                return Err(anyhow!("unknown instruction message"));
            };
            Ok(msg)
        }
    }
}

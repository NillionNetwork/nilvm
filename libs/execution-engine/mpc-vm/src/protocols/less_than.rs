//! Implementation of the MPC protocols for the less-than operation

use crate::{protocols::MPCProtocol, requirements::RuntimeRequirementType, utils::into_mpc_protocol};
use jit_compiler::{
    binary_protocol,
    bytecode2protocol::{errors::Bytecode2ProtocolError, Bytecode2ProtocolContext, ProtocolFactory},
    models::{bytecode::LessThan as BytecodeLessThan, protocols::ExecutionLine},
    public_relational_protocol, share_relational_protocol,
};

pub(crate) struct LessThan;

impl LessThan {
    /// Transforms a bytecode less-than into a protocol
    pub(crate) fn transform<F: ProtocolFactory<MPCProtocol>>(
        context: &mut Bytecode2ProtocolContext<MPCProtocol, F>,
        operation: &BytecodeLessThan,
    ) -> Result<MPCProtocol, Bytecode2ProtocolError> {
        let left_type = context.bytecode.memory_element_type(operation.left)?;
        let right_type = context.bytecode.memory_element_type(operation.right)?;
        use nada_value::NadaType::*;
        match (left_type, right_type) {
            // Checking for public-public tuples
            (Integer, Integer) | (UnsignedInteger, UnsignedInteger) | (Boolean, Boolean) => {
                LessThanPublic::public_protocol(context, operation, left_type)
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
                LessThanShares::share_protocol(context, operation, left_type)
            }
            _ => {
                let msg = format!("type {left_type} < {right_type} not supported");
                Err(Bytecode2ProtocolError::OperationNotSupported(msg))
            }
        }
    }
}

//  Less than with public inputs.
binary_protocol!(LessThanPublic, "LTC", ExecutionLine::Local, RuntimeRequirementType);
into_mpc_protocol!(LessThanPublic);
impl LessThanPublic {
    public_relational_protocol!(BytecodeLessThan);
}

//  Less than with share inputs.
binary_protocol!(
    LessThanShares,
    "LTS",
    ExecutionLine::Online,
    RuntimeRequirementType,
    &[(RuntimeRequirementType::Compare, 1)]
);
into_mpc_protocol!(LessThanShares);
impl LessThanShares {
    share_relational_protocol!(BytecodeLessThan);
}

#[cfg(any(test, feature = "vm"))]
pub(crate) mod vm {
    use crate::{
        protocols::{LessThanPublic, LessThanShares},
        vm::{plan::MPCProtocolPreprocessingElements, MPCInstructionRouter, MPCMessages},
    };
    use anyhow::{anyhow, Error};
    use execution_engine_vm::vm::{
        instructions::{into_instruction_messages, DefaultInstructionStateMachine, Instruction, InstructionResult},
        memory::MemoryValue,
        sm::ExecutionContext,
    };
    use math_lib::modular::{ModularNumber, SafePrime};
    use nada_value::{BigInt, NadaValue};
    use protocols::conditionals::less_than::online::state::{Comparands, CompareState, CompareStateMessage};
    use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer};

    impl<T> Instruction<T> for LessThanPublic
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
            let right = context.read(self.right)?;
            let left = context.read(self.left)?;

            use nada_value::NadaType::*;
            match (left.to_type(), right.to_type()) {
                (UnsignedInteger, UnsignedInteger) => {
                    let left = left.try_into_value()?;
                    let right = right.try_into_value()?;
                    let value = if left < right { ModularNumber::ONE } else { ModularNumber::ZERO };
                    Ok(InstructionResult::Value { value: NadaValue::new_boolean(value) })
                }
                (Integer, Integer) => {
                    let left = BigInt::from(&left.try_into_value()?);
                    let right = BigInt::from(&right.try_into_value()?);
                    let value = if left < right { ModularNumber::ONE } else { ModularNumber::ZERO };
                    Ok(InstructionResult::Value { value: NadaValue::new_boolean(value) })
                }
                (left, right) => {
                    Err(anyhow!("unsupported operands for less than public protocol: {left:?} < {right:?}"))
                }
            }
        }
    }

    impl<T> Instruction<T> for LessThanShares
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
            let prep_elements = share_elements.compare.pop().ok_or_else(|| anyhow!("shares not found"))?;
            let right = context.read(self.right)?;
            let left = context.read(self.left)?;

            use nada_value::NadaType::*;
            match (left.to_type(), right.to_type()) {
                (ShamirShareInteger, ShamirShareInteger)
                | (ShamirShareInteger, Integer)
                | (Integer, ShamirShareInteger)
                | (ShamirShareUnsignedInteger, ShamirShareUnsignedInteger)
                | (UnsignedInteger, ShamirShareUnsignedInteger)
                | (ShamirShareUnsignedInteger, UnsignedInteger) => {
                    let right = right.try_into_value()?;
                    let left = left.try_into_value()?;
                    let comparands = Comparands { left, right, prep_elements };

                    let (initial_state, messages) = CompareState::new(vec![comparands], context.secret_sharer())?;

                    Ok(InstructionResult::StateMachine {
                        state_machine: MPCInstructionRouter::LessThan(DefaultInstructionStateMachine::new(
                            initial_state,
                            ShamirShareBoolean,
                        )),
                        messages: into_instruction_messages(messages),
                    })
                }
                (left, right) => {
                    Err(anyhow!("unsupported operands for less than shares protocol: {left:?} < {right:?}"))
                }
            }
        }
    }

    impl From<CompareStateMessage> for MPCMessages {
        fn from(message: CompareStateMessage) -> Self {
            MPCMessages::LessThan(message)
        }
    }

    impl TryFrom<MPCMessages> for CompareStateMessage {
        type Error = Error;

        fn try_from(msg: MPCMessages) -> Result<Self, Self::Error> {
            let MPCMessages::LessThan(msg) = msg else {
                return Err(anyhow!("unknown instruction message"));
            };
            Ok(msg)
        }
    }
}

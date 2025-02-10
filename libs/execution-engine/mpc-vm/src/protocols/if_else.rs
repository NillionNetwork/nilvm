//! Implementation of the MPC protocols for the if-else operation

use crate::{protocols::MPCProtocol, requirements::RuntimeRequirementType, utils::into_mpc_protocol};
use jit_compiler::{
    bytecode2protocol::{errors::Bytecode2ProtocolError, Bytecode2Protocol, Bytecode2ProtocolContext, ProtocolFactory},
    if_else,
    models::{
        bytecode::IfElse as BytecodeIfElse,
        protocols::{memory::ProtocolAddress, ExecutionLine},
    },
};
use nada_value::NadaType;

if_else!(IfElse, "IFELSE", ExecutionLine::Online, RuntimeRequirementType);
into_mpc_protocol!(IfElse);
if_else!(IfElsePublicCond, "IFELSEPC", ExecutionLine::Local, RuntimeRequirementType);
into_mpc_protocol!(IfElsePublicCond);
if_else!(IfElsePublicBranches, "IFELSEPB", ExecutionLine::Local, RuntimeRequirementType);
into_mpc_protocol!(IfElsePublicBranches);

impl IfElse {
    /// Transforms a bytecode if_else into a protocol
    pub(crate) fn transform<F: ProtocolFactory<MPCProtocol>>(
        context: &mut Bytecode2ProtocolContext<MPCProtocol, F>,
        operation: &BytecodeIfElse,
    ) -> Result<MPCProtocol, Bytecode2ProtocolError> {
        let first_ty = context.bytecode.memory_element_type(operation.first)?;
        let second_ty = context.bytecode.memory_element_type(operation.second)?;
        let third_ty = context.bytecode.memory_element_type(operation.third)?;

        if first_ty.is_public() {
            let protocol_ty = if second_ty.is_public() && third_ty.is_public() {
                operation.ty.clone()
            } else {
                operation.ty.as_shamir_share()?
            };
            let protocol = IfElsePublicCond {
                address: ProtocolAddress::default(),
                cond: Bytecode2Protocol::adapted_protocol(context, operation.first, &NadaType::Boolean)?,
                left: Bytecode2Protocol::adapted_protocol(context, operation.second, &protocol_ty)?,
                right: Bytecode2Protocol::adapted_protocol(context, operation.third, &protocol_ty)?,
                ty: protocol_ty,
                source_ref_index: operation.source_ref_index,
            };
            Ok(protocol.into())
        } else if second_ty.is_public() && third_ty.is_public() {
            let protocol_ty = operation.ty.as_shamir_share()?;
            let protocol = IfElsePublicBranches {
                address: ProtocolAddress::default(),
                cond: Bytecode2Protocol::adapted_protocol(context, operation.first, &NadaType::ShamirShareBoolean)?,
                left: Bytecode2Protocol::adapted_protocol(context, operation.second, &protocol_ty)?,
                right: Bytecode2Protocol::adapted_protocol(context, operation.third, &protocol_ty)?,
                ty: protocol_ty,
                source_ref_index: operation.source_ref_index,
            };
            Ok(protocol.into())
        } else {
            let protocol_ty = operation.ty.as_shamir_share()?;
            let protocol = IfElse {
                address: ProtocolAddress::default(),
                cond: Bytecode2Protocol::adapted_protocol(context, operation.first, &NadaType::ShamirShareBoolean)?,
                left: Bytecode2Protocol::adapted_protocol(context, operation.second, &protocol_ty)?,
                right: Bytecode2Protocol::adapted_protocol(context, operation.third, &protocol_ty)?,
                ty: protocol_ty,
                source_ref_index: operation.source_ref_index,
            };
            Ok(protocol.into())
        }
    }
}

#[cfg(any(test, feature = "vm"))]
pub(crate) mod vm {
    use crate::{
        protocols::{IfElse, IfElsePublicBranches, IfElsePublicCond},
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
    use protocols::conditionals::if_else::{IfElseOperands, IfElseState, IfElseStateMessage};
    use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer};

    impl<T> Instruction<T> for IfElse
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
            let cond = context.read(self.cond)?;

            use nada_value::NadaType::*;

            let (left_type, left_value) = (left.to_type(), left.try_into_value()?);
            let (right_type, right_value) = (right.to_type(), right.try_into_value()?);
            let (cond_type, cond_value) = (cond.to_type(), cond.try_into_value()?);

            // If the condition is a secret boolean the result is always secret.
            match (&left_type, &right_type) {
                // Integers.
                (ty @ ShamirShareInteger, ShamirShareInteger)
                | (ty @ ShamirShareInteger, Integer)
                | (Integer, ty @ ShamirShareInteger)
                // Unsigned Integers.
                | (ty @ ShamirShareUnsignedInteger, ShamirShareUnsignedInteger)
                | (UnsignedInteger, ty @ ShamirShareUnsignedInteger)
                | (ty @ ShamirShareUnsignedInteger, UnsignedInteger) => {
                    let operands = IfElseOperands { cond: cond_value, left: left_value, right: right_value };

                    let (initial_state, messages) = IfElseState::new(vec![operands], context.secret_sharer())?;
                    Ok(InstructionResult::StateMachine {
                        state_machine: MPCInstructionRouter::IfElse(DefaultInstructionStateMachine::new(initial_state, ty.clone())),
                        messages: into_instruction_messages(messages),
                    })

                }
                (left, right) => Err(anyhow!(
                "unsupported operands for if else shares protocol: if {cond_type:?} then {left:?} else {right:?}"
            )),
            }
        }
    }

    impl From<IfElseStateMessage> for MPCMessages {
        fn from(message: IfElseStateMessage) -> Self {
            MPCMessages::IfElse(message)
        }
    }

    impl TryFrom<MPCMessages> for IfElseStateMessage {
        type Error = Error;

        fn try_from(msg: MPCMessages) -> Result<Self, Self::Error> {
            let MPCMessages::IfElse(msg) = msg else {
                return Err(anyhow!("unknown instruction message"));
            };
            Ok(msg)
        }
    }

    impl<T> Instruction<T> for IfElsePublicCond
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
            let cond = context.read(self.cond)?;
            let cond_type = cond.to_type();
            if !cond_type.is_public() {
                return Err(anyhow!("unsupported condition for if else with public condition: {cond_type:?}"));
            }

            let right = context.read(self.right)?;
            let left = context.read(self.left)?;

            use nada_value::NadaType::*;

            let (left_type, left_value) = (left.to_type(), left.try_into_value()?);
            let (right_type, right_value) = (right.to_type(), right.try_into_value()?);
            let cond_value = cond.try_into_value()?;

            // If the condition is a public boolean, the two operands can be:
            //  * both public. In this case, the result is public.
            //  * both secret. In this case, the result is secret.
            //  * one public one secret. In this case, the result is secret. This
            //    case should be carefully handled as in the case that the branch
            //    with the public value is taken, then the result secret is known.
            let result = if cond_value != ModularNumber::ZERO { left_value } else { right_value };

            match (left_type, right_type) {
                // Both branches are public
                (Integer, Integer) => Ok(InstructionResult::Value { value: NadaValue::new_integer(result) }),
                (UnsignedInteger, UnsignedInteger) => {
                    // Comment for formatting.
                    Ok(InstructionResult::Value { value: NadaValue::new_unsigned_integer(result) })
                }
                // Both branches are secret or one is secret and the other is public
                (ShamirShareInteger, ShamirShareInteger)
                | (ShamirShareInteger, Integer)
                | (Integer, ShamirShareInteger) => {
                    Ok(InstructionResult::Value { value: NadaValue::new_shamir_share_integer(result) })
                }
                (ShamirShareUnsignedInteger, ShamirShareUnsignedInteger)
                | (UnsignedInteger, ShamirShareUnsignedInteger)
                | (ShamirShareUnsignedInteger, UnsignedInteger) => {
                    // Comment for formatting.
                    Ok(InstructionResult::Value { value: NadaValue::new_shamir_share_unsigned_integer(result) })
                }
                (left, right) => Err(anyhow!(
                    "unsupported operands for if else shares protocol: if {cond_type:?} then {left:?} else {right:?}"
                )),
            }
        }
    }

    impl<T> Instruction<T> for IfElsePublicBranches
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        type PreprocessingElement = MPCProtocolPreprocessingElements<T>;
        type Router = MPCInstructionRouter<T>;
        type Message = MPCMessages;

        #[allow(clippy::arithmetic_side_effects)]
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
            let cond = context.read(self.cond)?;

            use nada_value::NadaType::*;

            let (left_type, left_value) = (left.to_type(), left.try_into_value()?);
            let (right_type, right_value) = (right.to_type(), right.try_into_value()?);
            let (cond_type, cond_value) = (cond.to_type(), cond.try_into_value()?);

            // If the condition is a secret boolean the result is always secret.
            match (&left_type, &right_type) {
                // Both branches are public
                (Integer, Integer) | (UnsignedInteger, UnsignedInteger) => {
                    let result = cond_value * &left_value + &((ModularNumber::ONE - &cond_value) * &right_value);
                    if left_type.is_integer() {
                        Ok(InstructionResult::Value { value: NadaValue::new_shamir_share_integer(result) })
                    } else {
                        Ok(InstructionResult::Value { value: NadaValue::new_shamir_share_unsigned_integer(result) })
                    }
                }
                (left, right) => Err(anyhow!(
                    "unsupported operands for if else public protocol: if {cond_type:?} then {left:?} else {right:?}"
                )),
            }
        }
    }
}

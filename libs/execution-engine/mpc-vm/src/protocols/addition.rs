//! Implementation of the MPC protocols for the addition operation

use crate::{protocols::MPCProtocol, requirements::RuntimeRequirementType, utils::into_mpc_protocol};
use jit_compiler::{
    binary_protocol,
    bytecode2protocol::{errors::Bytecode2ProtocolError, Bytecode2ProtocolContext, ProtocolFactory},
    models::{bytecode::Addition as BytecodeAddition, protocols::ExecutionLine},
    public_binary_protocol, share_binary_protocol,
};
use nada_value::{NadaPrimitiveType, NadaTypeMetadata};

binary_protocol!(Addition, "ADD", ExecutionLine::Local, RuntimeRequirementType);
into_mpc_protocol!(Addition);

impl Addition {
    public_binary_protocol!(BytecodeAddition);
    share_binary_protocol!(BytecodeAddition);

    /// Transforms a bytecode addition into a protocol
    pub(crate) fn transform<F: ProtocolFactory<MPCProtocol>>(
        context: &mut Bytecode2ProtocolContext<MPCProtocol, F>,
        operation: &BytecodeAddition,
    ) -> Result<MPCProtocol, Bytecode2ProtocolError> {
        let left_type = context.bytecode.memory_element_type(operation.left)?;
        let left_metadata: NadaTypeMetadata = left_type.into();
        let right_type = context.bytecode.memory_element_type(operation.right)?;
        let right_metadata: NadaTypeMetadata = right_type.into();

        // Check the primitive types of the operands match and are Integer or UnsignedInteger
        match (left_metadata.nada_primitive_type(), right_metadata.nada_primitive_type()) {
            (Some(NadaPrimitiveType::Integer), Some(NadaPrimitiveType::Integer))
            | (Some(NadaPrimitiveType::UnsignedInteger), Some(NadaPrimitiveType::UnsignedInteger))
            | (Some(NadaPrimitiveType::Boolean), Some(NadaPrimitiveType::Boolean)) => {}
            _ => {
                return Err(Bytecode2ProtocolError::OperationNotSupported(format!(
                    "type {} + {} not supported",
                    left_type, right_type
                )));
            }
        };

        if left_type.is_public() && right_type.is_public() {
            // If both operands are public, the result is public
            Self::public_protocol(context, operation)
        } else {
            // Otherwise the result is a share
            Self::share_protocol(context, operation)
        }
    }
}

#[cfg(any(test, feature = "vm"))]
pub mod vm {
    use crate::{
        protocols::Addition,
        vm::{plan::MPCProtocolPreprocessingElements, MPCInstructionRouter, MPCMessages},
    };
    use anyhow::{anyhow, Error};
    use execution_engine_vm::vm::{
        instructions::{Instruction, InstructionResult},
        memory::MemoryValue,
        sm::ExecutionContext,
    };
    use math_lib::modular::SafePrime;
    use nada_value::NadaValue;
    use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer};

    impl<T> Instruction<T> for Addition
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

            use nada_value::NadaType::*;

            let (left_type, left_value) = (left.to_type(), left.try_into_value()?);
            let (right_type, right_value) = (right.to_type(), right.try_into_value()?);

            //  * Both left and right are public, the result is public.
            //  * Otherwise, the result is secret
            let result = left_value + &right_value;
            match (&left_type, &right_type) {
                // Both are public
                (Integer, Integer) => Ok(InstructionResult::Value { value: NadaValue::new_integer(result) }),
                (UnsignedInteger, UnsignedInteger) => {
                    Ok(InstructionResult::Value { value: NadaValue::new_unsigned_integer(result) })
                }
                (Boolean, Boolean) => Ok(InstructionResult::Value { value: NadaValue::new_boolean(result) }),
                // Integers.
                (ShamirShareInteger, ShamirShareInteger)
                | (ShamirShareInteger, Integer)
                | (Integer, ShamirShareInteger) => {
                    Ok(InstructionResult::Value { value: NadaValue::new_shamir_share_integer(result) })
                }
                // Unsigned Integers.
                (ShamirShareUnsignedInteger, ShamirShareUnsignedInteger)
                | (UnsignedInteger, ShamirShareUnsignedInteger)
                | (ShamirShareUnsignedInteger, UnsignedInteger) => {
                    Ok(InstructionResult::Value { value: NadaValue::new_shamir_share_unsigned_integer(result) })
                }
                // Booleans.
                (ShamirShareBoolean, ShamirShareBoolean)
                | (ShamirShareBoolean, Boolean)
                | (Boolean, ShamirShareBoolean) => {
                    Ok(InstructionResult::Value { value: NadaValue::new_shamir_share_boolean(result) })
                }
                (left, right) => {
                    Err(anyhow!("unsupported operands for addition of shares protocol: {left:?} + {right:?}"))
                }
            }
        }
    }
}

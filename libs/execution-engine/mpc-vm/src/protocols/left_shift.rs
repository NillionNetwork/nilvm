//! Implementation of the MPC protocols for the left-shift operation

use crate::{protocols::MPCProtocol, requirements::RuntimeRequirementType, utils::into_mpc_protocol};
use jit_compiler::{
    binary_protocol,
    bytecode2protocol::{errors::Bytecode2ProtocolError, Bytecode2ProtocolContext, ProtocolFactory},
    models::{bytecode::LeftShift as BytecodeLeftShift, protocols::ExecutionLine},
    public_shift_protocol, share_shift_protocol,
};

pub(crate) struct LeftShift;

impl LeftShift {
    /// Transforms a bytecode left shift into a protocol
    pub(crate) fn transform<F: ProtocolFactory<MPCProtocol>>(
        context: &mut Bytecode2ProtocolContext<MPCProtocol, F>,
        operation: &BytecodeLeftShift,
    ) -> Result<MPCProtocol, Bytecode2ProtocolError> {
        // Check types
        let right_type = context.bytecode.memory_element_type(operation.right)?;
        if !right_type.is_public() {
            return Err(Bytecode2ProtocolError::OperationNotSupported(format!(
                "The amount of shift should be public. {} not supported",
                right_type
            )));
        }

        let left_type = context.bytecode.memory_element_type(operation.left)?;

        if left_type.is_public() {
            LeftShiftPublic::public_protocol(context, operation)
        } else {
            LeftShiftShares::share_protocol(context, operation)
        }
    }
}

binary_protocol!(LeftShiftPublic, "SHFTC", ExecutionLine::Local, RuntimeRequirementType);
into_mpc_protocol!(LeftShiftPublic);
impl LeftShiftPublic {
    public_shift_protocol!(BytecodeLeftShift);
}

binary_protocol!(LeftShiftShares, "SHFTS", ExecutionLine::Local, RuntimeRequirementType);
into_mpc_protocol!(LeftShiftShares);
impl LeftShiftShares {
    share_shift_protocol!(BytecodeLeftShift);
}

#[cfg(any(test, feature = "vm"))]
pub(crate) mod vm {
    use crate::{
        protocols::{LeftShiftPublic, LeftShiftShares},
        vm::{plan::MPCProtocolPreprocessingElements, MPCInstructionRouter, MPCMessages},
    };
    use anyhow::{anyhow, Error};
    use execution_engine_vm::vm::{
        instructions::{Instruction, InstructionResult},
        memory::MemoryValue,
        sm::ExecutionContext,
    };
    use math_lib::modular::{ModularNumber, ModularPow, SafePrime};
    use nada_value::NadaValue;

    use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer};

    impl<T> Instruction<T> for LeftShiftPublic
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
            let (left_type, left) = (left.to_type(), left.try_into_value()?);
            let (right_type, right) = (right.to_type(), right.try_into_value()?);
            let result = left * &ModularNumber::two().exp_mod(&right.into_value());

            match (left_type, right_type) {
                (Integer, UnsignedInteger) => Ok(InstructionResult::Value { value: NadaValue::new_integer(result) }),
                (UnsignedInteger, UnsignedInteger) => {
                    Ok(InstructionResult::Value { value: NadaValue::new_unsigned_integer(result) })
                }
                (left, right) => Err(anyhow!("unsupported operands for left shift protocol: {left:?} << {right:?}")),
            }
        }
    }

    impl<T> Instruction<T> for LeftShiftShares
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
            let (left_type, left) = (left.to_type(), left.try_into_value()?);
            let (right_type, right) = (right.to_type(), right.try_into_value()?);
            let result = left * &ModularNumber::two().exp_mod(&right.into_value());

            match (left_type, right_type) {
                (ShamirShareInteger, UnsignedInteger) => {
                    Ok(InstructionResult::Value { value: NadaValue::new_shamir_share_integer(result) })
                }
                (ShamirShareUnsignedInteger, UnsignedInteger) => {
                    Ok(InstructionResult::Value { value: NadaValue::new_shamir_share_unsigned_integer(result) })
                }
                (left, right) => Err(anyhow!("unsupported operands for left shift protocol: {left:?} << {right:?}")),
            }
        }
    }
}

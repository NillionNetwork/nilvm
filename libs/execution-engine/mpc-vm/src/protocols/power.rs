//! Implementation of the MPC protocols for the power operation

use crate::{protocols::MPCProtocol, requirements::RuntimeRequirementType, utils::into_mpc_protocol};
use jit_compiler::{
    binary_protocol,
    bytecode2protocol::{errors::Bytecode2ProtocolError, Bytecode2ProtocolContext, ProtocolFactory},
    models::{bytecode::Power as BytecodePower, protocols::ExecutionLine},
    public_binary_protocol,
};

pub(crate) struct Power;
impl Power {
    /// Transforms a bytecode power into a protocol
    pub(crate) fn transform<F: ProtocolFactory<MPCProtocol>>(
        context: &mut Bytecode2ProtocolContext<MPCProtocol, F>,
        operation: &BytecodePower,
    ) -> Result<MPCProtocol, Bytecode2ProtocolError> {
        // Check types
        let exponent_type = context.bytecode.memory_element_type(operation.right)?;
        if !exponent_type.is_public() {
            return Err(Bytecode2ProtocolError::OperationNotSupported("secret power exponent".to_string()));
        }

        let base_type = context.bytecode.memory_element_type(operation.left)?;
        if base_type.is_public() {
            PowerPublicBasePublicExponent::public_protocol(context, operation)
        } else {
            Err(Bytecode2ProtocolError::OperationNotSupported("power with secret base".to_string()))
        }
    }
}

binary_protocol!(PowerPublicBasePublicExponent, "POWC", ExecutionLine::Local, RuntimeRequirementType);
into_mpc_protocol!(PowerPublicBasePublicExponent);
impl PowerPublicBasePublicExponent {
    public_binary_protocol!(BytecodePower);
}

#[cfg(any(test, feature = "vm"))]
pub(crate) mod vm {
    use crate::{
        protocols::PowerPublicBasePublicExponent,
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

    impl<T> Instruction<T> for PowerPublicBasePublicExponent
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        type PreprocessingElement = MPCProtocolPreprocessingElements<T>;
        type Router = MPCInstructionRouter<T>;
        type Message = MPCMessages;

        /// Public base and public exponent: use exp_mod.
        fn run<F>(
            self,
            context: &mut ExecutionContext<F, T>,
            _: Self::PreprocessingElement,
        ) -> Result<InstructionResult<Self::Router, T>, Error>
        where
            F: Instruction<T>,
        {
            let exponent = context.read(self.right)?;
            let base = context.read(self.left)?;

            let exponent_type = exponent.to_type();
            let exponent = exponent.try_into_value()?;
            if exponent < ModularNumber::ZERO {
                return Err(anyhow!("power with negative exponent"));
            }

            let base_type = base.to_type();
            let base = base.try_into_value()?;

            use nada_value::NadaType::*;
            match (base_type, exponent_type) {
                (Integer, Integer) => {
                    let result = base.exp_mod(&exponent.into_value());
                    Ok(InstructionResult::Value { value: NadaValue::new_integer(result) })
                }
                (UnsignedInteger, UnsignedInteger) => {
                    let result = base.exp_mod(&exponent.into_value());
                    Ok(InstructionResult::Value { value: NadaValue::new_unsigned_integer(result) })
                }
                (base, exponent) => {
                    Err(anyhow!("unsupported operands for power public protocol: {base:?} ^ {exponent:?}"))
                }
            }
        }
    }
}

//! Implementation of the MPC protocols for the not operation

use crate::{protocols::MPCProtocol, requirements::RuntimeRequirementType, utils::into_mpc_protocol};
use jit_compiler::{
    bytecode2protocol::{errors::Bytecode2ProtocolError, Bytecode2Protocol, Bytecode2ProtocolContext, ProtocolFactory},
    models::{
        bytecode::Not as BytecodeNot,
        protocols::{memory::ProtocolAddress, ExecutionLine},
    },
    unary_protocol,
};
use nada_value::NadaType;

// TODO We should have different variants for this protocol (Public and Shares)
unary_protocol!(Not, "NOT", ExecutionLine::Local, RuntimeRequirementType);
into_mpc_protocol!(Not);

impl Not {
    pub(crate) fn transform<F: ProtocolFactory<MPCProtocol>>(
        context: &mut Bytecode2ProtocolContext<MPCProtocol, F>,
        operation: &BytecodeNot,
    ) -> Result<MPCProtocol, Bytecode2ProtocolError> {
        let operand_ty = context.bytecode.memory_element_type(operation.operand)?;
        match operand_ty {
            NadaType::Boolean => {
                let protocol = Self {
                    address: ProtocolAddress::default(),
                    operand: Bytecode2Protocol::adapted_protocol(context, operation.operand, &NadaType::Boolean)?,
                    ty: NadaType::Boolean,
                    source_ref_index: operation.source_ref_index,
                };
                Ok(protocol.into())
            }
            NadaType::SecretBoolean | NadaType::ShamirShareBoolean => {
                let expected_type = NadaType::ShamirShareBoolean;
                let protocol = Self {
                    address: ProtocolAddress::default(),
                    operand: Bytecode2Protocol::adapted_protocol(context, operation.operand, &expected_type)?,
                    ty: expected_type,
                    source_ref_index: operation.source_ref_index,
                };
                Ok(protocol.into())
            }
            _ => {
                let msg = format!("not {operand_ty} is not supported");
                Err(Bytecode2ProtocolError::OperationNotSupported(msg))
            }
        }
    }
}

#[cfg(any(test, feature = "vm"))]
pub(crate) mod vm {
    use crate::{
        protocols::Not,
        vm::{plan::MPCProtocolPreprocessingElements, MPCInstructionRouter, MPCMessages},
    };
    use anyhow::{anyhow, Error};
    use execution_engine_vm::vm::{
        instructions::{Instruction, InstructionResult},
        memory::MemoryValue,
        sm::ExecutionContext,
    };
    use math_lib::modular::{ModularNumber, SafePrime};
    use nada_value::NadaValue;
    use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer};

    impl<T> Instruction<T> for Not
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
            // Note: currently the NOT operation is only used internally for the greater-or-equal and
            // less-or-equal operations. Since their output is always binary, the variable 'not_operand'
            // does not overflow. However, for future use of NOT, we have to ensure the value under the shares
            // of 'operand' is only binary.
            let operand = context.read(self.operand)?;

            use nada_value::NadaType::*;
            match operand.to_type() {
                Boolean => {
                    let operand = operand.try_into_value()?;
                    let value = NadaValue::new_boolean(ModularNumber::ONE - &operand); // negation operation
                    Ok(InstructionResult::Value { value })
                }
                ShamirShareBoolean => {
                    let operand = operand.try_into_value()?;
                    let value = NadaValue::new_shamir_share_boolean(ModularNumber::ONE - &operand); // negation operation
                    Ok(InstructionResult::Value { value })
                }
                operand => Err(anyhow!("unsupported operands for Not protocol: !{operand:?}")),
            }
        }
    }
}

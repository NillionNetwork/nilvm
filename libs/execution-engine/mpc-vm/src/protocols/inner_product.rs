//! Implementation of the MPC protocols for the inner product operation

use crate::{protocols::MPCProtocol, requirements::RuntimeRequirementType, utils::into_mpc_protocol};
use jit_compiler::{
    binary_protocol,
    bytecode2protocol::{errors::Bytecode2ProtocolError, Bytecode2Protocol, Bytecode2ProtocolContext, ProtocolFactory},
    models::{
        bytecode::InnerProduct as BytecodeInnerProduct,
        protocols::{memory::ProtocolAddress, ExecutionLine},
    },
};
use nada_compiler_backend::mir::TypedElement;

pub(crate) struct InnerProduct;

impl InnerProduct {
    /// Transforms a bytecode inner product into a protocol
    pub(crate) fn transform<F: ProtocolFactory<MPCProtocol>>(
        context: &mut Bytecode2ProtocolContext<MPCProtocol, F>,
        operation: &BytecodeInnerProduct,
    ) -> Result<MPCProtocol, Bytecode2ProtocolError> {
        let left_type = context.bytecode.memory_element_type(operation.left)?;
        let right_type = context.bytecode.memory_element_type(operation.right)?;
        if left_type.is_public() && right_type.is_public() {
            // If both operands are public, the result is public
            InnerProductPublic::public_protocol(context, operation)
        } else if left_type.is_public() || right_type.is_public() {
            InnerProductSharePublic::share_protocol(context, operation)
        } else {
            // Otherwise the result is a share
            InnerProductShares::share_protocol(context, operation)
        }
    }
}

binary_protocol!(InnerProductShares, "INNPS", ExecutionLine::Online, RuntimeRequirementType);
into_mpc_protocol!(InnerProductShares);
impl InnerProductShares {
    pub(crate) fn share_protocol<F: ProtocolFactory<MPCProtocol>>(
        context: &mut Bytecode2ProtocolContext<MPCProtocol, F>,
        operation: &BytecodeInnerProduct,
    ) -> Result<MPCProtocol, Bytecode2ProtocolError> {
        let left_operation = context
            .bytecode
            .operation(operation.left)?
            .ok_or(Bytecode2ProtocolError::OperationNotFound(operation.left))?;

        let right_operation = context
            .bytecode
            .operation(operation.right)?
            .ok_or(Bytecode2ProtocolError::OperationNotFound(operation.right))?;
        let expected_type = operation.ty.as_shamir_share()?;
        let left =
            Bytecode2Protocol::adapted_protocol(context, operation.left, &left_operation.ty().as_shamir_share()?)?;
        let right =
            Bytecode2Protocol::adapted_protocol(context, operation.right, &right_operation.ty().as_shamir_share()?)?;
        let protocol = Self {
            address: ProtocolAddress::default(),
            left,
            right,
            ty: expected_type,
            source_ref_index: operation.source_ref_index,
        };
        Ok(protocol.into())
    }
}

binary_protocol!(InnerProductPublic, "INNPC", ExecutionLine::Local, RuntimeRequirementType);
into_mpc_protocol!(InnerProductPublic);
impl InnerProductPublic {
    pub(crate) fn public_protocol<F: ProtocolFactory<MPCProtocol>>(
        context: &mut Bytecode2ProtocolContext<MPCProtocol, F>,
        operation: &BytecodeInnerProduct,
    ) -> Result<MPCProtocol, Bytecode2ProtocolError> {
        let expected_type = operation.ty.as_public()?;
        let left_operation = context
            .bytecode
            .operation(operation.left)?
            .ok_or(Bytecode2ProtocolError::OperationNotFound(operation.left))?;

        let right_operation = context
            .bytecode
            .operation(operation.right)?
            .ok_or(Bytecode2ProtocolError::OperationNotFound(operation.right))?;

        let left = Bytecode2Protocol::adapted_protocol(context, operation.left, &left_operation.ty().as_public()?)?;
        let right = Bytecode2Protocol::adapted_protocol(context, operation.right, &right_operation.ty().as_public()?)?;

        let protocol = Self {
            address: ProtocolAddress::default(),
            left,
            right,
            ty: expected_type,
            source_ref_index: operation.source_ref_index,
        };
        Ok(protocol.into())
    }
}

binary_protocol!(InnerProductSharePublic, "INNPM", ExecutionLine::Local, RuntimeRequirementType);
into_mpc_protocol!(InnerProductSharePublic);
impl InnerProductSharePublic {
    pub(crate) fn share_protocol<F: ProtocolFactory<MPCProtocol>>(
        context: &mut Bytecode2ProtocolContext<MPCProtocol, F>,
        operation: &BytecodeInnerProduct,
    ) -> Result<MPCProtocol, Bytecode2ProtocolError> {
        let left_operation = context
            .bytecode
            .operation(operation.left)?
            .ok_or(Bytecode2ProtocolError::OperationNotFound(operation.left))?;

        let right_operation = context
            .bytecode
            .operation(operation.right)?
            .ok_or(Bytecode2ProtocolError::OperationNotFound(operation.right))?;
        let expected_type = operation.ty.as_shamir_share()?;
        let left_operation_type =
            if left_operation.ty().is_secret() { &left_operation.ty().as_shamir_share()? } else { left_operation.ty() };
        let right_operation_type = if right_operation.ty().is_secret() {
            &right_operation.ty().as_shamir_share()?
        } else {
            right_operation.ty()
        };
        let left = Bytecode2Protocol::adapted_protocol(context, operation.left, left_operation_type)?;
        let right = Bytecode2Protocol::adapted_protocol(context, operation.right, right_operation_type)?;
        let protocol = Self {
            address: ProtocolAddress::default(),
            left,
            right,
            ty: expected_type,
            source_ref_index: operation.source_ref_index,
        };
        Ok(protocol.into())
    }
}

#[cfg(any(test, feature = "vm"))]
pub(crate) mod vm {
    use crate::{
        protocols::{InnerProductPublic, InnerProductSharePublic, InnerProductShares},
        vm::{plan::MPCProtocolPreprocessingElements, MPCInstructionRouter, MPCMessages},
    };
    use anyhow::{anyhow, Error};
    use execution_engine_vm::vm::{
        instructions::{into_instruction_messages, DefaultInstructionStateMachine, Instruction, InstructionResult},
        memory::MemoryValue,
        sm::ExecutionContext,
    };
    use jit_compiler::models::protocols::memory::ProtocolAddress;
    use math_lib::modular::{ModularNumber, SafePrime};
    use nada_value::{NadaType, NadaValue};
    use protocols::multiplication::multiplication_shares::{MultState, OperandShares};
    use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer};

    /// Retrieves all the array values from memory
    fn array_content<I, T>(
        context: &mut ExecutionContext<I, T>,
        addr: ProtocolAddress,
    ) -> Result<Vec<ModularNumber<T>>, Error>
    where
        I: Instruction<T>,
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        match context.memory.read_value(addr)? {
            NadaValue::Array { values, .. } => values.into_iter().map(|v| v.try_into_value()).collect(),
            value => {
                let ty = value.to_type();
                Err(anyhow!("input type is not supported: expected array, found {ty:?}"))
            }
        }
    }

    enum InnerProductType {
        Secret,
        Public,
        Mixed,
    }

    fn check_inner_types(
        lhs_inner_type: &NadaType,
        rhs_inner_type: &NadaType,
        check_type: InnerProductType,
    ) -> Result<(), Error> {
        use NadaType::*;
        let check_result = match check_type {
            InnerProductType::Secret => matches!(
                (lhs_inner_type, rhs_inner_type),
                (ShamirShareInteger, ShamirShareInteger) | (ShamirShareUnsignedInteger, ShamirShareUnsignedInteger)
            ),

            InnerProductType::Public => {
                matches!((lhs_inner_type, rhs_inner_type), (Integer, Integer) | (UnsignedInteger, UnsignedInteger))
            }
            InnerProductType::Mixed => matches!(
                (lhs_inner_type, rhs_inner_type),
                (Integer, ShamirShareInteger)
                    | (UnsignedInteger, ShamirShareUnsignedInteger)
                    | (ShamirShareInteger, Integer)
                    | (ShamirShareUnsignedInteger, UnsignedInteger)
            ),
        };
        if check_result {
            Ok(())
        } else {
            Err(anyhow!("unsupported operands for inner product protocol: {lhs_inner_type:?} (*) {rhs_inner_type:?}"))
        }
    }

    fn type_check_inputs(lhs: &NadaType, rhs: &NadaType, check_type: InnerProductType) -> Result<(), Error> {
        use NadaType::*;
        match (lhs, rhs) {
            (
                Array { size: lhs_size, inner_type: lhs_inner_type },
                Array { size: rhs_size, inner_type: rhs_inner_type },
            ) => {
                if lhs_size != rhs_size {
                    return Err(anyhow!("only same size arrays are valid for inner product"));
                }
                check_inner_types(lhs_inner_type, rhs_inner_type, check_type)?;
                Ok(())
            }
            (lhs, rhs) => Err(anyhow!("unsupported operands for inner product protocol: {lhs:?} (*) {rhs:?}")),
        }
    }

    impl<T> Instruction<T> for InnerProductShares
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
            let right = context.memory.runtime_memory_type(self.right)?;
            let left = context.memory.runtime_memory_type(self.left)?;
            type_check_inputs(&left, &right, InnerProductType::Secret)?;
            use nada_value::NadaType::*;
            match (left, right) {
                (Array { inner_type, .. }, Array { .. }) => {
                    let left_addr = self.left;
                    let right_addr = self.right;
                    let left_values = array_content(context, left_addr)?;
                    let right_values = array_content(context, right_addr)?;

                    let operands = OperandShares { left: left_values, right: right_values };
                    let (initial_state, messages) = MultState::new(vec![operands], context.secret_sharer())?;

                    Ok(InstructionResult::StateMachine {
                        state_machine: MPCInstructionRouter::Multiplication(DefaultInstructionStateMachine::new(
                            initial_state,
                            *inner_type,
                        )),
                        messages: into_instruction_messages(messages),
                    })
                }
                (left, right) => {
                    Err(anyhow!("unsupported operands for inner product protocol: {left:?} (*) {right:?}"))
                }
            }
        }
    }

    impl<T> Instruction<T> for InnerProductPublic
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
            let right = context.memory.runtime_memory_type(self.right)?;
            let left = context.memory.runtime_memory_type(self.left)?;
            type_check_inputs(&left, &right, InnerProductType::Public)?;
            use nada_value::NadaType::*;
            match (left, right) {
                (Array { inner_type, .. }, Array { .. }) => {
                    let left_addr = self.left;
                    let right_addr = self.right;
                    let left_values = array_content(context, left_addr)?;
                    let right_values = array_content(context, right_addr)?;
                    let result = left_values
                        .into_iter()
                        .zip(right_values)
                        .map(|(lhs, rhs)| lhs * &rhs)
                        .fold(ModularNumber::ZERO, |acc, product| acc + &product);
                    match *inner_type {
                        Integer => Ok(InstructionResult::Value { value: NadaValue::new_integer(result) }),
                        UnsignedInteger => {
                            Ok(InstructionResult::Value { value: NadaValue::new_unsigned_integer(result) })
                        }
                        _ => Err(anyhow!("invalid inner type for public inner product")),
                    }
                }
                (left, right) => {
                    Err(anyhow!("unsupported operands for inner product protocol: {left:?} (*) {right:?}"))
                }
            }
        }
    }

    impl<T> Instruction<T> for InnerProductSharePublic
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
            let right = context.memory.runtime_memory_type(self.right)?;
            let left = context.memory.runtime_memory_type(self.left)?;
            type_check_inputs(&left, &right, InnerProductType::Mixed)?;
            use nada_value::NadaType::*;
            match (left, right) {
                (Array { inner_type, .. }, Array { inner_type: rhs_inner_type, .. }) => {
                    let left_addr = self.left;
                    let right_addr = self.right;
                    let left_values = array_content(context, left_addr)?;
                    let right_values = array_content(context, right_addr)?;
                    let result = left_values
                        .into_iter()
                        .zip(right_values)
                        .map(|(lhs, rhs)| lhs * &rhs)
                        .fold(ModularNumber::ZERO, |acc, product| acc + &product);
                    match (*inner_type.clone(), *rhs_inner_type.clone()) {
                        (ShamirShareInteger, Integer) | (Integer, ShamirShareInteger) => {
                            Ok(InstructionResult::Value { value: NadaValue::new_shamir_share_integer(result) })
                        }
                        (ShamirShareUnsignedInteger, UnsignedInteger)
                        | (UnsignedInteger, ShamirShareUnsignedInteger) => {
                            Ok(InstructionResult::Value { value: NadaValue::new_shamir_share_unsigned_integer(result) })
                        }
                        _ => Err(anyhow!(
                            "invalid inner types for public inner product {inner_type:?} - {rhs_inner_type:?}"
                        )),
                    }
                }
                (left, right) => {
                    Err(anyhow!("unsupported operands for inner product protocol: {left:?} (*) {right:?}"))
                }
            }
        }
    }
}

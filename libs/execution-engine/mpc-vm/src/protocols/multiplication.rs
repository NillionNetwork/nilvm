//! Implementation of the MPC protocols for the multiplication operation
use crate::{protocols::MPCProtocol, requirements::RuntimeRequirementType, utils::into_mpc_protocol};
use jit_compiler::{
    binary_protocol,
    bytecode2protocol::{errors::Bytecode2ProtocolError, Bytecode2ProtocolContext, ProtocolFactory},
    models::{bytecode::Multiplication as BytecodeMultiplication, protocols::ExecutionLine},
    public_binary_protocol, share_binary_protocol,
};
use nada_value::{NadaPrimitiveType, NadaTypeMetadata};

pub(crate) struct Multiplication;

impl Multiplication {
    /// Transforms a bytecode multiplication into a protocol
    pub(crate) fn transform<F: ProtocolFactory<MPCProtocol>>(
        context: &mut Bytecode2ProtocolContext<MPCProtocol, F>,
        operation: &BytecodeMultiplication,
    ) -> Result<MPCProtocol, Bytecode2ProtocolError> {
        let left_type = context.bytecode.memory_element_type(operation.left)?;
        let left_metadata: NadaTypeMetadata = left_type.into();
        let right_type = context.bytecode.memory_element_type(operation.right)?;
        let right_metadata: NadaTypeMetadata = right_type.into();

        // Check the primitive types of the operands match and are Integer, UnsignedInteger or Boolean
        match (left_metadata.nada_primitive_type(), right_metadata.nada_primitive_type()) {
            (Some(NadaPrimitiveType::Integer), Some(NadaPrimitiveType::Integer))
            | (Some(NadaPrimitiveType::UnsignedInteger), Some(NadaPrimitiveType::UnsignedInteger))
            | (Some(NadaPrimitiveType::Boolean), Some(NadaPrimitiveType::Boolean)) => {}
            _ => {
                return Err(Bytecode2ProtocolError::OperationNotSupported(format!(
                    "type {} * {} not supported",
                    left_type, right_type
                )));
            }
        };

        if left_type.is_public() && right_type.is_public() {
            // If both operands are public, the result is public
            MultiplicationPublic::public_protocol(context, operation)
        } else if left_type.is_public() || right_type.is_public() {
            MultiplicationSharePublic::share_protocol(context, operation)
        } else {
            MultiplicationShares::share_protocol(context, operation)
        }
    }
}

binary_protocol!(MultiplicationPublic, "MULC", ExecutionLine::Local, RuntimeRequirementType);
into_mpc_protocol!(MultiplicationPublic);
impl MultiplicationPublic {
    public_binary_protocol!(BytecodeMultiplication);
}

binary_protocol!(MultiplicationShares, "MULS", ExecutionLine::Online, RuntimeRequirementType);
into_mpc_protocol!(MultiplicationShares);
impl MultiplicationShares {
    share_binary_protocol!(BytecodeMultiplication);
}

binary_protocol!(MultiplicationSharePublic, "MULSP", ExecutionLine::Local, RuntimeRequirementType);
into_mpc_protocol!(MultiplicationSharePublic);
impl MultiplicationSharePublic {
    share_binary_protocol!(BytecodeMultiplication);
}

#[cfg(any(test, feature = "vm"))]
#[cfg(test)]
mod tests {
    use crate::{tests::compile_bytecode, MPCProtocol, MPCProtocolFactory};
    use anyhow::Error;
    use itertools::Itertools;
    use jit_compiler::{
        bytecode2protocol::Bytecode2Protocol,
        models::{
            bytecode::{memory::BytecodeAddress, Operation},
            memory::AddressType,
        },
    };

    #[test]
    #[allow(clippy::indexing_slicing)]
    /// (a + b) * c
    fn multiplication_of_shares_1() -> Result<(), Error> {
        let bytecode = compile_bytecode("multiplication_of_shares_1")?;

        // Load A
        let address = BytecodeAddress::new(0, AddressType::Heap);
        assert!(matches!(&bytecode.operation(address)?.unwrap(), Operation::Load(_)));
        assert!(bytecode.memory_element_type(address)?.is_secret_integer());

        // Load B
        let address = BytecodeAddress::new(1, AddressType::Heap);
        assert!(matches!(&bytecode.operation(address)?.unwrap(), Operation::Load(_)));
        assert!(bytecode.memory_element_type(address)?.is_secret_integer());

        // Load C
        let address = BytecodeAddress::new(2, AddressType::Heap);
        assert!(matches!(&bytecode.operation(address)?.unwrap(), Operation::Load(_)));
        assert!(bytecode.memory_element_type(address)?.is_secret_integer());

        // A + B
        let address = BytecodeAddress::new(3, AddressType::Heap);
        assert!(matches!(&bytecode.operation(address)?.unwrap(), Operation::Addition(_)));
        assert!(bytecode.memory_element_type(address)?.is_secret_integer());

        // (A + B) * C
        let address = BytecodeAddress::new(4, AddressType::Heap);
        assert!(matches!(&bytecode.operation(address)?.unwrap(), Operation::Multiplication(_)));
        assert!(bytecode.memory_element_type(address)?.is_secret_integer());

        let body = Bytecode2Protocol::transform(MPCProtocolFactory, &bytecode)?;
        // The input memory should contain 3 elements
        assert_eq!(3, body.input_memory_scheme.len());

        let protocols = body.protocols.values().collect_vec();
        assert_eq!(protocols.len(), 2);

        assert!(matches!(protocols[0], MPCProtocol::Addition(_))); // a + b
        assert!(matches!(protocols[1], MPCProtocol::MultiplicationShares(_)), "actual: {}", protocols[1]); // (a + b) * c
        Ok(())
    }

    #[test]
    #[allow(clippy::indexing_slicing)]
    /// a * (b + c)
    fn multiplication_of_shares_2() -> Result<(), Error> {
        let bytecode = compile_bytecode("multiplication_of_shares_2")?;

        // Load A
        let address = BytecodeAddress::new(0, AddressType::Heap);
        assert!(matches!(&bytecode.operation(address)?.unwrap(), Operation::Load(_)));
        assert!(bytecode.memory_element_type(address)?.is_secret_integer());

        // Load B
        let address = BytecodeAddress::new(1, AddressType::Heap);
        assert!(matches!(&bytecode.operation(address)?.unwrap(), Operation::Load(_)));
        assert!(bytecode.memory_element_type(address)?.is_secret_integer());

        // Load C
        let address = BytecodeAddress::new(2, AddressType::Heap);
        assert!(matches!(&bytecode.operation(address)?.unwrap(), Operation::Load(_)));
        assert!(bytecode.memory_element_type(address)?.is_secret_integer());

        // B + C
        let address = BytecodeAddress::new(3, AddressType::Heap);
        assert!(matches!(&bytecode.operation(address)?.unwrap(), Operation::Addition(_)));
        assert!(bytecode.memory_element_type(address)?.is_secret_integer());

        // A * (B + C)
        let address = BytecodeAddress::new(4, AddressType::Heap);
        assert!(matches!(&bytecode.operation(address)?.unwrap(), Operation::Multiplication(_)));
        assert!(bytecode.memory_element_type(address)?.is_secret_integer());

        let body = Bytecode2Protocol::transform(MPCProtocolFactory, &bytecode)?;
        // The input memory should contain 3 elements
        assert_eq!(3, body.input_memory_scheme.len());

        let protocols = body.protocols.values().collect_vec();
        assert_eq!(protocols.len(), 2);

        assert!(matches!(protocols[0], MPCProtocol::Addition(_))); // b + c
        assert!(matches!(protocols[1], MPCProtocol::MultiplicationShares(_)), "actual {}", protocols[1]); // a * (b + c)
        Ok(())
    }
}

#[cfg(any(test, feature = "vm"))]
pub(crate) mod vm {
    use crate::{
        protocols::{MultiplicationPublic, MultiplicationSharePublic, MultiplicationShares},
        vm::{plan::MPCProtocolPreprocessingElements, MPCInstructionRouter, MPCMessages},
    };
    use anyhow::{anyhow, Error};
    use execution_engine_vm::vm::{
        instructions::{into_instruction_messages, DefaultInstructionStateMachine, Instruction, InstructionResult},
        memory::MemoryValue,
        sm::ExecutionContext,
    };
    use math_lib::modular::SafePrime;
    use nada_value::NadaValue;
    use protocols::multiplication::multiplication_shares::{MultState, MultStateMessage, OperandShares};
    use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer};

    impl<T> Instruction<T> for MultiplicationPublic
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
            match (left.to_type(), right.to_type()) {
                // Both are public
                (Integer, Integer) => {
                    let result = left.try_into_value()? * &right.try_into_value()?;
                    Ok(InstructionResult::Value { value: NadaValue::new_integer(result) })
                }
                (UnsignedInteger, UnsignedInteger) => {
                    let result = left.try_into_value()? * &right.try_into_value()?;
                    Ok(InstructionResult::Value { value: NadaValue::new_unsigned_integer(result) })
                }
                (Boolean, Boolean) => {
                    let result = left.try_into_value()? * &right.try_into_value()?;
                    Ok(InstructionResult::Value { value: NadaValue::new_boolean(result) })
                }

                (left, right) => Err(anyhow!(
                    "unsupported operands for multiplication of public values protocol: {left:?} * {right:?}"
                )),
            }
        }
    }

    impl<T> Instruction<T> for MultiplicationSharePublic
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
            match (left.to_type(), right.to_type()) {
                // Integers.
                (ShamirShareInteger, Integer) | (Integer, ShamirShareInteger) => {
                    let result = left.try_into_value()? * &right.try_into_value()?;
                    Ok(InstructionResult::Value { value: NadaValue::new_shamir_share_integer(result) })
                }
                // Unsigned Integers.
                (UnsignedInteger, ShamirShareUnsignedInteger) | (ShamirShareUnsignedInteger, UnsignedInteger) => {
                    let result = left.try_into_value()? * &right.try_into_value()?;
                    Ok(InstructionResult::Value { value: NadaValue::new_shamir_share_unsigned_integer(result) })
                }
                // Booleans - Multiplication is the same as logical AND
                (Boolean, ShamirShareBoolean) | (ShamirShareBoolean, Boolean) => {
                    let result = left.try_into_value()? * &right.try_into_value()?;
                    Ok(InstructionResult::Value { value: NadaValue::new_shamir_share_boolean(result) })
                }

                (left, right) => Err(anyhow!(
                    "unsupported operands for multiplication of share-public protocol: {left:?} * {right:?}"
                )),
            }
        }
    }

    impl<T> Instruction<T> for MultiplicationShares
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
                (ty @ ShamirShareInteger, ShamirShareInteger)
                | (ty @ ShamirShareUnsignedInteger, ShamirShareUnsignedInteger)
                | (ty @ ShamirShareBoolean, ShamirShareBoolean) => {
                    let operands =
                        OperandShares { left: vec![left.try_into_value()?], right: vec![right.try_into_value()?] };

                    let (initial_state, messages) = MultState::new(vec![operands], context.secret_sharer())?;

                    Ok(InstructionResult::StateMachine {
                        state_machine: MPCInstructionRouter::Multiplication(DefaultInstructionStateMachine::new(
                            initial_state,
                            ty,
                        )),
                        messages: into_instruction_messages(messages),
                    })
                }
                (left, right) => {
                    Err(anyhow!("unsupported operands for multiplication of shares protocol: {left:?} * {right:?}"))
                }
            }
        }
    }

    impl From<MultStateMessage> for MPCMessages {
        fn from(message: MultStateMessage) -> Self {
            MPCMessages::Multiplication(message)
        }
    }

    impl TryFrom<MPCMessages> for MultStateMessage {
        type Error = Error;

        fn try_from(msg: MPCMessages) -> Result<Self, Self::Error> {
            let MPCMessages::Multiplication(msg) = msg else {
                return Err(anyhow!("unknown instruction message"));
            };
            Ok(msg)
        }
    }
}

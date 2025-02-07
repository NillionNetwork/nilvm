//! Implementation of the MPC protocols for the reveal operation

use crate::{protocols::MPCProtocol, requirements::RuntimeRequirementType, utils::into_mpc_protocol};
use jit_compiler::{
    bytecode2protocol::{errors::Bytecode2ProtocolError, Bytecode2Protocol, Bytecode2ProtocolContext, ProtocolFactory},
    models::{
        bytecode::Reveal as BytecodeReveal,
        protocols::{memory::ProtocolAddress, ExecutionLine},
    },
    unary_protocol,
};

unary_protocol!(Reveal, "REV", ExecutionLine::Online, RuntimeRequirementType);
into_mpc_protocol!(Reveal);
impl Reveal {
    /// Transforms a bytecode reveal into a protocol
    pub(crate) fn transform<F: ProtocolFactory<MPCProtocol>>(
        context: &mut Bytecode2ProtocolContext<MPCProtocol, F>,
        operation: &BytecodeReveal,
    ) -> Result<MPCProtocol, Bytecode2ProtocolError> {
        let operand_ty = context.bytecode.memory_element_type(operation.operand)?.as_shamir_share()?;
        let protocol = Self {
            address: ProtocolAddress::default(),
            operand: Bytecode2Protocol::adapted_protocol(context, operation.operand, &operand_ty)?,
            ty: operation.ty.as_public()?,
            source_ref_index: operation.source_ref_index,
        };
        Ok(protocol.into())
    }
}

#[cfg(any(test, feature = "vm"))]
pub(crate) mod vm {
    use crate::{
        protocols::Reveal,
        vm::{plan::MPCProtocolPreprocessingElements, MPCInstructionRouter, MPCMessages},
    };
    use anyhow::{anyhow, Error};
    use execution_engine_vm::vm::{
        instructions::{into_instruction_messages, DefaultInstructionStateMachine, Instruction, InstructionResult},
        memory::MemoryValue,
        sm::ExecutionContext,
    };
    use math_lib::modular::{EncodedModularNumber, SafePrime};
    use protocols::reveal::{RevealMode, RevealState, RevealStateMessage};
    use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer};

    impl<T> Instruction<T> for Reveal
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
            let operand = context.read(self.operand)?;

            use nada_value::NadaType::*;

            // Operand is always secret.
            let (operand_type, operand_value) = (operand.to_type(), operand.try_into_value()?);
            match &operand_type {
                ShamirShareInteger | ShamirShareUnsignedInteger | ShamirShareBoolean => {
                    let return_type = operand_type.to_public()?;
                    let operands = RevealMode::new_all(vec![operand_value]);

                    let (initial_state, messages) = RevealState::new(operands, context.secret_sharer())?;

                    Ok(InstructionResult::StateMachine {
                        state_machine: MPCInstructionRouter::Reveal(DefaultInstructionStateMachine::new(
                            initial_state,
                            return_type,
                        )),
                        messages: into_instruction_messages(messages),
                    })
                }
                operand_type => {
                    Err(anyhow!("unsupported operand for reveal shares protocol: {operand_type:?}.to_public()"))
                }
            }
        }
    }

    impl From<RevealStateMessage<EncodedModularNumber>> for MPCMessages {
        fn from(message: RevealStateMessage<EncodedModularNumber>) -> Self {
            MPCMessages::Reveal(message)
        }
    }

    impl TryFrom<MPCMessages> for RevealStateMessage<EncodedModularNumber> {
        type Error = Error;

        fn try_from(msg: MPCMessages) -> Result<Self, Self::Error> {
            let MPCMessages::Reveal(msg) = msg else {
                return Err(anyhow!("unknown instruction message"));
            };
            Ok(msg)
        }
    }
}

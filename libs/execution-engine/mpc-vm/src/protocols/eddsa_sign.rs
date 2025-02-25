//! Implementation of the MPC protocols for the eddsa sign operation

use crate::{protocols::MPCProtocol, requirements::RuntimeRequirementType, utils::into_mpc_protocol};
use jit_compiler::{
    binary_protocol,
    bytecode2protocol::{errors::Bytecode2ProtocolError, Bytecode2Protocol, Bytecode2ProtocolContext, ProtocolFactory},
    models::{
        bytecode::EddsaSign as BytecodeEddsaSign,
        protocols::{memory::ProtocolAddress, ExecutionLine},
    },
};
use nada_value::NadaType;

// EddsaSign protocol
binary_protocol!(EddsaSign, "EddsaSign", ExecutionLine::Online, RuntimeRequirementType);
into_mpc_protocol!(EddsaSign);
impl EddsaSign {
    pub(crate) fn eddsa_protocol<F: ProtocolFactory<MPCProtocol>>(
        context: &mut Bytecode2ProtocolContext<MPCProtocol, F>,
        operation: &BytecodeEddsaSign,
    ) -> Result<MPCProtocol, Bytecode2ProtocolError> {
        let expected_type = NadaType::EddsaSignature;
        let left =
            Bytecode2Protocol::adapted_protocol(context, operation.left, &nada_value::NadaType::EddsaPrivateKey)?;
        let right = Bytecode2Protocol::adapted_protocol(context, operation.right, &nada_value::NadaType::EddsaMessage)?;
        let protocol = Self {
            address: ProtocolAddress::default(),
            left,
            right,
            ty: expected_type,
            source_ref_index: operation.source_ref_index,
        };
        Ok(protocol.into())
    }

    /// Transforms a bytecode EdDSA signature into a protocol
    pub(crate) fn transform<F: ProtocolFactory<MPCProtocol>>(
        context: &mut Bytecode2ProtocolContext<MPCProtocol, F>,
        operation: &BytecodeEddsaSign,
    ) -> Result<MPCProtocol, Bytecode2ProtocolError> {
        // Check types
        let right_type = context.bytecode.memory_element_type(operation.right)?;
        if !right_type.is_public() {
            return Err(Bytecode2ProtocolError::OperationNotSupported(format!(
                "The message digest should be public. {} not supported",
                right_type
            )));
        }
        let key_type = context.bytecode.memory_element_type(operation.left)?;
        if key_type.is_public() {
            return Err(Bytecode2ProtocolError::OperationNotSupported(format!(
                "Eddsa private key detected as public. {} not supported",
                right_type
            )));
        }
        Self::eddsa_protocol(context, operation)
    }
}

#[cfg(any(test, feature = "vm"))]
pub(crate) mod vm {
    use crate::{
        protocols::EddsaSign,
        vm::{plan::MPCProtocolPreprocessingElements, MPCInstructionRouter, MPCMessages},
    };
    use anyhow::{anyhow, Error};
    use execution_engine_vm::vm::{
        instructions::{into_instruction_messages, DefaultInstructionStateMachine, Instruction, InstructionResult},
        sm::ExecutionContext,
    };
    use math_lib::modular::SafePrime;
    use nada_value::{NadaType, NadaValue};
    use protocols::threshold_eddsa::{EddsaSignState, EddsaSignStateMessage};
    use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, SecretSharerProperties, ShamirSecretSharer};

    use basic_types::PartyMessage;
    use protocols::threshold_eddsa::output::EddsaSignatureOutput;
    use state_machine::StateMachineOutput;

    impl EddsaSign {
        /// Tailored handle_message being used by MPCInstructionRouter::handle_message for the EdDSA signature operation
        pub(crate) fn handle_message<I, T>(
            sm: &mut DefaultInstructionStateMachine<MPCMessages, EddsaSignState>,
            message: PartyMessage<MPCMessages>,
        ) -> Result<InstructionResult<I::Router, T>, Error>
        where
            I: Instruction<T, Message = MPCMessages>,
            T: SafePrime,
            ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
        {
            let (party_id, message) = message.into_parts();
            // Check if the message is understood by the instruction.
            let msg = message.try_into()?;
            // Delegates the message to the instruction state machine
            match sm.sm.handle_message(PartyMessage::new(party_id, msg))? {
                StateMachineOutput::Final(value_output) => {
                    // Manage the resulting share from the instruction state machine
                    let value = match value_output {
                        EddsaSignatureOutput::Success { element } => element,
                        EddsaSignatureOutput::Abort { reason } => return Err(reason.into()),
                    };
                    let value = NadaValue::new_eddsa_signature(value);
                    Ok(InstructionResult::Value { value })
                }
                StateMachineOutput::Empty => Ok(InstructionResult::Empty),
                StateMachineOutput::Messages(messages) => {
                    Ok(InstructionResult::InstructionMessage { messages: into_instruction_messages(messages) })
                }
            }
        }
    }

    impl<T> Instruction<T> for EddsaSign
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
            // key.eddsa_sign(message)
            let right = context.read(self.right)?;
            let left = context.read(self.left)?;

            // Destructure the value inside EddsaPrivateKey
            let (key_shares, eddsa_message) = match (left, right) {
                (NadaValue::EddsaPrivateKey(key_shares), NadaValue::EddsaMessage(eddsa_message)) => {
                    (key_shares, eddsa_message)
                }
                (left_operand, right_operand) => {
                    return Err(anyhow!(
                        "Expected left operand to be an EdDSA private key and right operand to be an EdDSA message digest, but got {left_operand:?} and {right_operand:?}"
                    ));
                }
            };

            let secret_sharer = context.secret_sharer();
            let parties = secret_sharer.parties();

            let (initial_state, messages) = EddsaSignState::new(parties, eddsa_message, key_shares)?;

            Ok(InstructionResult::StateMachine {
                state_machine: MPCInstructionRouter::EddsaSign(DefaultInstructionStateMachine::new(
                    initial_state,
                    NadaType::EddsaSignature,
                )),
                messages: into_instruction_messages(messages),
            })
        }
    }

    impl From<EddsaSignStateMessage> for MPCMessages {
        fn from(message: EddsaSignStateMessage) -> Self {
            MPCMessages::EddsaSign(message)
        }
    }

    impl TryFrom<MPCMessages> for EddsaSignStateMessage {
        type Error = Error;

        fn try_from(msg: MPCMessages) -> Result<Self, Self::Error> {
            let MPCMessages::EddsaSign(msg) = msg else {
                return Err(anyhow!("unknown instruction message"));
            };
            Ok(msg)
        }
    }
}

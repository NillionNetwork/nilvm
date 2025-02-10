//! Implementation of the MPC protocols for the ecdsa sign operation

use crate::{protocols::MPCProtocol, requirements::RuntimeRequirementType, utils::into_mpc_protocol};
use jit_compiler::{
    binary_protocol,
    bytecode2protocol::{errors::Bytecode2ProtocolError, Bytecode2Protocol, Bytecode2ProtocolContext, ProtocolFactory},
    models::{
        bytecode::EcdsaSign as BytecodeEcdsaSign,
        protocols::{memory::ProtocolAddress, ExecutionLine},
    },
};
use nada_value::NadaType;

// EcdsaSign protocol
binary_protocol!(
    EcdsaSign,
    "EcdsaSign",
    ExecutionLine::Online,
    RuntimeRequirementType,
    &[(RuntimeRequirementType::EcdsaAuxInfo, 1)]
);
into_mpc_protocol!(EcdsaSign);
impl EcdsaSign {
    pub(crate) fn ecdsa_protocol<F: ProtocolFactory<MPCProtocol>>(
        context: &mut Bytecode2ProtocolContext<MPCProtocol, F>,
        operation: &BytecodeEcdsaSign,
    ) -> Result<MPCProtocol, Bytecode2ProtocolError> {
        let expected_type = NadaType::EcdsaSignature;
        let left =
            Bytecode2Protocol::adapted_protocol(context, operation.left, &nada_value::NadaType::EcdsaPrivateKey)?;
        let right =
            Bytecode2Protocol::adapted_protocol(context, operation.right, &nada_value::NadaType::EcdsaDigestMessage)?;
        let protocol = Self {
            address: ProtocolAddress::default(),
            left,
            right,
            ty: expected_type,
            source_ref_index: operation.source_ref_index,
        };
        Ok(protocol.into())
    }

    /// Transforms a bytecode ECDSA signature into a protocol
    pub(crate) fn transform<F: ProtocolFactory<MPCProtocol>>(
        context: &mut Bytecode2ProtocolContext<MPCProtocol, F>,
        operation: &BytecodeEcdsaSign,
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
                "Ecdsa private key detected as public. {} not supported",
                right_type
            )));
        }
        Self::ecdsa_protocol(context, operation)
    }
}

#[cfg(any(test, feature = "vm"))]
pub(crate) mod vm {
    use crate::{
        protocols::EcdsaSign,
        vm::{plan::MPCProtocolPreprocessingElements, MPCInstructionRouter, MPCMessages},
    };
    use anyhow::{anyhow, Error};
    use execution_engine_vm::vm::{
        instructions::{into_instruction_messages, DefaultInstructionStateMachine, Instruction, InstructionResult},
        sm::ExecutionContext,
    };
    use math_lib::modular::SafePrime;
    use nada_value::{NadaType, NadaValue};
    use protocols::threshold_ecdsa::signing::{EcdsaSignState, EcdsaSignStateMessage};
    use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, SecretSharerProperties, ShamirSecretSharer};

    use basic_types::PartyMessage;
    use protocols::threshold_ecdsa::signing::output::EcdsaSignatureShareOutput;
    use state_machine::StateMachineOutput;

    impl EcdsaSign {
        /// Tailored handle_message being used by MPCInstructionRouter::handle_message for the ECDSA signature operation
        pub(crate) fn handle_message<I, T>(
            sm: &mut DefaultInstructionStateMachine<MPCMessages, EcdsaSignState>,
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
                        EcdsaSignatureShareOutput::Success { element } => element,
                        EcdsaSignatureShareOutput::Abort { reason } => return Err(reason.into()),
                    };
                    let value = NadaValue::new_ecdsa_signature(value);
                    Ok(InstructionResult::Value { value })
                }
                StateMachineOutput::Empty => Ok(InstructionResult::Empty),
                StateMachineOutput::Messages(messages) => {
                    Ok(InstructionResult::InstructionMessage { messages: into_instruction_messages(messages) })
                }
            }
        }
    }

    impl<T> Instruction<T> for EcdsaSign
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
            share_elements: Self::PreprocessingElement,
        ) -> Result<InstructionResult<Self::Router, T>, Error>
        where
            F: Instruction<T>,
        {
            let right = context.read(self.right)?;
            let left = context.read(self.left)?;

            // Destructure the value inside EcdsaPrivateKey
            let (key_shares, digest_message) = match (left, right) {
                (NadaValue::EcdsaPrivateKey(key_shares), NadaValue::EcdsaDigestMessage(digest_message)) => {
                    (key_shares, digest_message)
                }
                (left_operand, right_operand) => {
                    return Err(anyhow!(
                        "Expected left operand to be an ECDSA private key and right operand to be an ECDSA message digest, but got {left_operand:?} and {right_operand:?}"
                    ));
                }
            };

            let aux_info = share_elements
                .ecdsa_aux_info
                .ok_or_else(|| anyhow!("ECDSA auxiliary info material not found for sign operation"))?;
            // execution id is unique per protocol execution: address is unique within one program and program_id is unique per call execution.
            let address_str = self.address.to_string();
            let compute_id_bytes = context.compute_id.as_bytes();
            let mut eid = address_str.as_bytes().to_vec();
            eid.extend_from_slice(compute_id_bytes);
            let secret_sharer = context.secret_sharer();
            let parties = secret_sharer.parties();
            let party = secret_sharer.local_party_id();

            let (initial_state, messages) =
                EcdsaSignState::new(eid, parties, party.clone(), key_shares, aux_info, digest_message)?;

            Ok(InstructionResult::StateMachine {
                state_machine: MPCInstructionRouter::EcdsaSign(DefaultInstructionStateMachine::new(
                    initial_state,
                    NadaType::EcdsaSignature,
                )),
                messages: into_instruction_messages(messages),
            })
        }
    }

    impl From<EcdsaSignStateMessage> for MPCMessages {
        fn from(message: EcdsaSignStateMessage) -> Self {
            MPCMessages::EcdsaSign(message)
        }
    }

    impl TryFrom<MPCMessages> for EcdsaSignStateMessage {
        type Error = Error;

        fn try_from(msg: MPCMessages) -> Result<Self, Self::Error> {
            let MPCMessages::EcdsaSign(msg) = msg else {
                return Err(anyhow!("unknown instruction message"));
            };
            Ok(msg)
        }
    }
}

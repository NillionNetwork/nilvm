//! Implementation of the MPC protocols for the public key derive operation

use crate::{protocols::MPCProtocol, requirements::RuntimeRequirementType, utils::into_mpc_protocol};
use jit_compiler::{
    bytecode2protocol::{errors::Bytecode2ProtocolError, Bytecode2Protocol, Bytecode2ProtocolContext, ProtocolFactory},
    models::{
        bytecode::PublicKeyDerive as BytecodePublicKeyDerive,
        protocols::{memory::ProtocolAddress, ExecutionLine},
    },
    unary_protocol,
};

unary_protocol!(PublicKeyDerive, "PKD", ExecutionLine::Online, RuntimeRequirementType);
into_mpc_protocol!(PublicKeyDerive);
impl PublicKeyDerive {
    /// Transforms a bytecode public key derive into a protocol
    pub(crate) fn transform<F: ProtocolFactory<MPCProtocol>>(
        context: &mut Bytecode2ProtocolContext<MPCProtocol, F>,
        operation: &BytecodePublicKeyDerive,
    ) -> Result<MPCProtocol, Bytecode2ProtocolError> {
        let operand_ty = context.bytecode.memory_element_type(operation.operand)?;
        if operand_ty.is_public() {
            return Err(Bytecode2ProtocolError::OperationNotSupported(format!(
                "Corresponding private key detected as public. {} not supported",
                operand_ty
            )));
        }

        let protocol = Self {
            address: ProtocolAddress::default(),
            operand: Bytecode2Protocol::adapted_protocol(context, operation.operand, operand_ty)?,
            ty: operation.ty.as_public()?,
            source_ref_index: operation.source_ref_index,
        };
        Ok(protocol.into())
    }
}

#[cfg(any(test, feature = "vm"))]
pub(crate) mod vm {
    use crate::{
        protocols::PublicKeyDerive,
        vm::{plan::MPCProtocolPreprocessingElements, MPCInstructionRouter, MPCMessages},
    };
    use anyhow::{anyhow, Error};
    use execution_engine_vm::vm::{
        instructions::{Instruction, InstructionResult},
        sm::ExecutionContext,
    };
    use math_lib::modular::SafePrime;
    use nada_value::NadaValue;
    use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer};
    use threshold_keypair::publickey::EcdsaPublicKeyArray;

    impl<T> Instruction<T> for PublicKeyDerive
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

            // Operand is always secret.
            match operand {
                NadaValue::EcdsaPrivateKey(ecdsa_private_key) => {
                    let public_key: [u8; 33] = ecdsa_private_key
                        .into_inner()
                        .into_inner()
                        .key_info
                        .shared_public_key
                        .into_inner()
                        .to_bytes(true)
                        .as_bytes()
                        .try_into()
                        .map_err(|_| {
                            anyhow::anyhow!("Ecdsa public key extracted from private key has incorrect length")
                        })?;
                    Ok(InstructionResult::Value {
                        value: NadaValue::new_ecdsa_public_key(EcdsaPublicKeyArray(public_key)),
                    })
                }
                NadaValue::EddsaPrivateKey(eddsa_private_key) => {
                    let public_key: [u8; 32] = eddsa_private_key
                        .into_inner()
                        .into_inner()
                        .key_info
                        .shared_public_key
                        .into_inner()
                        .to_bytes(true)
                        .as_bytes()
                        .try_into()
                        .map_err(|_| {
                            anyhow::anyhow!("Eddsa public key extracted from private key has incorrect length")
                        })?;
                    Ok(InstructionResult::Value { value: NadaValue::new_eddsa_public_key(public_key) })
                }
                operand_type => {
                    Err(anyhow!("unsupported operand for public key derive protocol: {operand_type:?}.public_key()"))
                }
            }
        }
    }
}

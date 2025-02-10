//! Implementation of the MPC protocols for the random operation

use crate::{protocols::MPCProtocol, requirements::RuntimeRequirementType, utils::into_mpc_protocol};
use jit_compiler::{
    bytecode2protocol::{errors::Bytecode2ProtocolError, Bytecode2ProtocolContext, ProtocolFactory},
    models::{
        bytecode::Random as BytecodeRandom,
        protocols::{memory::ProtocolAddress, ExecutionLine, ProtocolDependencies},
        SourceRefIndex,
    },
    protocol,
};
use nada_value::NadaType;
use std::fmt::{Display, Formatter};

pub(crate) struct Random;

impl Random {
    /// Transforms a bytecode random into a protocol
    #[allow(clippy::collapsible_else_if)]
    pub(crate) fn transform<F: ProtocolFactory<MPCProtocol>>(
        context: &mut Bytecode2ProtocolContext<MPCProtocol, F>,
        operation: &BytecodeRandom,
    ) -> Result<MPCProtocol, Bytecode2ProtocolError> {
        match operation.ty {
            NadaType::SecretInteger | NadaType::SecretUnsignedInteger => RandomInteger::transform(context, operation),
            NadaType::SecretBoolean => RandomBoolean::transform(context, operation),
            _ => Err(Bytecode2ProtocolError::OperationNotSupported(format!(
                "type {} not supported for random()",
                operation.ty
            ))),
        }
    }
}

/// Random protocol
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RandomInteger {
    /// Address of the protocol
    pub address: ProtocolAddress,
    /// The protocol output type
    pub ty: NadaType,
    /// Source code info about this element.
    pub source_ref_index: SourceRefIndex,
}

protocol!(RandomInteger, RuntimeRequirementType, &[(RuntimeRequirementType::RandomInteger, 1)], ExecutionLine::Online);
into_mpc_protocol!(RandomInteger);

impl RandomInteger {
    /// Transforms a bytecode random into a protocol
    pub(crate) fn transform<F: ProtocolFactory<MPCProtocol>>(
        _: &mut Bytecode2ProtocolContext<MPCProtocol, F>,
        operation: &BytecodeRandom,
    ) -> Result<MPCProtocol, Bytecode2ProtocolError> {
        let expected_type = operation.ty.as_shamir_share()?;
        let protocol = Self {
            address: ProtocolAddress::default(),
            ty: expected_type,
            source_ref_index: operation.source_ref_index,
        };
        Ok(protocol.into())
    }
}

/// Random Boolean protocol
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RandomBoolean {
    /// Address of the protocol
    pub address: ProtocolAddress,
    /// The protocol output type
    pub ty: NadaType,
    /// Source code info about this element.
    pub source_ref_index: SourceRefIndex,
}

impl ProtocolDependencies for RandomBoolean {
    fn dependencies(&self) -> Vec<ProtocolAddress> {
        Vec::new()
    }
}

protocol!(RandomBoolean, RuntimeRequirementType, &[(RuntimeRequirementType::RandomBoolean, 1)], ExecutionLine::Online);
into_mpc_protocol!(RandomBoolean);

impl RandomBoolean {
    /// Transforms a bytecode random into a Random Boolean protocol
    pub(crate) fn transform<F: ProtocolFactory<MPCProtocol>>(
        _: &mut Bytecode2ProtocolContext<MPCProtocol, F>,
        operation: &BytecodeRandom,
    ) -> Result<MPCProtocol, Bytecode2ProtocolError> {
        let expected_type = operation.ty.as_shamir_share()?;
        let protocol = Self {
            address: ProtocolAddress::default(),
            ty: expected_type,
            source_ref_index: operation.source_ref_index,
        };
        Ok(protocol.into())
    }
}

impl Display for RandomBoolean {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} - rty({}) = RANDOM-BOOLEAN", self.address, self.ty)
    }
}

impl ProtocolDependencies for RandomInteger {
    fn dependencies(&self) -> Vec<ProtocolAddress> {
        Vec::new()
    }
}

impl Display for RandomInteger {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} - rty({}) = RANDOM-INTEGER", self.address, self.ty)
    }
}

#[cfg(any(test, feature = "vm"))]
pub(crate) mod vm {
    use crate::{
        protocols::{random::RandomBoolean, RandomInteger},
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

    impl<T> Instruction<T> for RandomInteger
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        type PreprocessingElement = MPCProtocolPreprocessingElements<T>;
        type Router = MPCInstructionRouter<T>;
        type Message = MPCMessages;

        fn run<F>(
            self,
            _: &mut ExecutionContext<F, T>,
            mut share_elements: Self::PreprocessingElement,
        ) -> Result<InstructionResult<Self::Router, T>, Error>
        where
            F: Instruction<T>,
        {
            use nada_value::NadaType::*;

            // Pull the random integer elements from preprocessing.
            let rand_elements =
                share_elements.random_integer.pop().ok_or_else(|| anyhow!("shares not found for random integer"))?;

            match self.ty {
                ShamirShareInteger => {
                    Ok(InstructionResult::Value { value: NadaValue::new_shamir_share_integer(rand_elements) })
                }
                ShamirShareUnsignedInteger => {
                    Ok(InstructionResult::Value { value: NadaValue::new_shamir_share_unsigned_integer(rand_elements) })
                }
                _ => Err(anyhow!("unexpected value for `self.ty`: {:?}", self.ty)),
            }
        }
    }

    impl<T> Instruction<T> for RandomBoolean
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        type PreprocessingElement = MPCProtocolPreprocessingElements<T>;
        type Router = MPCInstructionRouter<T>;
        type Message = MPCMessages;

        fn run<F>(
            self,
            _: &mut ExecutionContext<F, T>,
            mut share_elements: Self::PreprocessingElement,
        ) -> Result<InstructionResult<Self::Router, T>, Error>
        where
            F: Instruction<T>,
        {
            use nada_value::NadaType::*;

            // Pull the random boolean elements from preprocessing.
            let rand_elements =
                share_elements.random_boolean.pop().ok_or_else(|| anyhow!("shares not found for random boolean"))?;

            match self.ty {
                ShamirShareBoolean => {
                    Ok(InstructionResult::Value { value: NadaValue::new_shamir_share_boolean(rand_elements) })
                }
                _ => Err(anyhow!("unexpected value for `self.ty`: {:?}", self.ty)),
            }
        }
    }
}

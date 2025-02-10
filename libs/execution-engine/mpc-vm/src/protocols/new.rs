//! Implementation of the MPC protocols for the new operation

use crate::{protocols::MPCProtocol, requirements::RuntimeRequirementType, utils::into_mpc_protocol};
use jit_compiler::{
    bytecode2protocol::{errors::Bytecode2ProtocolError, Bytecode2Protocol, Bytecode2ProtocolContext, ProtocolFactory},
    models::{
        bytecode::{memory::BytecodeAddress, Operation},
        memory::address_count,
        protocols::{memory::ProtocolAddress, ExecutionLine, ProtocolDependencies},
        SourceRefIndex,
    },
    protocol,
};
use nada_value::NadaType;
use std::fmt::{Display, Formatter};

/// New array protocol implementation
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NewArray {
    /// Address of the protocol
    pub address: ProtocolAddress,
    /// List of protocol memory addresses of the array contain
    pub elements: Vec<ProtocolAddress>,
    /// Protocol type. It matches the array type.
    pub ty: NadaType,
    /// Source code info about this element.
    pub source_ref_index: SourceRefIndex,
}

protocol!(NewArray, RuntimeRequirementType, &[], ExecutionLine::Local);
into_mpc_protocol!(NewArray);

impl NewArray {
    pub(crate) fn transform<F: ProtocolFactory<MPCProtocol>>(
        context: &mut Bytecode2ProtocolContext<MPCProtocol, F>,
        bytecode_address: BytecodeAddress,
        mut inner_type: Box<NadaType>,
        size: usize,
    ) -> Result<MPCProtocol, Bytecode2ProtocolError> {
        if inner_type.is_secret() {
            inner_type = Box::new(inner_type.as_shamir_share()?)
        }

        let mut next_address = bytecode_address.next()?;
        // There are two representations possible for an array in Bytecode
        // For plain arrays or whenever an operation is performed - New,Get,Get or New,Load,Load
        // For multi-dimensional arrays, if no ops are performed, the structure is different, for 2D: New,New,Load,Load,New,Load,Load,New...
        let next_operation = context
            .bytecode
            .operation(next_address)?
            .ok_or(Bytecode2ProtocolError::logic("operation not found: finding array elements"))?;
        let advance_step = if let Operation::New(_) = next_operation { address_count(&inner_type)? } else { 1usize };

        let mut inner_protocols = vec![];
        for _ in 0..size {
            let inner_protocol = Bytecode2Protocol::adapted_protocol(context, next_address, &inner_type)?;
            inner_protocols.push(inner_protocol);
            next_address = next_address.advance(advance_step)?;
        }

        let protocol = Self {
            address: ProtocolAddress::default(),
            elements: inner_protocols,
            ty: NadaType::Array { inner_type, size },
            source_ref_index: SourceRefIndex::default(),
        };
        Ok(protocol.into())
    }
}
impl ProtocolDependencies for NewArray {
    fn dependencies(&self) -> Vec<ProtocolAddress> {
        self.elements.clone()
    }
}

impl Display for NewArray {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let elements: Vec<_> = self.elements.iter().map(|e| e.to_string()).collect();
        write!(f, "{} - rty({}) = NEWA {:?}", self.address, self.ty, elements)
    }
}

/// New tuple protocol implementation
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NewTuple {
    /// Address of the protocol
    pub address: ProtocolAddress,
    /// Left branch protocol memory address
    pub left: ProtocolAddress,
    /// Right branch protocol memory address
    pub right: ProtocolAddress,
    /// Protocol type. It matches the tuple type.
    pub ty: NadaType,
    /// Source code info about this element.
    pub source_ref_index: SourceRefIndex,
}

protocol!(NewTuple, RuntimeRequirementType, &[], ExecutionLine::Local);
into_mpc_protocol!(NewTuple);

impl NewTuple {
    pub(crate) fn transform<F: ProtocolFactory<MPCProtocol>>(
        context: &mut Bytecode2ProtocolContext<MPCProtocol, F>,
        bytecode_address: BytecodeAddress,
        mut left_type: Box<NadaType>,
        mut right_type: Box<NadaType>,
    ) -> Result<MPCProtocol, Bytecode2ProtocolError> {
        if left_type.is_secret() {
            left_type = Box::new(left_type.as_shamir_share()?)
        }
        let left_address = bytecode_address.next()?;
        let left = Bytecode2Protocol::adapted_protocol(context, left_address, &left_type)?;

        if right_type.is_secret() {
            right_type = Box::new(right_type.as_shamir_share()?)
        }
        let right_address = left_address.next()?;
        let right = Bytecode2Protocol::adapted_protocol(context, right_address, &right_type)?;

        let protocol = Self {
            address: ProtocolAddress::default(),
            left,
            right,
            ty: NadaType::Tuple { left_type, right_type },
            source_ref_index: SourceRefIndex::default(),
        };
        Ok(protocol.into())
    }
}

impl ProtocolDependencies for NewTuple {
    fn dependencies(&self) -> Vec<ProtocolAddress> {
        vec![self.left, self.right]
    }
}

impl Display for NewTuple {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} - rty({}) = NEWT ({}, {})", self.address, self.ty, self.left, self.right)
    }
}

#[cfg(any(test, feature = "vm"))]
pub(crate) mod vm {
    use crate::{
        protocols::{NewArray, NewTuple},
        vm::{plan::MPCProtocolPreprocessingElements, MPCInstructionRouter, MPCMessages},
    };
    use anyhow::Error;
    use execution_engine_vm::vm::{
        instructions::{Instruction, InstructionResult},
        sm::ExecutionContext,
    };
    use math_lib::modular::SafePrime;
    use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer};

    impl<T> Instruction<T> for NewArray
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
            {
                // Firstly, we reserve an address in the memory to indicate that from this point
                // a compound type is allocated.
                context.memory.store_header(self.address, self.ty);
                // We also create a pointer for each inner element in the memory. These pointers
                // point to the inner elements.
                let mut pointer_address = self.address.next()?;
                for ref_address in self.elements.into_iter() {
                    context.memory.create_ptr(pointer_address, ref_address)?;
                    pointer_address = pointer_address.next()?;
                }
                Ok(InstructionResult::Empty)
            }
        }
    }

    impl<T> Instruction<T> for NewTuple
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
            // Firstly, we reserve an address in the memory to indicate that from this point
            // a compound type is allocated.
            context.memory.store_header(self.address, self.ty);
            // Pointer to the left element
            context.memory.create_ptr(self.address.next()?, self.left)?;
            // Pointer to the right element
            context.memory.create_ptr(self.address.advance(2)?, self.right)?;
            Ok(InstructionResult::Empty)
        }
    }
}

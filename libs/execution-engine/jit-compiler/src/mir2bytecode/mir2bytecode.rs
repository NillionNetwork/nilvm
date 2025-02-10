//! This crate implements the transformation between the MIR and bytecode levels of Nada-lang

use crate::{
    mir2bytecode::errors::MIR2BytecodeError,
    models::{
        bytecode::{
            memory::BytecodeAddress, Addition, AddressedElement, Division, EcdsaSign, Equals, IfElse, InnerProduct,
            Input, LeftShift, LessThan, Literal, LiteralRef, Load, Modulo, Multiplication, New, Not, Operation, Output,
            Power, ProgramBytecode, PublicOutputEquality, Random, Reveal, RightShift, Subtraction, TruncPr,
        },
        memory::AddressType,
        SourceRefIndex,
    },
};
use itertools::Itertools;
use nada_compiler_backend::{
    literal_value::{LiteralValue, LiteralValueExt},
    mir::{
        ArrayAccessor as MIRArrayAccessor, Input as MIRInput, InputReference as MIRInputReference,
        Literal as MIRLiteral, Operation as MIROperation, OperationId, Output as MIROutput, ProgramMIR,
        TupleAccessor as MIRTupleAccessor, TupleIndex,
    },
};
use nada_type::NadaType;
use std::collections::{HashMap, HashSet};

/// Context required by the MIR to Bytecode transformer.
#[derive(Default)]
pub(crate) struct MIR2BytecodeContext {
    /// Bytecode that is being created by the transformation
    pub(crate) bytecode: ProgramBytecode,
    /// Input conversion table from its id into bytecode memory address
    input_addresses: HashMap<String, BytecodeAddress>,
    /// Operation conversion table from its id into bytecode memory address
    operation_addresses: HashMap<OperationId, BytecodeAddress>,
}

impl MIR2BytecodeContext {
    pub(crate) fn with_bytecode(mut self, bytecode: ProgramBytecode) -> Self {
        self.bytecode = bytecode;
        self
    }

    /// Get party ID from its name
    pub(crate) fn party_id(&self, party: &String) -> Result<usize, MIR2BytecodeError> {
        self.bytecode
            .parties
            .iter()
            .enumerate()
            .find(|(_, p)| &p.name == party)
            .map(|(id, _)| id)
            .ok_or_else(|| MIR2BytecodeError::PartyNotFound(party.to_string()))
    }

    /// Returns the next available address.
    fn next_address(&self) -> BytecodeAddress {
        let address = self.bytecode.operations_count();
        BytecodeAddress(address, AddressType::Heap)
    }

    /// Adds an input to the bytecode. In addition, it update the input conversion table.
    pub(crate) fn add_input(&mut self, input: Input) -> Result<(), MIR2BytecodeError> {
        let input_name = input.name.clone();
        let address = self.bytecode.add_input(input)?;
        self.input_addresses.insert(input_name, address);
        Ok(())
    }

    /// Retrives the memory address for an input. It receives the input name.
    pub(crate) fn input_address(&self, name: &String) -> Option<BytecodeAddress> {
        self.input_addresses.get(name).copied()
    }

    /// Retrieves the memory address for a literal. It receives the literal name.
    pub(crate) fn literal_address(&self, name: &String) -> Result<BytecodeAddress, MIR2BytecodeError> {
        self.bytecode
            .literals()
            .enumerate()
            .find(|(_, p)| &p.name == name)
            .map(|(id, _)| BytecodeAddress(id, AddressType::Literals))
            .ok_or_else(|| MIR2BytecodeError::LiteralNotFound(name.to_string()))
    }

    /// Adds an operation to the bytecode. In addition, it updates the operation conversion table.
    pub(crate) fn add_operation(
        &mut self,
        mir_operation_id: OperationId,
        operation: Operation,
    ) -> Result<(), MIR2BytecodeError> {
        let address = self.next_address();
        let operation = operation.with_address(address);
        self.bytecode.add_operation(operation);
        // A mir operation can be translated into more than one bytecode operations. For instance,
        // the input reference to an Array. The link is created only once, between the id and the
        // address of the first operation was created.
        self.operation_addresses.entry(mir_operation_id).or_insert(address);
        Ok(())
    }

    /// Create a memory indirection. The source is the mir operation id and the target is the
    /// resolved bytecode memory address
    pub(crate) fn resolve_memory_indirection(&mut self, mir_operation_id: OperationId, address: BytecodeAddress) {
        self.operation_addresses.entry(mir_operation_id).or_insert(address);
    }

    /// Retrieve the memroy address for an operation. It receives the MIROperation ID.
    pub(crate) fn operation_address(
        &self,
        mir_operation_id: OperationId,
    ) -> Result<BytecodeAddress, MIR2BytecodeError> {
        let mut source_address = *self
            .operation_addresses
            .get(&mir_operation_id)
            .ok_or(MIR2BytecodeError::OperationNotFound(mir_operation_id))?;
        // If we have a memory indirection, we use the incoming address as source. In this way, we
        // resolve the memory indirection.
        // This happens because the 'new' operations create as many 'get' operations as they need
        // to represent their content
        while let Operation::Get(o) =
            self.bytecode.operation(source_address)?.ok_or(MIR2BytecodeError::OperationNotFound(mir_operation_id))?
        {
            source_address = o.source_address
        }
        Ok(source_address)
    }
}

/// The result of an operation transformation can be
///  - A collection of operations that represents the translation of the MIR operation into bytecode.
///  - A memory indirection. In this case, the operation doesn't have a representation bytecode.
///    Usually, this is the scenario for the accessors.
pub(crate) enum TransformOperationResult {
    /// Resultant operations from a MIR Operation transformation
    Operations(Vec<Operation>),
    /// Resultant memory indirection from a MIR Operation transformation
    MemoryIndirection(OperationId, BytecodeAddress),
}

/// MIR to bytecode transformation
pub struct MIR2Bytecode;

impl MIR2Bytecode {
    /// Transforms a MIR model into a Bytecode
    pub fn transform(mir: &ProgramMIR) -> Result<ProgramBytecode, MIR2BytecodeError> {
        let bytecode = ProgramBytecode::default()
            .with_source_files((&mir.source_files).into())
            .with_source_refs(mir.source_refs.iter().map(|s| s.into()).collect_vec());
        let mut context = MIR2BytecodeContext::default().with_bytecode(bytecode);

        for party in &mir.parties {
            context.bytecode.parties.push(party.into());
        }

        for mir_input in mir.inputs.iter() {
            let input = Self::transform_input(&context, mir_input)?;
            context.add_input(input)?;
        }

        for literal in mir.literals.iter() {
            let literal = Self::transform_literal(literal)?;
            context.bytecode.add_literal(literal);
        }

        // Create a plan for the MIR operation. We will traverse the operations in the order that
        // the plan defines.
        let plan = Self::create_plan(mir)?;
        for mir_operation in plan.into_iter() {
            if !context.operation_addresses.contains_key(&mir_operation.id()) {
                match Self::transform_operation(&context, mir_operation)? {
                    TransformOperationResult::Operations(operations) => {
                        for operation in operations {
                            context.add_operation(mir_operation.id(), operation)?;
                        }
                    }
                    TransformOperationResult::MemoryIndirection(mir_operation_id, address) => {
                        context.resolve_memory_indirection(mir_operation_id, address);
                    }
                }
            }
        }

        for mir_output in mir.outputs.iter() {
            let output = Self::transform_output(&context, mir_output)?;
            context.bytecode.add_output(output)?;
        }
        Ok(context.bytecode)
    }

    /// Creates a bytecode input from a MIR Input.
    fn transform_input(context: &MIR2BytecodeContext, mir_input: &MIRInput) -> Result<Input, MIR2BytecodeError> {
        let party = context.party_id(&mir_input.party)?;
        Ok(Input::new(party, mir_input.name.clone(), mir_input.ty.clone(), (&mir_input.source_ref_index).into()))
    }

    /// Creates a bytecode literal from a MIR Literal.
    fn transform_literal(literal: &MIRLiteral) -> Result<Literal, MIR2BytecodeError> {
        let MIRLiteral { ty, name, value } = literal;
        // For now all literal values are converted to strings in the MIR. That should work for integers and decimals,
        // but for other types (arrays for instance) we might want to find a better alternative if we want to
        // support literals for them.
        Ok(Literal { name: name.clone(), value: LiteralValue::from_str(value, ty)?, ty: ty.clone() })
    }

    /// Creates a bytecode output from a MIR Output.
    fn transform_output(context: &MIR2BytecodeContext, mir_output: &MIROutput) -> Result<Output, MIR2BytecodeError> {
        let party_id = context.party_id(&mir_output.party)?;
        let address = context
            .operation_address(mir_output.operation_id)
            .map_err(|e| MIR2BytecodeError::BytecodeElementNotCreated(String::from("Output"), e.to_string()))?;
        Ok(Output::new(
            party_id,
            mir_output.name.clone(),
            address,
            mir_output.ty.clone(),
            (&mir_output.source_ref_index).into(),
        ))
    }

    /// Creates a plan of how the instructions have to be traversed.
    ///
    /// Currently, the first operations must match the load instruction of the inputs. That is
    /// because this part of the memory matches the input memory.
    ///
    /// After the inputs, we add the rest of the operation. The depending operations are added first,
    /// in this way, we could execute the program while traversing the operations sequentially if we
    /// need it.
    pub(crate) fn create_plan(mir: &ProgramMIR) -> Result<Vec<&MIROperation>, MIR2BytecodeError> {
        let mut plan = Vec::new();
        let mut planned_operations = HashSet::new();

        // Firstly, we have to traverse the InputReference. They will be translated into 'Operation::Load'.
        // The inputs will be loaded in the order that is defined by mir.inputs defines. For that,
        // we have to identify the input references for planning. In addition, they will be marked
        // as planned.
        let mut input_references = HashMap::new();
        for (id, operation) in mir.operations.iter() {
            match operation {
                o @ MIROperation::InputReference(input_ref) => {
                    if input_references.contains_key(&input_ref.refers_to) {
                        // We only accept a reference for each input
                        return Err(MIR2BytecodeError::RedundantLoad(input_ref.refers_to.clone()));
                    }
                    input_references.insert(input_ref.refers_to.to_string(), o);
                    planned_operations.insert(*id);
                }
                _ => {
                    // Otherwise, do nothing
                }
            }
        }
        // When we have the input references, we can traverse the inputs and add the references
        // to them into the plan.
        for input in mir.inputs.iter() {
            let input_ref = input_references
                .remove(&input.name)
                .ok_or_else(|| MIR2BytecodeError::InputNotReferenced(input.name.clone()))?;
            plan.push(input_ref)
        }
        if !input_references.is_empty() {
            return Err(MIR2BytecodeError::InputsNotExist(input_references.into_keys().collect()));
        }

        // Get all instructions from the outputs and reverse all of them. This way,
        // the first output will be traverses first.
        let mut pending_operations = mir
            .outputs
            .iter()
            .filter(|output| !planned_operations.contains(&output.operation_id))
            .map(|output| {
                mir.operation(output.operation_id)
                    .map_err(|_| MIR2BytecodeError::OperationNotFound(output.operation_id))
            })
            .rev()
            .collect::<Result<Vec<&MIROperation>, MIR2BytecodeError>>()?;

        while let Some(operation) = pending_operations.pop() {
            let incoming_operation_ids = operation.incoming_operations();
            let incoming_operations = incoming_operation_ids
                .into_iter()
                .filter(|incoming_operation_id| !planned_operations.contains(incoming_operation_id))
                .map(|incoming_operation_id| {
                    mir.operation(incoming_operation_id)
                        .map_err(|_| MIR2BytecodeError::OperationNotFound(incoming_operation_id))
                })
                .rev()
                .collect::<Result<Vec<&MIROperation>, MIR2BytecodeError>>()?;

            if !incoming_operations.is_empty() {
                pending_operations.push(operation);
                pending_operations.extend(incoming_operations);
            } else {
                planned_operations.insert(operation.id());
                plan.push(operation);
            }
        }
        Ok(plan)
    }

    /// Transforms a MIR Operation into a list of BytecodeOperation.
    fn transform_operation(
        context: &MIR2BytecodeContext,
        mir_operation: &MIROperation,
    ) -> Result<TransformOperationResult, MIR2BytecodeError> {
        match mir_operation {
            MIROperation::Not(o) => Not::from_mir(context, o),
            MIROperation::Reveal(o) => Reveal::from_mir(context, o),
            MIROperation::InputReference(o) => Self::transform_input_reference(context, o),
            MIROperation::LiteralReference(o) => LiteralRef::from_mir(context, o),
            MIROperation::Addition(o) => Addition::from_mir(context, o),
            MIROperation::Subtraction(o) => Subtraction::from_mir(context, o),
            MIROperation::Multiplication(o) => Multiplication::from_mir(context, o),
            MIROperation::Modulo(o) => Modulo::from_mir(context, o),
            MIROperation::Power(o) => Power::from_mir(context, o),
            MIROperation::LeftShift(o) => LeftShift::from_mir(context, o),
            MIROperation::RightShift(o) => RightShift::from_mir(context, o),
            MIROperation::Division(o) => Division::from_mir(context, o),
            MIROperation::LessThan(o) => LessThan::from_mir(context, o),
            MIROperation::PublicOutputEquality(o) => PublicOutputEquality::from_mir(context, o),
            MIROperation::Equals(o) => Equals::from_mir(context, o),
            MIROperation::NotEquals(_) => Err(MIR2BytecodeError::OperationNotSupported("not equals")), /* Handled in MIR pre-processing. */
            MIROperation::LessOrEqualThan(_) => Err(MIR2BytecodeError::OperationNotSupported("less or equal than")), /* Handled in MIR pre-processing. */
            MIROperation::GreaterThan(_) => Err(MIR2BytecodeError::OperationNotSupported("greater than")), /* Handled in MIR pre-processing. */
            MIROperation::GreaterOrEqualThan(_) => {
                Err(MIR2BytecodeError::OperationNotSupported("greater or equal than"))
            } /* Handled in MIR pre-processing. */
            MIROperation::Cast(_) => Err(MIR2BytecodeError::OperationNotSupported("cast")), /* Handled in MIR pre-processing. */
            MIROperation::Unzip(_) => Err(MIR2BytecodeError::OperationNotSupported("unzip")), /* Handled in MIR pre-processing. */
            MIROperation::Zip(_) => Err(MIR2BytecodeError::OperationNotSupported("zip")), /* Handled in MIR pre-processing. */
            MIROperation::New(o) => New::from_mir(context, o),
            MIROperation::ArrayAccessor(o) => Self::from_array_accessor(context, o),
            MIROperation::TupleAccessor(o) => Self::from_tuple_accessor(context, o),
            MIROperation::Map(_) => Err(MIR2BytecodeError::OperationNotSupported("map")), /* Handled in MIR pre-processing. */
            MIROperation::Reduce(_) => Err(MIR2BytecodeError::OperationNotSupported("reduce")), /* Handled in MIR pre-processing */
            MIROperation::NadaFunctionArgRef(_) => {
                Err(MIR2BytecodeError::OperationNotSupported("nada function arg ref"))
            }
            MIROperation::NadaFunctionCall(_) => Err(MIR2BytecodeError::OperationNotSupported("nada function call")),
            MIROperation::Random(op) => Random::from_mir(context, op),
            MIROperation::IfElse(o) => IfElse::from_mir(context, o),
            MIROperation::TruncPr(o) => TruncPr::from_mir(context, o),
            MIROperation::EcdsaSign(o) => EcdsaSign::from_mir(context, o),
            MIROperation::InnerProduct(o) => InnerProduct::from_mir(context, o),
            MIROperation::BooleanAnd(_) => Err(MIR2BytecodeError::OperationNotSupported("bitwise and")), // MIR pre-processed
            MIROperation::BooleanOr(_) => Err(MIR2BytecodeError::OperationNotSupported("bitwise or")), // MIR pre-processed
            MIROperation::BooleanXor(_) => Err(MIR2BytecodeError::OperationNotSupported("bitwise xor")), // MIR pre-processed
        }
    }

    /// Transforms a input reference into a list of operations.
    fn transform_input_reference(
        context: &MIR2BytecodeContext,
        mir_input_ref: &MIRInputReference,
    ) -> Result<TransformOperationResult, MIR2BytecodeError> {
        let input_address = context
            .input_address(&mir_input_ref.refers_to)
            .ok_or_else(|| MIR2BytecodeError::InputNotFound(mir_input_ref.refers_to.clone()))?;
        let operations = Self::create_input_load_operations(
            input_address,
            &mir_input_ref.ty,
            (&mir_input_ref.source_ref_index).into(),
        )?;
        Ok(TransformOperationResult::Operations(operations))
    }

    /// Creates the operations for loading an input from the memory:
    /// - Primitive inputs: it creates a 'Load' operation.
    /// - Compound inputs: it creates a 'New' operation and traverses its contents calling this
    ///   function recursively.
    ///
    /// These operations are usually the first operations in the list of bytecode operations.
    fn create_input_load_operations(
        mut input_address: BytecodeAddress,
        ty: &NadaType,
        source_ref_index: SourceRefIndex,
    ) -> Result<Vec<Operation>, MIR2BytecodeError> {
        let mut inner_types = vec![ty];
        let mut operations = vec![];
        while let Some(ty) = inner_types.pop() {
            match ty {
                NadaType::Array { inner_type, size } => {
                    // If the input is an array, we create a 'New' and traverse its content.
                    let new_op = New { address: BytecodeAddress::default(), ty: ty.clone(), source_ref_index };
                    operations.push(new_op.into());
                    inner_types.extend(vec![inner_type.as_ref(); *size]);
                }
                NadaType::Tuple { left_type, right_type } => {
                    // If the input is a tuple, we create a 'New' and traverse its content.
                    let new_op = New { address: BytecodeAddress::default(), ty: ty.clone(), source_ref_index };
                    operations.push(new_op.into());
                    inner_types.push(right_type);
                    inner_types.push(left_type);
                }
                // In this point we should have only primitive types.
                _ if !ty.is_primitive() => return Err(MIR2BytecodeError::UnsupportedInputType("new compound type")),
                _ => {
                    // Load is used to move from input memory to the heap.
                    let load_op =
                        Load { input_address, address: BytecodeAddress::default(), ty: ty.clone(), source_ref_index };
                    operations.push(load_op.into());
                }
            }
            input_address = input_address.next()?;
        }
        Ok(operations)
    }

    /// Transform a MIR [`ArrayAccessor`] into a memory indirection.
    fn from_array_accessor(
        context: &MIR2BytecodeContext,
        mir_array_accessor: &MIRArrayAccessor,
    ) -> Result<TransformOperationResult, MIR2BytecodeError> {
        // This address points to the `Operation::New` or the source array
        let source_address = context.operation_address(mir_array_accessor.source)?;
        // The address of the element is index of the element plus one (the 'New' operation address)
        let offset = mir_array_accessor.index.checked_add(1).ok_or(MIR2BytecodeError::ArrayAccessorOffset)?;
        let array_element_address = source_address.advance(offset)?;
        Ok(TransformOperationResult::MemoryIndirection(mir_array_accessor.id, array_element_address))
    }

    /// Transform a MIR [`TupleAccessor`] into a memory indirection.
    fn from_tuple_accessor(
        context: &MIR2BytecodeContext,
        mir_tuple_accessor: &MIRTupleAccessor,
    ) -> Result<TransformOperationResult, MIR2BytecodeError> {
        // This address points to the `Operation::New` or the source array
        let source_address = context.operation_address(mir_tuple_accessor.source)?;
        // The address of the element is index of the element plus one (the 'New' operation address)
        let offset = match mir_tuple_accessor.index {
            TupleIndex::Left => 1,
            TupleIndex::Right => 2,
        };
        let tuple_element_address = source_address.advance(offset)?;
        Ok(TransformOperationResult::MemoryIndirection(mir_tuple_accessor.id, tuple_element_address))
    }
}

//! This crate provides utilities for generating MIR programs programmatically
use crate::{
    Addition, Input, InputReference, Map, Multiplication, NadaFunction, NadaFunctionArg, NadaFunctionArgRef,
    NadaFunctionCall, Operation, OperationId, OperationMap, Output, Party, ProgramMIR, Reduce, SourceRef,
    SourceRefIndex,
};
use nada_type::NadaType;

/// Add functionality for generating ProgramMIR
impl ProgramMIR {
    /// Add an operation to the operation container
    pub fn add_operation<O: Into<Operation>>(&mut self, operation: O) -> OperationId {
        let operation: Operation = operation.into();
        let operation_id = operation.id();
        self.operations.insert(operation_id, operation);
        operation_id
    }

    /// Creates an empty ProgramMIR
    pub fn build() -> Self {
        let mut program = ProgramMIR::default();
        program.source_refs.push(SourceRef::default());
        program
    }

    /// Add a party
    pub fn add_party<S: Into<String>>(&mut self, name: S) {
        let name = name.into();
        if !self.parties.iter().any(|p| *p.name == name) {
            self.parties.push(Party { name, source_ref_index: SourceRefIndex::default() });
        }
    }

    /// Add an input to the program inputs
    pub fn add_input<S: Into<String>>(&mut self, name: S, ty: NadaType, party: S) {
        let party = party.into();
        let input = Input {
            name: name.into(),
            ty,
            source_ref_index: SourceRefIndex::default(),
            party: party.clone(),
            doc: Default::default(),
        };
        self.add_party(party);
        self.inputs.push(input);
    }

    /// Add an output to the program outputs
    pub fn add_output<S: Into<String>>(&mut self, name: S, id: OperationId, ty: NadaType, party: S) {
        let party = party.into();
        let output = Output {
            name: name.into(),
            ty,
            operation_id: id,
            source_ref_index: SourceRefIndex::default(),
            party: party.clone(),
        };
        self.add_party(party);
        self.outputs.push(output);
    }

    /// Add a function to the program functions
    pub fn add_function(&mut self, function: NadaFunction) -> OperationId {
        let function_id = function.id;
        self.functions.push(function);
        function_id
    }
}

/// Add functionality for generating NadaFunction
impl NadaFunction {
    /// Add an operation to the NadaFunction
    pub fn add_operation<O: Into<Operation>>(&mut self, operation: O) -> OperationId {
        let operation: Operation = operation.into();
        let operation_id = operation.id();
        self.operations.insert(operation_id, operation);
        self.return_operation_id = operation_id;
        operation_id
    }

    /// Creates a new NadaFunction
    pub fn build<S: Into<String>>(name: S, ty: NadaType, id: OperationId) -> Self {
        Self {
            name: name.into(),
            id,
            source_ref_index: SourceRefIndex::default(),
            args: vec![],
            operations: OperationMap::default(),
            return_type: ty,
            return_operation_id: OperationId::INVALID,
        }
    }

    /// Add a formal argument to the NadaFunction
    pub fn add_arg<S: Into<String>>(&mut self, name: S, ty: NadaType, id: OperationId) -> OperationId {
        let arg = NadaFunctionArg { name: name.into(), ty: ty.clone(), source_ref_index: SourceRefIndex::default() };
        let arg_ref = Operation::NadaFunctionArgRef(NadaFunctionArgRef {
            function_id: self.id,
            id,
            ty,
            refers_to: arg.name.clone(),
            source_ref_index: SourceRefIndex::default(),
        });
        self.args.push(arg);
        self.add_operation(arg_ref)
    }
}

/// Add functionality for generating InputReference
impl InputReference {
    /// Creates a new InputReference
    pub fn build<S: Into<String>>(refers_to: S, ty: NadaType, id: OperationId) -> Operation {
        Operation::InputReference(InputReference {
            refers_to: refers_to.into(),
            ty,
            id,
            source_ref_index: SourceRefIndex::default(),
        })
    }
}

/// Add functionality for generating NadaFunctionCall
impl NadaFunctionCall {
    /// Creates a new NadaFunction call.
    pub fn build(function_id: OperationId, args: Vec<OperationId>, ty: NadaType, id: OperationId) -> Operation {
        Operation::NadaFunctionCall(NadaFunctionCall {
            id,
            function_id,
            args,
            return_type: ty,
            source_ref_index: SourceRefIndex::default(),
        })
    }
}

/// Add functionality for generating Map operation
impl Map {
    /// Creates a new Map operation
    pub fn build(function_id: OperationId, inner: OperationId, ty: NadaType, id: OperationId) -> Operation {
        Operation::Map(Map { id, function_id, inner, ty, source_ref_index: SourceRefIndex::default() })
    }
}

/// Add functionality for generating a Reduce operation
impl Reduce {
    /// Creates a new Reduce operation
    pub fn build(
        function_id: OperationId,
        initial: OperationId,
        inner: OperationId,
        ty: NadaType,
        id: OperationId,
    ) -> Operation {
        Operation::Reduce(Reduce { id, function_id, initial, inner, ty, source_ref_index: SourceRefIndex::default() })
    }
}

/// Add functionality for generating NadaFunctionArgRef
impl NadaFunctionArgRef {
    /// Creates a new NadaFunctionArgRef
    pub fn build<S: Into<String>>(function_id: OperationId, refers_to: S, ty: NadaType, id: OperationId) -> Operation {
        Operation::NadaFunctionArgRef(NadaFunctionArgRef {
            function_id,
            id,
            refers_to: refers_to.into(),
            ty,
            source_ref_index: SourceRefIndex::default(),
        })
    }
}

/// Add functionality for generating a binary operation
impl Addition {
    /// Creates a new addition operation.
    pub fn build(left: OperationId, right: OperationId, ty: NadaType, id: OperationId) -> Operation {
        Operation::Addition(Addition { id, left, right, ty, source_ref_index: SourceRefIndex::default() })
    }
}

/// Add functionality for generating a binary operation
impl Multiplication {
    /// Creates a new multiplication operation.
    pub fn build(left: OperationId, right: OperationId, ty: NadaType, id: OperationId) -> Operation {
        Operation::Multiplication(Multiplication { id, left, right, ty, source_ref_index: SourceRefIndex::default() })
    }
}

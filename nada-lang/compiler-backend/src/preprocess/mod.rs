//! MIR Preprocessor module

pub mod error;
pub(crate) mod function_preprocessor;
pub(crate) mod operation_preprocessors;
pub mod preprocessor;

use self::error::MIRPreprocessorError;
use mir_model::{
    ArrayAccessor, NadaFunction, Operation, OperationId, OperationIdGenerator, ProgramMIR, SourceInfo, TypedElement,
};
use nada_value::NadaType;
pub use preprocessor::preprocess;
use std::collections::HashMap;

type FunctionMap = HashMap<OperationId, NadaFunction>;
type InputRefMap = HashMap<String, OperationId>;
type LiteralRefMap = HashMap<String, OperationId>;

/// Generate a funcion map where the keys are the function ids.
fn function_index_map(functions: Vec<NadaFunction>) -> FunctionMap {
    let mut function_index = HashMap::new();
    for function in functions {
        function_index.insert(function.id, function);
    }
    function_index
}

pub(crate) trait MIROperationPreprocessor {
    fn preprocess(
        self,
        context: &mut PreprocessorContext,
    ) -> Result<MIROperationPreprocessorResult, MIRPreprocessorError>;
}

pub(crate) struct MIROperationPreprocessorResult {
    pub(crate) operations: Vec<Operation>,
}

/// Preprocessor context.
#[derive(Debug, Clone)]
pub(crate) struct PreprocessorContext {
    /// MIR that is being preprocessed
    pub(crate) mir: ProgramMIR,
    /// Operation ID generator
    pub(crate) operation_id_generator: OperationIdGenerator,
    /// Index of the Literal references that the program contains.
    pub(crate) literal_ref_index: LiteralRefMap,
    /// Index of the Input references that the program contains.
    pub(crate) input_ref_index: InputRefMap,
    /// Index of the functions that the program defines.
    functions: FunctionMap,
}

impl PreprocessorContext {
    /// Creates a [`PreprocessorContext`] from a [`ProgramMIR`]
    pub(crate) fn new(mir: ProgramMIR) -> Self {
        let mut input_ref_index = HashMap::new();
        let mut literal_ref_index = HashMap::new();
        for (id, operation) in mir.operations.iter() {
            match operation {
                Operation::InputReference(input_ref) => {
                    input_ref_index.insert(input_ref.refers_to.clone(), *id);
                }
                Operation::LiteralReference(literal_ref) => {
                    literal_ref_index.insert(literal_ref.refers_to.clone(), *id);
                }
                _ => {}
            }
        }
        Self {
            functions: function_index_map(mir.functions.clone()),
            operation_id_generator: mir.operation_id_generator(),
            input_ref_index,
            literal_ref_index,
            mir,
        }
    }

    pub(crate) fn function(&self, function_id: OperationId) -> Result<&NadaFunction, MIRPreprocessorError> {
        self.functions.get(&function_id).ok_or(MIRPreprocessorError::MissingFunction(function_id))
    }
}

/// Create the array accessors to the output of an operation. If the output type of the operation
/// is not an array, the function returns an error.
fn create_array_accessors(
    context: &mut PreprocessorContext,
    operation_id: OperationId,
) -> Result<Vec<Operation>, MIRPreprocessorError> {
    let mut accessors = vec![];
    let operation = context.mir.operation(operation_id).map_err(|_| MIRPreprocessorError::OperationNotFound)?;
    if let NadaType::Array { size, inner_type } = operation.ty() {
        let inner_type = inner_type.as_ref();
        for index in 0..*size {
            let accessor = Operation::ArrayAccessor(ArrayAccessor {
                id: context.operation_id_generator.next_id(),
                index,
                source: operation_id,
                ty: inner_type.clone(),
                source_ref_index: operation.source_ref_index(),
            });
            accessors.push(accessor);
        }
        Ok(accessors)
    } else {
        Err(MIRPreprocessorError::UnexpectedType(format!("expected Array, found {}", operation.ty())))
    }
}

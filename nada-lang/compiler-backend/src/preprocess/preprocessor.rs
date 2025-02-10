//! The MIR Pre-processor.
//!
//! Expands operations to simplify bytecode generation.

use super::{
    error::MIRPreprocessorError, MIROperationPreprocessor, MIROperationPreprocessorResult, PreprocessorContext,
};
use crate::preprocess::operation_preprocessors::IsPreprocessable;
use mir_model::{Operation, OperationId, ProgramMIR};

/// MIR Operation visitor
pub(crate) trait MIROperationVisitor {
    /// Visit operation
    fn visit(
        &mut self,
        context: &mut PreprocessorContext,
        operation_id: OperationId,
    ) -> Result<Vec<OperationId>, MIRPreprocessorError>;
}

/// Visitor that handles preprocessing of MIR Operations.
pub(crate) struct PreprocessingVisitor;

impl MIROperationVisitor for PreprocessingVisitor {
    #[allow(clippy::todo)]
    fn visit(
        &mut self,
        context: &mut PreprocessorContext,
        operation_id: OperationId,
    ) -> Result<Vec<OperationId>, MIRPreprocessorError> {
        let operation = context.mir.operations.remove(&operation_id).ok_or(MIRPreprocessorError::OperationNotFound)?;
        if operation.is_preprocessable() {
            let MIROperationPreprocessorResult { operations } = operation.preprocess(context)?;
            let operations_ids: Vec<_> = operations.iter().map(|o| o.id()).rev().collect();
            for operation in operations {
                match &operation {
                    Operation::InputReference(input_ref) => {
                        context.input_ref_index.insert(input_ref.refers_to.clone(), input_ref.id);
                    }
                    Operation::LiteralReference(literal_ref) => {
                        context.input_ref_index.insert(literal_ref.refers_to.clone(), literal_ref.id);
                    }
                    _ => {}
                }
                context.mir.operations.insert(operation.id(), operation);
            }
            Ok(operations_ids)
        } else {
            context.mir.operations.insert(operation.id(), operation);
            Ok(vec![])
        }
    }
}

/// Pre-process MIR
///
/// This is the entry point of the MIR pre-processor.
pub fn preprocess(mir: ProgramMIR) -> Result<ProgramMIR, MIRPreprocessorError> {
    mir.check_function_recursion()?;
    let mut context = PreprocessorContext::new(mir);
    let mut visitor = PreprocessingVisitor;
    // PLAN: Given all operations are in the instructions map, we loop through and visit each operation
    // Returning either the same operation or a list of operations that replace the original one
    // This builds the new instructions map that replaces the original one.
    let mut instructions: Vec<_> = context.mir.operations.keys().copied().collect();
    while let Some(id) = instructions.pop() {
        instructions.extend(visitor.visit(&mut context, id)?);
    }
    Ok(context.mir)
}

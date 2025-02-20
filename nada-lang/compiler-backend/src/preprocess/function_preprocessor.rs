//! The NADA Function preprocessor
//!
//! Pre-processing of functions involves expanding the operations, while replacing the function arguments,
//! using the call arguments in place of the function arguments.

use std::collections::HashMap;

use mir_model::{
    delegate_to_inner, Addition, ArrayAccessor, BooleanAnd, BooleanOr, BooleanXor, Cast, Division, EcdsaSign,
    EddsaSign, Equals, GreaterOrEqualThan, GreaterThan, IfElse, InnerProduct, InputReference, LeftShift,
    LessOrEqualThan, LessThan, LiteralReference, Map, Modulo, Multiplication, NadaFunction, NadaFunctionArgRef,
    NadaFunctionCall, New, Not, NotEquals, Operation, OperationId, Power, PublicKeyDerive, PublicOutputEquality,
    Random, Reduce, Reveal, RightShift, Subtraction, TruncPr, TupleAccessor, Unzip, Zip,
};

use super::{
    error::MIRPreprocessorError, MIROperationPreprocessor, MIROperationPreprocessorResult, PreprocessorContext,
};

/// Creates a map of function operation identifiers to call arguments for a function.
///
/// The logic in this function relies on the assumption that the provided call arguments are given
/// in the same order as the function arguments.
///
/// # Arguments
/// * `call_args` - The call arguments from [`NadaFunctionCall`]
/// * `function` - The [`NadaFunction`]
fn function_input_index_map(
    context: &PreprocessorContext,
    function_id: OperationId,
    call_args: Vec<OperationId>,
) -> Result<HashMap<OperationId, OperationId>, MIRPreprocessorError> {
    let function = context.function(function_id)?;
    let mut input_index = HashMap::new();
    let arg_map: HashMap<_, _> = function.args.iter().zip(call_args).map(|(arg, id)| (arg.name.clone(), id)).collect();
    for (&id, operation) in function.operations.iter() {
        match operation {
            // Identify the call arguments that will be replaced.
            Operation::NadaFunctionArgRef(arg_ref) => {
                let formal_argument_id = arg_map
                    .get(&arg_ref.refers_to)
                    .ok_or(MIRPreprocessorError::InvalidFunctionArgument(arg_ref.refers_to.clone()))?;
                input_index.insert(id, *formal_argument_id);
            }
            // Identify the input references that will be replaced by the input references that are already
            // contained in the program
            Operation::InputReference(input_ref) => {
                if let Some(input_ref_id) = context.input_ref_index.get(&input_ref.refers_to).copied() {
                    input_index.insert(id, input_ref_id);
                }
            }
            // Identify the literal references that will be replaced by the input references that are already
            // contained in the program
            Operation::LiteralReference(literal_ref) => {
                if let Some(literal_ref_id) = context.literal_ref_index.get(&literal_ref.refers_to).copied() {
                    input_index.insert(id, literal_ref_id);
                }
            }
            _ => {
                // Do nothing
            }
        }
    }
    Ok(input_index)
}

/// The MIR preprocessor implementation for a [`NadaFunctionCall`]
impl MIROperationPreprocessor for NadaFunctionCall {
    fn preprocess(
        self,
        context: &mut PreprocessorContext,
    ) -> Result<MIROperationPreprocessorResult, MIRPreprocessorError> {
        let NadaFunctionCall { id, function_id, args, .. } = self;
        let (operations, return_operation_id) = {
            let NadaFunction { operations, return_operation_id, .. } = context.function(function_id)?;
            (operations.clone(), *return_operation_id)
        };
        let call_args = function_input_index_map(context, function_id, args)?;

        // we don't want to include function argument references in the new operation tree.
        let operations: HashMap<_, _> = operations.into_iter().filter(|(id, _)| !call_args.contains_key(id)).collect();

        let mut function_operations = Vec::new();
        let mut replacement_ids = HashMap::new();
        // Set the new MIR identifiers for all the function operations
        for (operation_id, mut operation) in operations {
            let id = if operation_id == return_operation_id { id } else { context.operation_id_generator.next_id() };
            replacement_ids.insert(operation.id(), id);
            operation.set_id(id);
            function_operations.push(operation);
        }

        // Change the identifiers in all operations referencing other function operations
        // Also replace references to function arguments by call arguments
        replacement_ids.extend(call_args);

        let mut resulting_operations = vec![];
        for mut operation in function_operations {
            // We replace all the pointers to function arguments by the corresponding call arguments
            for (&old_id, &new_id) in replacement_ids.iter() {
                operation.replace(old_id, new_id);
            }
            resulting_operations.push(operation);
        }

        Ok(MIROperationPreprocessorResult { operations: resulting_operations })
    }
}

/// Replace Incoming operations trait.
///
/// Extension trait for operations to facilitate replacing operation identifiers.
trait ReplaceIncomingOperations {
    /// Replaces the original operation identifier if the Operation contains it.
    /// Otherwise do nothing.
    fn replace(&mut self, original_id: OperationId, replacement_id: OperationId);
}

impl ReplaceIncomingOperations for Operation {
    fn replace(&mut self, original_id: OperationId, replacement_id: OperationId) {
        delegate_to_inner!(self, replace, original_id, replacement_id)
    }
}

/// Implementation of [`ReplaceOrDefault`] for binary operations
#[macro_export]
macro_rules! binary_replace_or_default {
    ($($name:ident),+) => {
        $(
        impl ReplaceIncomingOperations for $name {
            fn replace(&mut self, original_id: OperationId, replacement_id: OperationId) {
                if self.left == original_id {
                    self.left = replacement_id
                }
                if self.right == original_id {
                    self.right = replacement_id
                }
            }
        }
        )+
    };
}

/// NOP Implementation of [`ReplaceIncomingOperations`]
///
/// This is for homogeneity, and support operations that do not need to do anything
/// in `replace()`.
#[macro_export]
macro_rules! binary_replace_nop {
    ($($name:ident),+) => {
        $(
        impl ReplaceIncomingOperations for $name {
            fn replace(&mut self, _original_id: OperationId, _replacement_id: OperationId) {
                // Some operations do not need to do anything here.
            }
        }
        )+
    };
}

/// Implementation of [`ReplaceIncomingOperations`] for unary operations
#[macro_export]
macro_rules! unary_replace_or_default {
    ($(($name:ident, $field:tt)),+) => {
        $(
        impl ReplaceIncomingOperations for $name {
            fn replace(&mut self, original_id: OperationId, replacement_id: OperationId) {
                if self.$field == original_id {
                    self.$field = replacement_id
                }
            }
        }
        )+
    };
}

binary_replace_or_default!(
    Addition,
    Subtraction,
    Multiplication,
    Division,
    Modulo,
    Zip,
    LessThan,
    LessOrEqualThan,
    GreaterThan,
    GreaterOrEqualThan,
    Power,
    PublicOutputEquality,
    Equals,
    LeftShift,
    RightShift,
    TruncPr,
    InnerProduct,
    NotEquals,
    BooleanAnd,
    BooleanOr,
    BooleanXor,
    EcdsaSign,
    EddsaSign
);

binary_replace_nop!(InputReference, LiteralReference, Random, NadaFunctionArgRef);
unary_replace_or_default!(
    (Cast, target),
    (Not, this),
    (TupleAccessor, source),
    (Reveal, this),
    (PublicKeyDerive, this),
    (Map, inner),
    (Reduce, inner),
    (Unzip, this),
    (ArrayAccessor, source)
);

/// Implementation of [`ReplaceIncomingOperations`] for a [`NadaFunctionCall`].
///
/// Replaces any occurrence of the original operation identifier in the arguments by the
/// replacement identifier provided.
impl ReplaceIncomingOperations for NadaFunctionCall {
    fn replace(&mut self, original_id: OperationId, replacement_id: OperationId) {
        let mut call_args_copy = Vec::new();
        for arg in self.args.iter() {
            let arg = if *arg == original_id { replacement_id } else { *arg };
            call_args_copy.push(arg);
        }
        self.args = call_args_copy
    }
}

impl ReplaceIncomingOperations for New {
    fn replace(&mut self, original_id: OperationId, replacement_id: OperationId) {
        let mut elements_copy = Vec::new();
        for element in self.elements.iter() {
            let element = if *element == original_id { replacement_id } else { *element };
            elements_copy.push(element);
        }
        self.elements = elements_copy;
    }
}

impl ReplaceIncomingOperations for IfElse {
    fn replace(&mut self, original_id: OperationId, replacement_id: OperationId) {
        if self.this == original_id {
            self.this = replacement_id
        }
        if self.arg_0 == original_id {
            self.arg_0 = replacement_id
        }
        if self.arg_1 == original_id {
            self.arg_1 = replacement_id
        }
    }
}

#[cfg(test)]
mod test {
    use super::ReplaceIncomingOperations;
    use mir_model::{Addition, OperationId, SourceRefIndex};
    use nada_value::NadaType;

    #[test]
    fn replace_binary_op() {
        let mut addition = Addition {
            id: OperationId::with_id(0),
            left: OperationId::with_id(1),
            right: OperationId::with_id(2),
            ty: NadaType::SecretInteger,
            source_ref_index: SourceRefIndex::default(),
        };
        addition.replace(OperationId::with_id(1), OperationId::with_id(11));
        assert_eq!(addition.left, OperationId::with_id(11));
        addition.replace(OperationId::with_id(2), OperationId::with_id(22));
        assert_eq!(addition.right, OperationId::with_id(22));
    }
}

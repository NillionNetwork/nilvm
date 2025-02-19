//! Operation Pre-processors
//!
//! Implementations of MIR Pre-processors for different operations.
//! A pre-processor typically expands a complex MIR Operation into simpler operations.

use super::{create_array_accessors, MIROperationPreprocessor, MIROperationPreprocessorResult, PreprocessorContext};
use crate::preprocess::error::MIRPreprocessorError;
use mir_model::{
    Addition, BooleanAnd, BooleanOr, BooleanXor, Equals, GreaterOrEqualThan, GreaterThan, LessOrEqualThan, LessThan,
    Map, Multiplication, NadaFunctionCall, New, Not, NotEquals, Operation, Reduce, Subtraction, TupleAccessor,
    TupleIndex, TypedElement, Unzip, Zip,
};
use nada_value::NadaType;

/// Extension trait to check that operation is pre-processable
pub(crate) trait IsPreprocessable {
    /// Returns true if the operation needs to be handled by the pre-processor
    fn is_preprocessable(&self) -> bool;
}

impl IsPreprocessable for Operation {
    fn is_preprocessable(&self) -> bool {
        match self {
            Operation::Map(_)
            | Operation::Reduce(_)
            | Operation::Zip(_)
            | Operation::Unzip(_)
            | Operation::NadaFunctionCall(_)
            | Operation::LessOrEqualThan(_)
            | Operation::GreaterThan(_)
            | Operation::GreaterOrEqualThan(_)
            | Operation::NotEquals(_)
            | Operation::BooleanAnd(_)
            | Operation::BooleanOr(_)
            | Operation::BooleanXor(_) => true,
            Operation::Addition(_)
            | Operation::Subtraction(_)
            | Operation::Multiplication(_)
            | Operation::LessThan(_)
            | Operation::PublicOutputEquality(_)
            | Operation::Equals(_)
            | Operation::Cast(_)
            | Operation::InputReference(_)
            | Operation::LiteralReference(_)
            | Operation::NadaFunctionArgRef(_)
            | Operation::Modulo(_)
            | Operation::Power(_)
            | Operation::Division(_)
            | Operation::ArrayAccessor(_)
            | Operation::TupleAccessor(_)
            | Operation::New(_)
            | Operation::Random(_)
            | Operation::IfElse(_)
            | Operation::Reveal(_)
            | Operation::PublicKeyDerive(_)
            | Operation::Not(_)
            | Operation::LeftShift(_)
            | Operation::RightShift(_)
            | Operation::TruncPr(_)
            | Operation::InnerProduct(_)
            | Operation::EcdsaSign(_)
            | Operation::EddsaSign(_) => false,
        }
    }
}

impl MIROperationPreprocessor for Operation {
    fn preprocess(
        self,
        context: &mut PreprocessorContext,
    ) -> Result<MIROperationPreprocessorResult, MIRPreprocessorError> {
        match self {
            Operation::Map(o) => o.preprocess(context),
            Operation::Reduce(o) => o.preprocess(context),
            Operation::Zip(o) => o.preprocess(context),
            Operation::Unzip(o) => o.preprocess(context),
            Operation::NadaFunctionCall(o) => o.preprocess(context),
            Operation::LessOrEqualThan(o) => o.preprocess(context),
            Operation::GreaterThan(o) => o.preprocess(context),
            Operation::GreaterOrEqualThan(o) => o.preprocess(context),
            Operation::NotEquals(o) => o.preprocess(context),
            Operation::BooleanAnd(o) => o.preprocess(context),
            Operation::BooleanOr(o) => o.preprocess(context),
            Operation::BooleanXor(o) => o.preprocess(context),
            _ => Err(MIRPreprocessorError::NotPreprocessable),
        }
    }
}

impl MIROperationPreprocessor for BooleanAnd {
    fn preprocess(self, _: &mut PreprocessorContext) -> Result<MIROperationPreprocessorResult, MIRPreprocessorError> {
        let BooleanAnd { id, left, right, ty, source_ref_index, .. } = self;
        let product = Operation::Multiplication(Multiplication { id, left, right, ty, source_ref_index });
        Ok(MIROperationPreprocessorResult { operations: vec![product] })
    }
}

impl MIROperationPreprocessor for BooleanOr {
    fn preprocess(
        self,
        context: &mut PreprocessorContext,
    ) -> Result<MIROperationPreprocessorResult, MIRPreprocessorError> {
        let BooleanOr { id, left, right, ty, source_ref_index, .. } = self;
        // a | b = a + b - a*b
        let add_op_id = context.operation_id_generator.next_id();
        let addition = Operation::Addition(Addition { id: add_op_id, left, right, ty: ty.clone(), source_ref_index });
        let prod_op_id = context.operation_id_generator.next_id();
        let product =
            Operation::Multiplication(Multiplication { id: prod_op_id, left, right, ty: ty.clone(), source_ref_index });
        let subtraction = Operation::Subtraction(Subtraction {
            id,
            left: add_op_id,
            right: prod_op_id,
            ty: ty.clone(),
            source_ref_index,
        });
        Ok(MIROperationPreprocessorResult { operations: vec![addition, product, subtraction] })
    }
}

impl MIROperationPreprocessor for BooleanXor {
    fn preprocess(
        self,
        context: &mut PreprocessorContext,
    ) -> Result<MIROperationPreprocessorResult, MIRPreprocessorError> {
        let BooleanXor { id, left, right, ty, source_ref_index, .. } = self;
        // a | b = a + b - 2*a*b == (a + b) - ((a * b) + (a * b))
        // We add an extra multiplication because we cannot multiply integer and boolean
        // Solving this in the pre-processing phase has advantages in that we don't need to add new instructions to the VM
        // but in the future we might need performance optimisations
        let add_op_id = context.operation_id_generator.next_id();
        let addition = Operation::Addition(Addition { id: add_op_id, left, right, ty: ty.clone(), source_ref_index });
        let prod_op_id = context.operation_id_generator.next_id();
        let product =
            Operation::Multiplication(Multiplication { id: prod_op_id, left, right, ty: ty.clone(), source_ref_index });
        // Addition of the two products - They are the same operation so left and right point to the same id.
        let product_add_op_id = context.operation_id_generator.next_id();
        let product_addition = Operation::Addition(Addition {
            id: product_add_op_id,
            left: prod_op_id,
            right: prod_op_id,
            ty: ty.clone(),
            source_ref_index,
        });

        let subtraction = Operation::Subtraction(Subtraction {
            id,
            left: add_op_id,
            right: product_add_op_id,
            ty: ty.clone(),
            source_ref_index,
        });
        Ok(MIROperationPreprocessorResult { operations: vec![addition, product, product_addition, subtraction] })
    }
}

impl MIROperationPreprocessor for LessOrEqualThan {
    fn preprocess(
        self,
        context: &mut PreprocessorContext,
    ) -> Result<MIROperationPreprocessorResult, MIRPreprocessorError> {
        let LessOrEqualThan { id, left, right, ty, source_ref_index, .. } = self;
        let lt_op_id = context.operation_id_generator.next_id();
        // We transform this operation into a "Not Greater Than", by applying a Not to a LessThan operation where the left
        // and right arguments are reversed
        let less_than =
            Operation::LessThan(LessThan { id: lt_op_id, left: right, right: left, ty: ty.clone(), source_ref_index });
        let operation = Operation::Not(Not { id, this: lt_op_id, ty, source_ref_index });
        Ok(MIROperationPreprocessorResult { operations: vec![operation, less_than] })
    }
}

impl MIROperationPreprocessor for GreaterOrEqualThan {
    fn preprocess(
        self,
        context: &mut PreprocessorContext,
    ) -> Result<MIROperationPreprocessorResult, MIRPreprocessorError> {
        let GreaterOrEqualThan { id, left, right, ty, source_ref_index, .. } = self;
        // We transform this operation into a "Not Less Than", by applying a Not to a LessThan operation
        let lt_op_id = context.operation_id_generator.next_id();
        let less_than = Operation::LessThan(LessThan { id: lt_op_id, left, right, ty: ty.clone(), source_ref_index });
        let not = Operation::Not(Not { id, this: lt_op_id, ty, source_ref_index });
        Ok(MIROperationPreprocessorResult { operations: vec![not, less_than] })
    }
}

impl MIROperationPreprocessor for GreaterThan {
    fn preprocess(self, _: &mut PreprocessorContext) -> Result<MIROperationPreprocessorResult, MIRPreprocessorError> {
        let GreaterThan { id, left, right, ty, source_ref_index, .. } = self;
        // We transform this operation into a "LessThan" operation where the left and right
        // arguments are reversed
        let operation =
            Operation::LessThan(LessThan { id, left: right, right: left, ty: ty.clone(), source_ref_index });
        Ok(MIROperationPreprocessorResult { operations: vec![operation] })
    }
}

impl MIROperationPreprocessor for NotEquals {
    fn preprocess(
        self,
        context: &mut PreprocessorContext,
    ) -> Result<MIROperationPreprocessorResult, MIRPreprocessorError> {
        let NotEquals { id, left, right, ty, source_ref_index, .. } = self;
        let op_id = context.operation_id_generator.next_id();

        let equals = Operation::Equals(Equals { id: op_id, left, right, ty: ty.clone(), source_ref_index });
        let not = Operation::Not(Not { id, this: op_id, ty, source_ref_index });
        Ok(MIROperationPreprocessorResult { operations: vec![not, equals] })
    }
}

impl MIROperationPreprocessor for Unzip {
    /// Unzip pre-processor.
    /// An unzip operation consumes its input from another operation ('inner'). The type of this
    /// input must be a Array<Tuple<Type1, Type2>>.
    /// On the other hand, the output type must be a Tuple<Array<Type1>, Array<Type2>>.
    ///
    /// The result of the expansion is:
    /// - New (resultant tuple): Represents the resultant tuple.
    /// - New (left Array): Represents the array that is contained in the left branch of the
    ///                      resultant tuple. It contains an accessor for each Tuple that the
    ///                      input contains.
    /// - New (right Array): Represents the array that is contained in the right branch of the
    ///                      resultant tuple. It contains an accessor for each Tuple that the
    ///                      input contains.
    /// - TupleAccessor (left) x size: For each tuple that the input contains, we will have an
    ///                      accessor to its left branch. Its inputs is an ArrayAccessor that
    ///                      represents the reading of the input array.
    /// - TupleAccessor (right) x size: For each tuple that the input contains, we will have an
    ///                      accessor to its right branch. Its inputs is an ArrayAccessor that
    ///                      represents the reading of the input array.
    /// - ArrayAccessor x size: For each element that input contains, we will have an ArrayAccessor.
    ///                      It represents the reading of the input array.
    fn preprocess(
        self,
        context: &mut PreprocessorContext,
    ) -> Result<MIROperationPreprocessorResult, MIRPreprocessorError> {
        // The output of an unzip is a Tuple of arrays. We need the inner array types, we will use
        // them for building the inner accessors.

        // Initially, we prepare some types that will be used during the unzip expansion. We
        // get these types from the output type of the original unzip operation.

        // The output type must be Tuple { left: Array<?>, right: Array<?> }.
        let NadaType::Tuple { left_type, right_type } = &self.ty else {
            let msg = format!("expected type for unzip was Tuple<Array<?>,Array<?>>, found {:?}", self.ty);
            return Err(MIRPreprocessorError::UnexpectedType(msg));
        };

        // The branches of the tuple must be arrays.
        let (NadaType::Array { inner_type: left_inner_type, .. }, NadaType::Array { inner_type: right_inner_type, .. }) =
            (left_type.as_ref(), right_type.as_ref())
        else {
            return Err(MIRPreprocessorError::UnexpectedType(format!(
                "expected type for unzip was Tuple<Array<?>,Array<?>>, found Tuple<{left_type:?}, {right_type:?}>"
            )));
        };

        // Create the array accessors for the unzip's input. The type of these operations must
        // be tuples. Type checking must be performed early in the compiler, so here is not required.
        let inner_array_accessors = create_array_accessors(context, self.this)?;

        // Create the tuple accessors to the tuples that are contained by the input array.
        let mut left_unzip_elements = vec![];
        let mut right_unzip_elements = vec![];
        for operation in inner_array_accessors.iter() {
            let left = Operation::TupleAccessor(TupleAccessor {
                id: context.operation_id_generator.next_id(),
                index: TupleIndex::Left,
                source: operation.id(),
                ty: left_inner_type.as_ref().clone(),
                source_ref_index: self.source_ref_index,
            });
            left_unzip_elements.push(left);

            let right = Operation::TupleAccessor(TupleAccessor {
                id: context.operation_id_generator.next_id(),
                index: TupleIndex::Right,
                source: operation.id(),
                ty: right_inner_type.as_ref().clone(),
                source_ref_index: self.source_ref_index,
            });
            right_unzip_elements.push(right);
        }

        let left_array = Operation::New(New {
            id: context.operation_id_generator.next_id(),
            ty: left_type.as_ref().clone(),
            elements: left_unzip_elements.iter().map(|o| o.id()).collect(),
            source_ref_index: self.source_ref_index,
        });
        let right_array = Operation::New(New {
            id: context.operation_id_generator.next_id(),
            ty: right_type.as_ref().clone(),
            elements: right_unzip_elements.iter().map(|o| o.id()).collect(),
            source_ref_index: self.source_ref_index,
        });

        // Resultant tuple
        let operation = Operation::New(New {
            id: self.id,
            ty: self.ty,
            elements: vec![left_array.id(), right_array.id()],
            source_ref_index: self.source_ref_index,
        });

        // Resultant tuple + New (left Array) + New (right Array)
        let mut operations = vec![operation, left_array, right_array];
        // TupleAccessors (left)
        operations.extend(left_unzip_elements);
        // TupleAccessors (right)
        operations.extend(right_unzip_elements);
        // ArrayAccessors
        operations.extend(inner_array_accessors);

        Ok(MIROperationPreprocessorResult { operations })
    }
}

impl MIROperationPreprocessor for Zip {
    /// Zip pre-processor.
    /// A zip operation consumes its inputs from two operations ('left' and 'right'). The types of
    /// this inputs must be Array<Type1> and Array<Type2>.
    /// On the other hand, the output type must be Array<Tuple<Type1, Type2>>.
    ///
    /// The result of the expansion is:
    /// - New (resultant Array): Represents the resultant array.
    /// - New (tuple) x size: Represent the tuples that are contained by the resultant array.
    /// - ArrayAccessor (left) x size: For each element that the left input contains, we will have an
    ///                                ArrayAccessor. It represents the reading of the left input array.
    /// - ArrayAccessor (right) x size: For each element that the right input contains, we will have an
    ///                                 ArrayAccessor. It represents the reading of the right input array.
    fn preprocess(
        self,
        context: &mut PreprocessorContext,
    ) -> Result<MIROperationPreprocessorResult, MIRPreprocessorError> {
        let left_accessors = create_array_accessors(context, self.left)?;
        let right_accessors = create_array_accessors(context, self.right)?;

        // The Zip type is an array and we are interested in the inner type which is a Tuple for each element.
        let inner_tuple_constructors: Vec<_> = left_accessors
            .iter()
            .zip(right_accessors.iter())
            .map(|(left, right)| {
                Operation::New(New {
                    id: context.operation_id_generator.next_id(),
                    ty: NadaType::Tuple {
                        left_type: Box::new(left.ty().clone()),
                        right_type: Box::new(right.ty().clone()),
                    },
                    elements: vec![left.id(), right.id()],
                    source_ref_index: self.source_ref_index,
                })
            })
            .collect();

        // The resultant array
        let new_op = Operation::New(New {
            id: self.id,
            ty: self.ty().clone(),
            elements: inner_tuple_constructors.iter().map(|o| o.id()).collect(),
            source_ref_index: self.source_ref_index,
        });

        // Resultant array
        let mut operations = vec![new_op];
        // The tuples that the resultant array contains
        operations.extend(inner_tuple_constructors);
        // Access to the left input array
        operations.extend(left_accessors);
        // Access to the right input array
        operations.extend(right_accessors);

        Ok(MIROperationPreprocessorResult { operations })
    }
}

impl MIROperationPreprocessor for Map {
    fn preprocess(
        self,
        context: &mut PreprocessorContext,
    ) -> Result<MIROperationPreprocessorResult, MIRPreprocessorError> {
        let accessors = create_array_accessors(context, self.inner)?;
        let function_calls: Vec<_> = accessors
            .iter()
            .map(|accessor| {
                Operation::NadaFunctionCall(NadaFunctionCall {
                    id: context.operation_id_generator.next_id(),
                    function_id: self.function_id,
                    args: vec![accessor.id()],
                    source_ref_index: self.source_ref_index,
                    return_type: self.ty.clone(),
                })
            })
            .collect();

        // The resultant array
        let new_op = Operation::New(New {
            id: self.id,
            ty: self.ty().clone(),
            elements: function_calls.iter().map(|o| o.id()).collect(),
            source_ref_index: self.source_ref_index,
        });

        let mut operations = vec![new_op];
        operations.extend(accessors);
        operations.extend(function_calls);

        Ok(MIROperationPreprocessorResult { operations })
    }
}

impl MIROperationPreprocessor for Reduce {
    fn preprocess(
        self,
        context: &mut PreprocessorContext,
    ) -> Result<MIROperationPreprocessorResult, MIRPreprocessorError> {
        let accessors = create_array_accessors(context, self.inner)?;
        let mut operation_ids: Vec<_> =
            (0..accessors.len().saturating_sub(1)).map(|_| context.operation_id_generator.next_id()).collect();
        operation_ids.push(self.id);
        let mut accumulator = self.initial;
        let mut operations: Vec<_> = accessors
            .iter()
            .zip(operation_ids)
            .map(|(accessor, id)| {
                let function_call = Operation::NadaFunctionCall(NadaFunctionCall {
                    id,
                    function_id: self.function_id,
                    args: vec![accumulator, accessor.id()],
                    source_ref_index: self.source_ref_index,
                    return_type: self.ty.clone(),
                });
                accumulator = id;
                function_call
            })
            .collect();

        operations.extend(accessors);

        Ok(MIROperationPreprocessorResult { operations })
    }
}

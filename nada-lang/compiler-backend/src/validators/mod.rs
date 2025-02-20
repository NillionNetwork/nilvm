//! Program's MIR validation. It is executed during the compilation process and check that the
//! MIR is well-built. This validation is required to avoid storing malformed MIR

pub mod report;

use anyhow::{anyhow, Context, Result};
use duplicate::duplicate_item;
use nada_value::{NadaType, NadaTypeMetadata};
use std::collections::{HashMap, HashSet};

use crate::validators::report::ValidationContext;
use mir_model::{
    Addition, ArrayAccessor, BinaryOperation, Division, EcdsaSign, EddsaSign, Equals, GreaterThan, HasOperands, IfElse,
    InnerProduct, Input, LeftShift, LessThan, Modulo, Multiplication, NamedElement, New, Not, NotEquals, Operation,
    OperationId, Power, ProgramMIR, PublicKeyDerive, PublicOutputEquality, Reveal, RightShift, SourceInfo, Subtraction,
    TruncPr, TupleAccessor, TupleIndex, TypedElement,
};

#[derive(Debug, Clone, Copy, PartialEq)]
enum ValidationResult {
    Succeeded,
    Failed,
}

fn validate_same_underlying_type<L: TypedElement + SourceInfo, R: TypedElement + SourceInfo>(
    left: &L,
    right: &R,
    context: &mut ValidationContext,
    program: &ProgramMIR,
) -> Result<ValidationResult> {
    Ok(if !left.ty().has_same_underlying_type(right.ty()) {
        context.report_incompatible_type(left, right, program)?;
        ValidationResult::Failed
    } else {
        ValidationResult::Succeeded
    })
}

fn validate_operand_has_same_underlying_type<O: TypedElement + SourceInfo>(
    operation: &O,
    operand_id: OperationId,
    context: &mut ValidationContext,
    program: &ProgramMIR,
) -> Result<ValidationResult> {
    let operand = program.operation(operand_id)?;

    let result = validate_same_underlying_type(operation, operand, context, program)?;

    Ok(result)
}

fn validate_operands_have_same_underlying_type<O: HasOperands + TypedElement + SourceInfo>(
    operation: &O,
    context: &mut ValidationContext,
    program: &ProgramMIR,
) -> Result<ValidationResult> {
    let mut result = ValidationResult::Succeeded;

    for operand in operation.operands() {
        if validate_operand_has_same_underlying_type(operation, operand, context, program)? == ValidationResult::Failed
        {
            result = ValidationResult::Failed;
        }
    }

    Ok(result)
}

fn validate_operands_have_same_numeric_underlying_type<O: HasOperands + TypedElement + SourceInfo + NamedElement>(
    operation: &O,
    context: &mut ValidationContext,
    program: &ProgramMIR,
) -> Result<ValidationResult> {
    let mut result = ValidationResult::Succeeded;
    if validate_operands_have_same_underlying_type(operation, context, program)? == ValidationResult::Failed {
        result = ValidationResult::Failed;
    }

    for operand_id in operation.operands() {
        let operand = program.operation(operand_id)?;
        let operand_meta_type: NadaTypeMetadata = operand.ty().into();

        if !operand_meta_type.is_numeric() {
            context.report_invalid_type(operation, operand, program)?;
            result = ValidationResult::Failed;
        }
    }

    Ok(result)
}

/// This trait extends MIR model element with the API validate.
pub trait Validatable {
    /// Validate that a MIR model element is well-formed
    fn validate(&self, context: &mut ValidationContext, program: &ProgramMIR) -> Result<()>;
}

#[duplicate_item(operation; [Addition]; [Multiplication]; [Subtraction])]
impl Validatable for operation {
    fn validate(&self, context: &mut ValidationContext, program: &ProgramMIR) -> Result<()> {
        validate_operands_have_same_underlying_type(self, context, program)?;

        Ok(())
    }
}

#[duplicate_item(operation; [Division]; [Modulo])]
impl Validatable for operation {
    fn validate(&self, context: &mut ValidationContext, program: &ProgramMIR) -> Result<()> {
        validate_operands_have_same_numeric_underlying_type(self, context, program)?;

        Ok(())
    }
}

#[duplicate_item(operation_; [LeftShift]; [RightShift])]
impl Validatable for operation_ {
    fn validate(&self, context: &mut ValidationContext, program: &ProgramMIR) -> Result<()> {
        validate_operand_has_same_underlying_type(self, self.left, context, program)?;

        let right_operand = program.operation(self.right)?;
        if !right_operand.ty().is_unsigned_integer() || !right_operand.ty().is_public() {
            context.report_invalid_operand(self, right_operand, "right", program)?;
        }

        Ok(())
    }
}

impl Validatable for Power {
    fn validate(&self, context: &mut ValidationContext, program: &ProgramMIR) -> Result<()> {
        validate_operands_have_same_underlying_type(self, context, program)?;

        let left_operand = program.operation(self.left)?;
        let right_operand = program.operation(self.right)?;

        let mut power_operand_validation = |operand: &Operation, operand_name| -> Result<()> {
            // Currently, we support composability for public power
            // However, the Execution Engine doesn't have support for composability when the operation
            // is a private variant
            if !operand.ty().is_public()
                && !matches!(operand, Operation::InputReference(_) | Operation::LiteralReference(_))
            {
                context.report_invalid_operand(self, operand, operand_name, program)?;
            }

            Ok(())
        };

        power_operand_validation(left_operand, "left")?;
        power_operand_validation(right_operand, "right")?;

        Ok(())
    }
}

fn validate_new_array(op: &New, context: &mut ValidationContext, program: &ProgramMIR) -> Result<()> {
    // Validate that we have at least one input
    let Some(first_element) = op.elements.first() else {
        context.report_error(op, "malformed array new operation: expected at least one element", program)?;
        return Ok(());
    };

    // Check that all inputs have the same type
    let first_element_operand = program.operation(*first_element)?;
    for (i, input) in op.elements.iter().enumerate() {
        let input_operand = program.operation(*input)?;
        if input_operand.ty() != first_element_operand.ty() {
            context.report_error(
                op,
                &format!(
                    "malformed array new operation: expected all elements to have type {}, but element {} has type {}",
                    first_element_operand.ty(),
                    i,
                    input_operand.ty()
                ),
                program,
            )?;
        }
    }

    Ok(())
}

fn validate_new_tuple(op: &New, context: &mut ValidationContext, program: &ProgramMIR) -> Result<()> {
    // Validate that we have exactly two inputs
    let [left_element, right_element, ..] = op.elements.as_slice() else {
        context.report_error(
            op,
            &format!("malformed tuple new operation: expected two elements, got {}", op.elements.len()),
            program,
        )?;
        return Ok(());
    };
    let left_element_operand = program.operation(*left_element)?;
    let right_element_operand = program.operation(*right_element)?;

    // Validate that the tuple has the expected input types
    let Some((left, right)) = op.ty.as_tuple() else {
        context.report_error(
            op,
            &format!("malformed tuple new operation: expected tuple type, got {}", op.ty),
            program,
        )?;
        return Ok(());
    };

    let left = left.to_type_kind();
    let right = right.to_type_kind();
    let (left_element_operand, right_element_operand) =
        (&left_element_operand.ty().to_type_kind(), &right_element_operand.ty().to_type_kind());
    if left_element_operand != &left || right_element_operand != &right {
        context.report_error(op, &format!(
            "malformed tuple new operation: expected tuple input types ({left:?}, {right:?}), got ({left_element_operand:?}, {right_element_operand:?})"
        ), program)?;
        return Ok(());
    }

    Ok(())
}

impl Validatable for New {
    fn validate(&self, context: &mut ValidationContext, program: &ProgramMIR) -> Result<()> {
        match &self.ty {
            NadaType::Array { .. } => validate_new_array(self, context, program)?,
            NadaType::Tuple { .. } => validate_new_tuple(self, context, program)?,
            ty if ty.is_primitive() => {
                context.report_error(self, "malformed new operation: not a compound type", program)?;
            }
            ty => Err(anyhow!("compound type ({ty:?}) is not supported"))?,
        }

        Ok(())
    }
}

impl Validatable for IfElse {
    fn validate(&self, context: &mut ValidationContext, program: &ProgramMIR) -> Result<()> {
        // Check that the return type is always secret when at least one of the branches is secret.
        let arg_0_operand = program.operation(self.arg_0)?;
        let arg_1_operand = program.operation(self.arg_1)?;
        if self.ty().is_public() && (arg_0_operand.ty().is_secret() || arg_1_operand.ty().is_secret()) {
            context.report_error(
                self,
                "if-else: output type is not secret while at least one of its branches is",
                program,
            )?;
            return Ok(());
        }

        let this_operand = program.operation(self.this)?;
        if !this_operand.ty().is_boolean() && !this_operand.ty().is_secret_boolean() {
            context.report_error(self, "if-else: condition type has to be a boolean", program)?;
            return Ok(());
        }

        validate_same_underlying_type(self, arg_0_operand, context, program)?;
        validate_same_underlying_type(self, arg_1_operand, context, program)?;

        Ok(())
    }
}

impl Validatable for Reveal {
    fn validate(&self, context: &mut ValidationContext, program: &ProgramMIR) -> Result<()> {
        // Check that the input type is not public.
        let this_operand = program.operation(self.this)?;
        if this_operand.ty().is_public() {
            context.report_error(self, &format!("reveal input type is public: {:?}", this_operand.ty()), program)?;
            return Ok(());
        }
        // Check that the return type is public.
        if !self.ty().is_public() {
            context.report_error(self, &format!("reveal output type is not public: {:?}", self.ty()), program)?;
            return Ok(());
        }

        Ok(())
    }
}

impl Validatable for PublicKeyDerive {
    fn validate(&self, context: &mut ValidationContext, program: &ProgramMIR) -> Result<()> {
        use NadaType::*;
        let this_operand = program.operation(self.this)?;

        // Check that the operand has a compatible type
        match this_operand.ty() {
            EcdsaPrivateKey => {}
            EddsaPrivateKey => {}
            _ => {
                context.report_error(
                    self,
                    "public key derive operation is not supported for the given type",
                    program,
                )?;
                return Ok(());
            }
        }

        Ok(())
    }
}

impl Validatable for TruncPr {
    fn validate(&self, context: &mut ValidationContext, program: &ProgramMIR) -> Result<()> {
        // Check that the return type is always secret.
        if !self.ty().is_secret() {
            context.report_error(
                self,
                "probabilistic truncation output type is not secret (use >> instead)",
                program,
            )?;
            return Ok(());
        }

        Ok(())
    }
}

impl Validatable for EcdsaSign {
    fn validate(&self, context: &mut ValidationContext, program: &ProgramMIR) -> Result<()> {
        use NadaType::*;
        let left = program.operation(self.left)?;
        let right = program.operation(self.right)?;

        // Check that the operands have compatible types
        match (left.ty(), right.ty()) {
            (EcdsaPrivateKey, EcdsaDigestMessage) => {}
            (_, _) => {
                context.report_error(self, "ecdsa sign operation is not supported for the given types", program)?;
                return Ok(());
            }
        }

        // Check that the return type is secret
        if !self.ty.is_secret() {
            context.report_error(self, "private ecdsa signature output type is not secret", program)?;
            return Ok(());
        }

        Ok(())
    }
}

impl Validatable for EddsaSign {
    fn validate(&self, context: &mut ValidationContext, program: &ProgramMIR) -> Result<()> {
        use NadaType::*;
        let left = program.operation(self.left)?;
        let right = program.operation(self.right)?;

        // Check that the operands have compatible types
        match (left.ty(), right.ty()) {
            (EddsaPrivateKey, EddsaMessage) => {}
            (_, _) => {
                context.report_error(self, "eddsa sign operation is not supported for the given types", program)?;
                return Ok(());
            }
        }

        Ok(())
    }
}

impl Validatable for Equals {
    fn validate(&self, context: &mut ValidationContext, program: &ProgramMIR) -> Result<()> {
        let left = program.operation(self.left)?;
        let right = program.operation(self.right)?;

        use NadaType::*;
        // Check that the operands have compatible types
        match (left.ty(), right.ty()) {
            (UnsignedInteger, UnsignedInteger)
            | (Integer, Integer)
            | (SecretUnsignedInteger, SecretUnsignedInteger)
            | (SecretInteger, SecretInteger)
            | (SecretUnsignedInteger, UnsignedInteger)
            | (SecretInteger, Integer)
            | (UnsignedInteger, SecretUnsignedInteger)
            | (Integer, SecretInteger)
            | (Boolean, Boolean)
            | (SecretBoolean, SecretBoolean)
            | (Boolean, SecretBoolean)
            | (SecretBoolean, Boolean) => {}
            (_, _) => {
                context.report_error(self, "equals operation is not supported for the given types", program)?;
                return Ok(());
            }
        }

        // Check that the return type is secret when at least one is secret.
        if (left.ty().is_secret() || right.ty().is_secret()) && self.ty.is_public() {
            context.report_error(
                self,
                "private output equality output type is not secret with private inputs",
                program,
            )?;
            return Ok(());
        }

        // Check that the return type is public when both are public
        if left.ty().is_public() && right.ty().is_public() && !self.ty.is_public() {
            context.report_error(
                self,
                "private output equality output type is not public with both public inputs",
                program,
            )?;
            return Ok(());
        }
        Ok(())
    }
}

impl Validatable for NotEquals {
    fn validate(&self, context: &mut ValidationContext, program: &ProgramMIR) -> Result<()> {
        let left = program.operation(self.left)?;
        let right = program.operation(self.right)?;

        use NadaType::*;
        // Check that the operands have compatible types
        match (left.ty(), right.ty()) {
            (UnsignedInteger, UnsignedInteger)
            | (Integer, Integer)
            | (SecretUnsignedInteger, SecretUnsignedInteger)
            | (SecretInteger, SecretInteger)
            | (SecretUnsignedInteger, UnsignedInteger)
            | (SecretInteger, Integer)
            | (UnsignedInteger, SecretUnsignedInteger)
            | (Integer, SecretInteger)
            | (Boolean, Boolean)
            | (SecretBoolean, SecretBoolean)
            | (Boolean, SecretBoolean)
            | (SecretBoolean, Boolean) => {}
            (_, _) => {
                context.report_error(self, "not-equals operation is not supported for the given types", program)?;
                return Ok(());
            }
        }

        // Check that the return type is secret when at least one is secret.
        if (left.ty().is_secret() || right.ty().is_secret()) && self.ty.is_public() {
            context.report_error(self, "not-equals output type is not secret with private inputs", program)?;
            return Ok(());
        }

        // Check that the return type is public when both are public
        if left.ty().is_public() && right.ty().is_public() && !self.ty.is_public() {
            context.report_error(self, "not-equals output type is not public with both public inputs", program)?;
            return Ok(());
        }
        Ok(())
    }
}

#[duplicate_item(operation_; [LessThan]; [GreaterThan]; [PublicOutputEquality])]
impl Validatable for operation_ {
    fn validate(&self, context: &mut ValidationContext, program: &ProgramMIR) -> Result<()> {
        let left_operand = program.operation(self.left)?;
        let right_operand = program.operation(self.right)?;

        validate_same_underlying_type(left_operand, right_operand, context, program)?;

        let expected_ty =
            if (left_operand.ty().is_secret() || right_operand.ty().is_secret()) && !self.public_output_only() {
                NadaType::SecretBoolean
            } else {
                NadaType::Boolean
            };

        if &expected_ty != self.ty() {
            context.report_unexpected_type(&expected_ty, self, program)?;
        }

        Ok(())
    }
}

impl Validatable for Not {
    fn validate(&self, context: &mut ValidationContext, program: &ProgramMIR) -> Result<()> {
        let operand = program.operation(self.this)?;
        if self.ty != *operand.ty() && *operand.ty() != NadaType::Boolean && *operand.ty() != NadaType::SecretBoolean {
            context.report_error(self, "not operation is not supported for the given type", program)?;
        }
        Ok(())
    }
}

impl Validatable for ArrayAccessor {
    fn validate(&self, context: &mut ValidationContext, program: &ProgramMIR) -> Result<()> {
        let array_source = program.operation(self.source)?;
        // Check that the array accessor type is an array
        if let NadaType::Array { size, inner_type } = array_source.ty() {
            if self.index >= *size {
                context.report_error(self, "array accessor out of bounds", program)?
            }
            if self.ty != *(*inner_type) {
                context.report_error(self, "invalid array accessor type", program)?
            }
        } else {
            context.report_error(self, "array accessor does not refer to an array type", program)?;
        }
        Ok(())
    }
}

impl Validatable for TupleAccessor {
    fn validate(&self, context: &mut ValidationContext, program: &ProgramMIR) -> Result<()> {
        let tuple_source = program.operation(self.source)?;
        // Check that the tuple accessor type is an tuple
        if let NadaType::Tuple { left_type, right_type } = tuple_source.ty() {
            let tuple_index_type = match self.index {
                TupleIndex::Left => left_type,
                TupleIndex::Right => right_type,
            };
            if self.ty != *(*tuple_index_type) {
                context.report_error(self, "invalid tuple accessor type", program)?
            }
        } else {
            context.report_error(self, "tuple accessor does not refer to an tuple type", program)?;
        }
        Ok(())
    }
}

impl Validatable for InnerProduct {
    fn validate(&self, context: &mut ValidationContext, program: &ProgramMIR) -> Result<()> {
        if let (
            NadaType::Array { inner_type: left_inner_type, size: left_size },
            NadaType::Array { inner_type: right_inner_type, size: right_size },
        ) = (program.operation(self.left)?.ty(), program.operation(self.right)?.ty())
        {
            if left_size != right_size {
                context.report_error(self, "inner product of mismatched array sizes", program)?
            }
            if !left_inner_type.is_primitive() || !right_inner_type.is_primitive() {
                context.report_error(self, "inner product of invalid array inner types", program)?
            }
            if !left_inner_type.has_same_underlying_type(right_inner_type) {
                context.report_error(self, "inner product of mismatched array inner types", program)?
            }
        } else {
            context.report_error(self, "inner product is not applied to array types", program)?;
        }
        Ok(())
    }
}

impl Validatable for Operation {
    fn validate(&self, context: &mut ValidationContext, program: &ProgramMIR) -> Result<()> {
        use Operation::*;

        match self {
            Addition(o) => o.validate(context, program),
            ArrayAccessor(o) => o.validate(context, program),
            Division(o) => o.validate(context, program),
            Equals(o) => o.validate(context, program),
            NotEquals(o) => o.validate(context, program),
            GreaterThan(o) => o.validate(context, program),
            IfElse(o) => o.validate(context, program),
            InputReference(_) | LiteralReference(_) | Random(_) => Ok(()),
            LeftShift(o) => o.validate(context, program),
            LessThan(o) => o.validate(context, program),
            Modulo(o) => o.validate(context, program),
            Multiplication(o) => o.validate(context, program),
            New(o) => o.validate(context, program),
            Not(o) => o.validate(context, program),
            Power(o) => o.validate(context, program),
            PublicOutputEquality(o) => o.validate(context, program),
            Reveal(o) => o.validate(context, program),
            PublicKeyDerive(o) => o.validate(context, program),
            RightShift(o) => o.validate(context, program),
            Subtraction(o) => o.validate(context, program),
            TruncPr(o) => o.validate(context, program),
            TupleAccessor(o) => o.validate(context, program),
            InnerProduct(o) => o.validate(context, program),
            EcdsaSign(o) => o.validate(context, program),
            EddsaSign(o) => o.validate(context, program),

            Cast(_)
            | GreaterOrEqualThan(_)
            | LessOrEqualThan(_)
            | Map(_)
            | NadaFunctionArgRef(_)
            | NadaFunctionCall(_)
            | Reduce(_)
            | Unzip(_)
            | Zip(_)
            | BooleanAnd(_)
            | BooleanOr(_)
            | BooleanXor(_) => Err(anyhow!("operation {self:?} is not supported")),
        }
    }
}

/// Validator implementation
pub trait Validator {
    /// Check if the model is well-built
    fn validate(&self) -> Result<ValidationContext>;
}

impl Validator for ProgramMIR {
    fn validate(&self) -> Result<ValidationContext> {
        let mut context = ValidationContext::default();
        validate_inputs(self, &mut context).with_context(|| format!("MIR inputs validation:\n{}", self.text_repr()))?;
        validate_outputs(self, &mut context)
            .with_context(|| format!("MIR outputs validation:\n{}", self.text_repr()))?;
        validate_operations(self, &mut context)
            .with_context(|| format!("MIR operations validation:\n{}", self.text_repr()))?;
        Ok(context)
    }
}

fn check_referenced_inputs<'a, I: IntoIterator<Item = &'a Operation>>(
    mir: &ProgramMIR,
    operations: I,
    inputs: &HashMap<&str, &'a Input>,
    context: &mut ValidationContext,
) -> Result<HashSet<String>> {
    let mut used_inputs = HashSet::new();
    for operation in operations {
        if let Operation::InputReference(input_ref) = operation {
            let input_name = &input_ref.refers_to;
            if inputs.contains_key(input_name.as_str()) {
                used_inputs.insert(input_name.to_string());
            } else {
                context.report_error(input_ref, &format!("input {input_name} is used, but it is not defined"), mir)?;
            }
        }
    }
    Ok(used_inputs)
}

/// Inputs validation check:
/// - inputs are declared once.
/// - inputs are used at least once
/// - the program doesn't use undefined inputs
fn validate_inputs(mir: &ProgramMIR, context: &mut ValidationContext) -> Result<()> {
    let mut inputs_by_name: HashMap<&str, Vec<&Input>> = HashMap::new();

    // Inputs are declared once: inputs counting
    for input in mir.inputs.iter() {
        let inputs = inputs_by_name.entry(&input.name).or_default();
        inputs.push(input);
    }
    // Inputs are declared once: counts check
    let mut inputs_index = HashMap::default();
    for (input_name, mut inputs) in inputs_by_name {
        let count = inputs.len();
        if let Some(input) = inputs.pop() {
            if !inputs.is_empty() {
                context.report_error(input, &format!("input {input_name} is repeated {count} times"), mir)?;
            }
            inputs_index.insert(input_name, input);
        }
    }

    // Inputs are used at least once
    let mut used_inputs = check_referenced_inputs(mir, mir.operations.values(), &inputs_index, context)?;
    for function in mir.functions.iter() {
        used_inputs.extend(check_referenced_inputs(mir, function.operations.values(), &inputs_index, context)?);
    }
    for (input_name, input) in inputs_index {
        if !used_inputs.contains(input_name) {
            context.report_error(input, &format!("input {input_name} is not used"), mir)?;
        }
    }
    Ok(())
}

/// Outputs validation check:
/// - each output type matches inner operation type
fn validate_outputs(mir: &ProgramMIR, context: &mut ValidationContext) -> Result<()> {
    for output in mir.outputs.iter() {
        let inner_operand = mir.operation(output.operation_id)?;
        validate_same_underlying_type(output, inner_operand, context, mir)?;
    }

    Ok(())
}

/// Operation validation
fn validate_operations(mir: &ProgramMIR, context: &mut ValidationContext) -> Result<()> {
    if mir.operations.is_empty() {
        return Err(anyhow!(
            "This program has no operations in it. If you need to implement this behaviour you should use store / retrieve secrets instead"
        ));
    }
    for operation in mir.operations.values() {
        operation.validate(context, mir)?;
    }

    Ok(())
}

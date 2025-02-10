//! Test program generator
//!
//! Currently it supports only binary operations
use crate::{input_provider::InputProvider, runner::TestCase};
use anyhow::{anyhow, Context, Result};

use colored::Colorize;
use evalexpr::{
    eval, eval_number, eval_with_context, ContextWithMutableFunctions, ContextWithMutableVariables, EvalexprError,
    Function, HashMapContext, Value,
};
use itertools::Itertools;
use log::{debug, info};
use operations::types::{BuiltOperations, DataType, Literal, PythonShape, UnderlyingType};
use serde_files_utils::string::write_string;
use std::{
    fs::{create_dir, remove_dir_all},
    path::Path,
};

/// Temporarily disabled operations
static DISABLED_OPERATIONS: [&str; 1] = ["TruncPr"];

/// Renders a program
///
/// - with any number of inputs that belong to a single party
/// - one operation that might be simple or composed of other operations
fn render_program(inputs: &[ProgramInput], operations: &[String]) -> String {
    let result = format!("r{}", operations.len().wrapping_sub(1));
    let inputs = inputs
        .iter()
        .map(|input| input_nada_declaration(&input.name, &input.ty, &input.value))
        .collect::<Vec<_>>()
        .join("\n    ");
    let operations = operations.iter().join("\n    ");
    format!(
        r#"
from nada_dsl import *

def nada_main():
    party1 = Party(name="Dealer")
    {inputs}
    {operations}
    return [Output({result}, "result", party1)]
"#,
    )
}

/// Checks if data type is supported
///
/// It allows filtering of generated tests to contain only supported data types.
/// Currently, the biggest limitation is lack of support of Array and Tuple data types.
fn data_type_is_supported(data_type: &DataType) -> bool {
    matches!(data_type.underlying_type(), Some(UnderlyingType::Integer) | Some(UnderlyingType::UnsignedInteger))
}

fn literal_input_string(type_name: &str, value: &str) -> String {
    format!("{type_name}({value})")
}

fn type_input_string(type_name: &str, input_name: &str) -> String {
    // TODO review this when we have FixedPoint support
    // let extra_args = if data_type.has_digits() { format!(", {DIGITS_INPUT_STR}") } else { "".to_owned() };
    format!("{type_name}(Input(name=\"{input_name}\", party=party1) )")
}

fn input_nada_declaration(input_name: &str, data_type: &DataType, value: &str) -> String {
    let type_name = data_type.name();
    if matches!(data_type, DataType::Literal(_)) {
        format!("{input_name} = {}", &literal_input_string(&type_name, value))
    } else {
        format!("{input_name} = {}", &type_input_string(&type_name, input_name))
    }
}

/// Generates the Python code for any operation as a string
fn generate_operation(operation: &PythonShape, left_input: &str, right_input: &str, output: &str) -> String {
    match &operation {
        PythonShape::BinaryOperator { symbol, .. } => format!("{output} = {left_input} {symbol} {right_input}"),
        PythonShape::InstanceMethod { name, .. } => format!("{output} = {left_input}.{name}({right_input})"),
    }
}

/// A program input
#[derive(Debug, Clone)]
pub struct ProgramInput {
    pub name: String,
    pub value: String,
    pub ty: DataType,
}

impl ProgramInput {
    fn new(name: &str, value: &str, ty: DataType) -> Self {
        Self { name: name.to_owned(), value: value.to_owned(), ty }
    }
}

/// Evaluates an operation string with a set of inputs and returns the result
fn evaluate_operation(inputs: &[ProgramInput], operation: &str) -> Result<f64> {
    // Remove the assignment
    let operation = operation.split_once(" = ").map(|o| o.1).unwrap_or(operation);
    // Replace some operations so they can be understood by the expression evaluation crate.
    let mut operation = operation
        .replace("**", "^") // Different way of representing exponentiation (power)
        .replace(".public_equals", "==") // Unsupported
        .replace(".trunc_pr", ">>"); // Unsupported

    // The built-in left and right shifts panic in case an overflow occurs, so we have to replace them with our own non-
    // panicking version. See shift_operation below for the implementation.
    if operation.contains("<<") {
        operation = format!("safe_shl({})", operation.replace(" << ", ", "));
    }
    if operation.contains(">>") {
        operation = format!("safe_shr({})", operation.replace(" >> ", ", "));
    }

    let mut context = HashMapContext::new();

    fn shift_operation<F>(argument: &Value, shift_fn: F) -> Result<Value, EvalexprError>
    where
        F: Fn(i64, u32) -> Option<i64>,
    {
        let parameters = argument.as_fixed_len_tuple(2)?;
        let [left, right] = parameters.as_slice() else {
            return Err(EvalexprError::CustomMessage("expected two parameters".into()));
        };
        let (left, right) = (left.as_int()?, right.as_int()?);
        let right: u32 = right
            .try_into()
            .map_err(|_| EvalexprError::CustomMessage("expected an u32 as the right parameter for a shift".into()))?;
        let result = shift_fn(left, right).ok_or_else(|| EvalexprError::CustomMessage("overflow".into()))?;
        Ok(Value::from(result))
    }

    context
        .set_function("safe_shl".into(), Function::new(|argument| shift_operation(argument, i64::checked_shl)))
        .unwrap();
    context
        .set_function("safe_shr".into(), Function::new(|argument| shift_operation(argument, i64::checked_shr)))
        .unwrap();

    // Insert input values into a context
    for input in inputs.iter() {
        context
            .set_value(
                input.name.clone(),
                eval(&input.value).with_context(|| anyhow!("evaluating input {}", input.value))?,
            )
            .with_context(|| anyhow!("setting value {} to {}", input.name, input.value))?;
    }

    // Evaluate the operation
    let result = eval_with_context(&operation, &context).unwrap_or(Value::from(f64::NAN));
    Ok(match result {
        Value::Float(value) => value,
        Value::Int(value) => value as f64,
        Value::Boolean(value) => {
            if value {
                1.
            } else {
                0.
            }
        }
        Value::String(_) | Value::Tuple(_) | Value::Empty => f64::NAN,
    })
}

#[derive(Debug, PartialEq)]
enum CheckResult {
    Pass(f64),
    NegativeUnsignedInteger,
    InvalidValue,
}

/// Evaluates an operation with a set of inputs to check if it is mathematically sound
fn check_operation(inputs: &[ProgramInput], operation: &str) -> Result<CheckResult> {
    // Check if any of the inputs has a negative unsigned integer
    for input in inputs.iter() {
        if input.ty.is_unsigned() {
            let value = eval_number(&input.value)?;
            if value < 0. {
                return Ok(CheckResult::NegativeUnsignedInteger);
            }
        }
    }

    let result = evaluate_operation(inputs, operation)?;
    let input_is_unsigned = inputs.iter().any(|input| input.ty.is_unsigned());

    // Check if the result will be a negative unsigned integer
    if input_is_unsigned && result < 0. {
        return Ok(CheckResult::NegativeUnsignedInteger);
    }

    // Check if the result is infinite or NaN (often due to a division by zero)
    if result.is_infinite() || result.is_nan() {
        return Ok(CheckResult::InvalidValue);
    }

    Ok(CheckResult::Pass(result))
}

fn print_no_inputs_for_test(program_name: &str) {
    print!("{}", "⚠".yellow());
    println!(" Ignoring test {program_name} because we could not find any valid inputs");
}

/// Generate the name for a program from the program definition
fn program_name(mut shapes: Vec<&PythonShape>, mut input_types: Vec<&DataType>) -> String {
    shapes.reverse();
    input_types.reverse();
    let mut program_name_parts = vec![];
    while let Some(shape) = shapes.pop() {
        program_name_parts.push(shape.name().to_string());
        if let Some(ty) = input_types.pop() {
            program_name_parts.push(ty.to_string());
        }
    }
    while let Some(ty) = input_types.pop() {
        program_name_parts.push(ty.to_string());
    }
    program_name_parts.iter().join("_").replace(' ', "_")
}

/// Assign a name for each input
fn assign_input_name(input_types: Vec<&DataType>) -> Vec<(String, &DataType)> {
    input_types.into_iter().enumerate().map(|(index, ty)| (format!("input_{index}"), ty)).collect()
}

/// Generate a list of operations.
fn generate_operations(inputs: &[(String, &DataType)], mut shapes: Vec<&PythonShape>) -> Result<Vec<String>> {
    let mut operations = Vec::with_capacity(shapes.len());
    let mut input_names: Vec<_> = inputs.iter().map(|(name, _)| name.clone()).collect();
    shapes.reverse();
    for (index, shape) in shapes.into_iter().enumerate() {
        match shape {
            PythonShape::BinaryOperator { .. } | PythonShape::InstanceMethod { .. } => {
                let right = input_names.pop().ok_or_else(|| anyhow!("not enough inputs"))?;
                let left = input_names.pop().ok_or_else(|| anyhow!("not enough inputs"))?;
                let result = format!("r{index}");
                operations.push(generate_operation(shape, &left, &right, &result));
                input_names.push(result);
            }
        }
    }
    Ok(operations)
}

/// Generate the input values for a program. The program is provided as a list of operations
fn generate_inputs(
    program_name: &str,
    inputs: Vec<(String, &DataType)>,
    operations: &[String],
) -> Result<Vec<ProgramInput>> {
    let mut provider = InputProvider::new();
    // This will be used to evaluate the operations later. We don't have support for casting for now
    // so all types are the same.
    let intermediate_value_type = if inputs.first().ok_or_else(|| anyhow!("no inputs"))?.1.is_unsigned() {
        DataType::Literal(Literal::UnsignedInteger)
    } else {
        DataType::Literal(Literal::Integer)
    };
    'new_inputs: loop {
        let mut program_inputs = Vec::with_capacity(inputs.len());
        // Generate values for every inputs
        for (input, ty) in inputs.iter() {
            let Ok(value) = provider.provide(**ty) else {
                print_no_inputs_for_test(program_name);
                return Ok(vec![]);
            };
            program_inputs.push(ProgramInput::new(input, &value, **ty))
        }

        let mut runtime_values = program_inputs.clone();
        for (index, operation) in operations.iter().enumerate() {
            let operation_result = match check_operation(&runtime_values, operation) {
                Ok(CheckResult::Pass(result)) => result,
                Ok(result) => {
                    debug!("{program_name}: detected issue with inputs: {inputs:?} ({result:?})",);
                    continue 'new_inputs;
                }
                Err(err) => return Err(err),
            };

            // Add the calculated value as a program input, but only to check that we don't get an invalid program because
            // of the result we calculated earlier.
            // For instance if we have a / (b - c) and the result of (b - c) is 0, then we would have r == 0 and we will be
            // checking (a / r). If that fails we know that our current values for a, b and c produce an invalid result.

            // Simulate integer truncation (2 / 3 == 0 in this case)
            // If we add support for rationals, we will have to revisit this, because this won't
            // be necessary
            let operation_result = operation_result.trunc();
            runtime_values.push(ProgramInput::new(
                &format!("r{index}"),
                &format!("{operation_result}"),
                intermediate_value_type,
            ));
        }
        return Ok(program_inputs);
    }
}

/// Generate the content for a program
fn generate_program_content(
    program_name: &str,
    shapes: Vec<&PythonShape>,
    input_types: Vec<&DataType>,
) -> Result<Option<(String, Vec<ProgramInput>)>> {
    debug!("Creating test program: {program_name}");
    let inputs = assign_input_name(input_types);
    let operations = generate_operations(&inputs, shapes)?;
    let program_inputs = generate_inputs(program_name, inputs, &operations)?;
    if program_inputs.is_empty() {
        return Ok(None);
    }
    Ok(Some((render_program(&program_inputs, &operations), program_inputs)))
}

/// Build a test case
fn generate_test_case(
    shapes: Vec<&PythonShape>,
    input_types: Vec<&DataType>,
    output_directory: &Path,
) -> Result<Option<TestCase>> {
    let name = program_name(shapes.clone(), input_types.clone());
    let Some((program, inputs)) = generate_program_content(&name, shapes, input_types)? else {
        return Ok(None);
    };
    let program_path = output_directory.join(format!("{name}.py"));
    write_string(&program_path, program)?;
    debug!("Created {}", program_path.to_string_lossy());
    Ok(Some(TestCase { name, program_path, compile_output: None, inputs }))
}

/// Build test cases that compose two operations compatible with the operation given.
fn generate_double_op_programs(
    python_shape: &PythonShape,
    operations: &BuiltOperations,
    left: &DataType,
    right: &DataType,
    output_directory: &Path,
    test_cases: &mut Vec<TestCase>,
) -> Result<u16> {
    let mut ignored_tests: u16 = 0;
    // loop for all the binary operations whose output data type matches `right`
    for (op_name, built_operation) in operations.binary_operations.iter() {
        debug!("Processing right double operation: {op_name}");
        let right_python_shape = &built_operation.metadata.python_shape;
        for ((op_left_type, op_right_type), _) in
            built_operation.allowed_combinations.iter().filter(|((_, _), output_type)| *output_type == right)
        {
            if let Some(test_case) = generate_test_case(
                vec![python_shape, right_python_shape],
                vec![left, op_left_type, op_right_type],
                output_directory,
            )? {
                test_cases.push(test_case);
            } else {
                ignored_tests += 1;
            }
        }
    }
    Ok(ignored_tests)
}

pub struct GeneratorOptions {
    pub(crate) single_ops: bool,
    pub(crate) double_ops: bool,
}

impl Default for GeneratorOptions {
    /// By default, all the operations are enabled
    fn default() -> Self {
        Self { single_ops: true, double_ops: true }
    }
}

impl GeneratorOptions {
    pub fn new(single_ops: bool, double_ops: bool) -> Self {
        Self { single_ops, double_ops }
    }

    pub fn from_args(single_ops_only: bool, double_ops_only: bool) -> Self {
        Self { single_ops: !double_ops_only, double_ops: !single_ops_only }
    }
}

/// Generate test programs
///
/// Generates all the test programs from the [`BuildOperations`].
pub fn generate_test_cases(
    operations: BuiltOperations,
    output_directory: &Path,
    options: &GeneratorOptions,
) -> Result<(Vec<TestCase>, u16)> {
    let mut test_cases: Vec<TestCase> = vec![];

    let _ = remove_dir_all(output_directory);
    create_dir(output_directory)?;
    // Remove disabled operations from the list of operations.
    let mut operations = operations;
    operations.binary_operations = operations
        .binary_operations
        .into_iter()
        .filter(|(name, _)| !DISABLED_OPERATIONS.contains(&name.as_str()))
        .collect();

    // Note about test values:
    // For now the auto-test runner creates a new test values provider for each program, so we have to do the same
    // so that the test values match between the runner and the generators below.
    let mut ignored_tests: u16 = 0;
    for (_, binary_op) in operations.binary_operations.iter() {
        let python_shape = &binary_op.metadata.python_shape;
        for (left, right) in binary_op.allowed_combinations.keys() {
            // Filter out unsupported data types
            if !data_type_is_supported(left) | !data_type_is_supported(right) {
                info!("Unsupported data type {} for operation {}", left, python_shape.name());
                continue;
            }
            if options.single_ops {
                let Some(test_case) = generate_test_case(vec![python_shape], vec![left, right], output_directory)?
                else {
                    // Evaluation failed, so we ignore this test
                    ignored_tests += 1;
                    continue;
                };
                test_cases.push(test_case);
            }

            if options.double_ops {
                // Now generate double operation cases
                ignored_tests += generate_double_op_programs(
                    python_shape,
                    &operations,
                    left,
                    right,
                    output_directory,
                    &mut test_cases,
                )?;
            }
        }
    }

    Ok((test_cases, ignored_tests))
}

use std::{fs, path::Path};

use anyhow::{anyhow, Context};

use crate::types::{BuiltBinaryOperation, BuiltOperations, DataType, PythonShape, Side, TestValue};

const SPACES: &str = "        ";

const BINARY_OPERATORS: &str = r#"@pytest.mark.parametrize(
    ("operator", "name", "ty"),
    [
{operators}
    ],
)
def test_binary_operator{index}_{left_name}_{right_name}(operator, name, ty):
    left = {left}
    right = {right}
    program_operation = operator(left, right)
    # recover operation from AST
    ast_operation = AST_OPERATIONS[program_operation.inner.id]
    op = process_operation(ast_operation, {}).mir
    assert list(op.keys()) == [name]
    print(ast_operation)
    print(program_operation)
    print(op)
    inner = op[name]
    print(inner)
    {left_ast}
    {right_ast}
    {left_output}
    {right_output}
    assert inner["type"] == to_type(ty)"#;

fn generate_binary_operations(operations: &BuiltOperations, contents: &mut String) -> anyhow::Result<()> {
    let mut index = 0;
    for left_type in DataType::all_types() {
        for right_type in DataType::all_types() {
            let mut operators = Vec::new();

            for (name, binary_operation) in &operations.binary_operations {
                if let Some(output) = binary_operation.allowed_combinations.get(&(left_type, right_type)) {
                    let output_data_type = output.name();
                    let name = if matches!(output, DataType::Literal(_)) { "LiteralReference" } else { name };
                    let left_type_name = left_type.name();

                    let operator_string = match &binary_operation.metadata.python_shape {
                        PythonShape::BinaryOperator { name: operator_name, .. } => {
                            let py_op = python_operator_name(operator_name);
                            format!("{SPACES}(operator.{py_op}, \"{name}\", \"{output_data_type}\")")
                        }
                        PythonShape::InstanceMethod { name: function_name } => {
                            format!("{SPACES}({left_type_name}.{function_name}, \"{name}\", \"{output_data_type}\")")
                        }
                    };

                    operators.push((operator_string, binary_operation));
                }
            }

            if operators.is_empty() {
                continue;
            }

            let left = if matches!(left_type, DataType::Literal(_)) {
                "create_literal({left_shape_type}, {left_value})"
            } else {
                "create_input({left_shape_type}, \"left\", \"party\")"
            };

            let right = if matches!(right_type, DataType::Literal(_)) {
                "create_literal({right_shape_type}, {right_value})"
            } else {
                "create_input({right_shape_type}, \"right\", \"party\")"
            };

            let (left_output, right_output) =
                if matches!(left_type, DataType::Literal(_)) && matches!(right_type, DataType::Literal(_)) {
                    ("", "") // If both are literals then we calculate the result directly.
                // TODO: check the result.
                } else {
                    (
                        if matches!(left_type, DataType::Literal(_)) {
                            "assert isinstance(left_ast, LiteralASTOperation) and len(left_ast.literal_name) == 32" // MD5 hash length
                        } else {
                            "assert isinstance(left_ast, InputASTOperation) and left_ast.name == \"left\""
                        },
                        if matches!(right_type, DataType::Literal(_)) {
                            "assert isinstance(right_ast, LiteralASTOperation) and len(right_ast.literal_name) == 32" // MD5 hash length
                        } else {
                            "assert isinstance(right_ast, InputASTOperation) and right_ast.name == \"right\""
                        },
                    )
                };
            // TODO do this better. If we check result we need to collect left and right from AST
            let (left_ast, right_ast) = if left_output.is_empty() {
                ("", "")
            } else {
                ("left_ast = AST_OPERATIONS[inner[\"left\"]]", "right_ast = AST_OPERATIONS[inner[\"right\"]]")
            };

            fn is_acceptable_value(
                left_test_value: &TestValue,
                right_test_value: &TestValue,
                operation: &BuiltBinaryOperation,
            ) -> bool {
                match operation.metadata.forbid_zero {
                    Some(Side::Left) | Some(Side::Both) if left_test_value.is_zero() => false,
                    Some(Side::Right) | Some(Side::Both) if right_test_value.is_zero() => false,
                    _ => true,
                }
            }

            for left_test_value in left_type.test_values() {
                for right_test_value in right_type.test_values() {
                    let valid_operators = operators
                        .iter()
                        .filter(|operator| is_acceptable_value(left_test_value, right_test_value, operator.1))
                        .map(|operator| operator.0.clone())
                        .collect::<Vec<_>>()
                        .join(",\n");

                    contents.push_str(
                        &BINARY_OPERATORS
                            .replace("{index}", &index.to_string())
                            .replace("{left}", left)
                            .replace("{right}", right)
                            .replace("{left_output}", left_output)
                            .replace("{right_output}", right_output)
                            .replace("{left_shape_type}", &left_type.name())
                            .replace("{right_shape_type}", &right_type.name())
                            .replace("{left_ast}", left_ast)
                            .replace("{right_ast}", right_ast)
                            .replace("{left_value}", &left_test_value.to_string())
                            .replace("{right_value}", &right_test_value.to_string())
                            .replace("{left_name}", &left_type.name().to_lowercase())
                            .replace("{right_name}", &right_type.name().to_lowercase())
                            .replace("{operators}", &valid_operators),
                    );
                    contents.push('\n');
                    index += 1;
                }
            }
            contents.push('\n');
        }
    }

    Ok(())
}

/// Returns the corresponding operator name in the Python operator interface
fn python_operator_name(operator_name: &str) -> &str {
    match operator_name {
        "and" => "and_",
        "or" => "or_",
        _ => operator_name,
    }
}

fn truncate_after_line(input: &str, target: &str) -> anyhow::Result<String> {
    let position = input
        .lines()
        .position(|line| line == target)
        .ok_or_else(|| anyhow!("unable to find the line to truncate from"))?;

    Ok(input.lines().take(position).collect::<Vec<_>>().join("\n"))
}

/// Generates Nada tests.
pub fn generate_tests(
    operations: &BuiltOperations,
    base_directory: &Path,
    target_directory: &Path,
) -> anyhow::Result<()> {
    let mut contents = String::from("# This file is automatically generated. Do not edit!\n");
    contents.push('\n');

    let base_filepath = base_directory.join("compiler_frontend_test.py");
    let existing_contents = fs::read_to_string(base_filepath.clone())
        .with_context(|| format!("failed reading base test file at {base_filepath:?}"))?;

    let existing_contents = truncate_after_line(
        &existing_contents,
        "# Generated tests are added below this line. Please leave it as it is.",
    )
    .with_context(|| "failed to find the line where to add the generated tests")?;

    contents.push_str(&existing_contents);
    contents.push_str("\n\n");

    generate_binary_operations(operations, &mut contents)?;

    fs::write(target_directory.join("compiler_frontend_generated_test.py"), &contents)
        .with_context(|| "failed writing test file")?;

    Ok(())
}

use std::env::current_dir;

use anyhow::Result;
use nada_compiler_backend::{
    mir::{InputReference, OperationIdGenerator, ProgramMIR, MIR_FILE_EXTENSION_JSON},
    validators::Validator,
};
use nada_value::NadaType;
use pynadac::Compiler;
use rstest::rstest;
use serde_files_utils::json::read_json;
use test_programs::PROGRAMS;

/// Read a mir model from the test repository
pub fn read_test_mir(test_id: &str) -> Result<ProgramMIR> {
    // This is to allow debugging with rust analyzer
    let cwd = current_dir().expect("failed to get cwd");
    let mut root = "../tests/resources/mir".to_string();
    if !cwd.ends_with("compiler-backend-tests") {
        root = format!("nada-lang/compiler-backend-tests/{root}");
    }
    let program_path = format!("{root}/{test_id}{MIR_FILE_EXTENSION_JSON}");
    read_json(program_path)
}

#[rstest]
#[case::input_single("input_single", true)]
#[case::duplicated_input("duplicated_input", false)]
#[case::incompatible_output_type("incompatible_output_type", false)]
#[case::addition_simple("addition_simple", true)]
#[case::addition_incompatible_types("addition_incompatible_types", false)]
#[case::addition_incompatible_operands("addition_incompatible_operands", false)]
#[case::subtraction_simple("subtraction_simple", true)]
#[case::multiplication_simple("multiplication_simple", true)]
#[case::division_simple("division_simple", true)]
#[case::modulo_secret_public("modulo_secret_public", true)]
#[case::circuit_simple("circuit_simple", true)]
#[case::circuit_simple_2("circuit_simple_2", true)]
#[case::less_than("less_than", true)]
#[case::less_than_incompatible_branch("less_than_incompatible_branch", false)]
#[case::less_than_incompatible_type("less_than_incompatible_type", false)]
#[case::import_file("import_file", true)]
#[case::array("array_complex", true)]
#[case::array_new("array_new", true)]
#[case::array_new_empty("array_new_empty", false)]
#[case::array_new_incompatible_types("array_new_incompatible_types", false)]
// This test is ignored because we need to add support for recursion.
#[ignore]
#[case::array_2dimensional("array2dimensional", true)]
#[case::tuple_new("tuple_new", true)]
#[case::tuple_new_empty("tuple_new_empty", false)]
#[case::map_simple("map_simple", true)]
#[case::if_else("if_else", true)]
#[case::if_else_public_public("if_else_public_public", true)]
#[case::if_else_public_secret("if_else_public_secret", true)]
#[case::if_else_unsigned("if_else_unsigned", true)]
#[case::if_else_unsigned_public_public("if_else_unsigned_public_public", true)]
#[case::if_else_public_literal_public_literal("if_else_public_literal_public_literal", true)]
#[case::if_else_secret_public_literal("if_else_secret_public_literal", true)]
#[case::if_else_unsigned_secret_public_literal("if_else_unsigned_secret_public_literal", true)]
#[case::if_else_public_cond_public_branches("if_else_public_cond_public_branches", true)]
#[case::if_else_public_cond_secret_branches("if_else_public_cond_secret_branches", true)]
#[case::if_else_reveal("if_else_reveal", true)]
#[case::if_else_reveal_secret("if_else_reveal_secret", true)]
#[case::reveal_add("reveal_add", true)]
#[case::reveal_many_operations("reveal_many_operations", true)]
#[case::reveal("reveal", true)]
#[case::reveal_unsigned("reveal_unsigned", true)]
#[case::shift_left("shift_left", true)]
#[case::shift_left_literal("shift_left_literal", true)]
#[case::shift_left_unsigned_literal("shift_left_unsigned_literal", true)]
#[case::shift_left_complex("shift_left_complex", true)]
#[case::shift_left_after_add("shift_left_after_add", true)]
#[case::shift_right("shift_right", true)]
#[case::shift_right_literal("shift_right_literal", true)]
#[case::shift_right_unsigned_literal("shift_right_unsigned_literal", true)]
#[case::shift_right_complex("shift_right_complex", true)]
#[case::shift_right_after_add("shift_right_after_add", true)]
#[case::trunc_pr("trunc_pr", true)]
#[case::trunc_pr_unsigned("trunc_pr_unsigned", true)]
#[case::trunc_pr_literal("trunc_pr_literal", true)]
#[case::trunc_pr_unsigned_literal("trunc_pr_unsigned_literal", true)]
#[case::trunc_pr_complex("trunc_pr_complex", true)]
#[case::auction_comparison_based_approach_0("auction_comparison_based_approach_0", true)]
#[case::equals("equals", true)]
#[case::equals_public("equals_public", true)]
#[case::ecdsa_sign("ecdsa_sign", true)]
fn tests(#[case] program_name: &str, #[case] is_valid: bool) -> Result<()> {
    let mir_program = if is_valid { PROGRAMS.mir(program_name)? } else { read_test_mir(program_name)? };
    let validation_result = mir_program.validate()?;

    // Only print the validation result if it is not the one we expect.
    if validation_result.is_successful() != is_valid {
        validation_result.print(&mir_program)?;
    }

    assert_eq!(validation_result.is_successful(), is_valid);
    Ok(())
}

#[rstest]
#[case::zip_incompatible_type("zip_incompatible_type")]
#[case::unzip_incompatible_type("unzip_incompatible_type")]
#[case::map_incompatible_type("map_incompatible_type")]
#[case::map_incompatible_function("map_incompatible_function")]
#[case::map_incompatible_operation("map_incompatible_operation")]
fn test_invalid_programs(#[case] program_name: &str) -> Result<()> {
    let mir_program = read_test_mir(program_name)?;
    assert!(mir_program.validate().is_err());
    Ok(())
}

#[test]
fn unused_inputs() -> Result<()> {
    let mut program = ProgramMIR::build();
    program.add_input("a", NadaType::Integer, "party");
    program.add_input("b", NadaType::Integer, "party");
    let mut id_generator = OperationIdGenerator::default();

    let a_ref = program.add_operation(InputReference::build("a", NadaType::Integer, id_generator.next_id()));
    program.add_output("output", a_ref, NadaType::Integer, "party");

    let validation_result = program.validate()?;
    assert!(!validation_result.is_successful());

    Ok(())
}

#[test]
fn undefined_inputs() -> Result<()> {
    let mut program = ProgramMIR::build();
    let mut id_generator = OperationIdGenerator::default();
    let a_ref = program.add_operation(InputReference::build("a", NadaType::Integer, id_generator.next_id()));
    program.add_output("output", a_ref, NadaType::Integer, "party");

    let validation_result = program.validate()?;
    assert!(!validation_result.is_successful());

    Ok(())
}

#[test]
fn no_compute() -> Result<()> {
    let program = r#"
from nada_dsl import *

def nada_main():
    party_bob = Party(name="Bob")
    num_1 = SecretInteger(Input(name="num_1", party=party1))

    return [Output(num_1, "d1", party_bob)]
    "#;
    let compiled_program = Compiler::compile_str(program, "no_compute");
    assert!(compiled_program.is_err());
    Ok(())
}

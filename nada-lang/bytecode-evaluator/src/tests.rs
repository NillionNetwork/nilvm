//! The bytecode evaluator tests
use crate::Evaluator;
use anyhow::{Error, Result};
use jit_compiler::{
    mir2bytecode::MIR2Bytecode,
    models::bytecode::{memory::BytecodeAddress, ProgramBytecode},
};
use math_lib::modular::{ModularNumber, U64SafePrime};
use nada_value::{clear::Clear, NadaType, NadaValue};
use rstest::rstest;
use serde_files_utils::json::read_json;
use std::{collections::HashMap, env::current_dir};
use test_programs::PROGRAMS;

type Prime = U64SafePrime;

fn run_evaluator_pred(
    test_id: &str,
    variables_file_id: &str,
    f: &dyn Fn(HashMap<String, NadaValue<Clear>>) -> Result<()>,
) -> Result<()> {
    let mut base_dir = current_dir()?;
    if !base_dir.ends_with("bytecode-evaluator") {
        base_dir.push("nada-lang/bytecode-evaluator");
    }
    let base_dir = base_dir.to_str().unwrap();
    let program_mir = &PROGRAMS.mir(test_id).expect("program not found");
    let bytecode: ProgramBytecode = MIR2Bytecode::transform(program_mir).expect("transformation failed");
    let values_file_path = format!("{base_dir}/../tests/resources/values/{variables_file_id}.json");
    let values: HashMap<String, NadaValue<Clear>> = read_json(values_file_path)?;
    let outputs = Evaluator::<Prime>::run(&bytecode, values)?;
    f(outputs)
}

fn run_evaluator(
    test_id: &str,
    variables_file_id: &str,
    expected_outputs: HashMap<String, NadaValue<Clear>>,
) -> Result<()> {
    let f = |variables: HashMap<String, NadaValue<Clear>>| {
        assert_eq!(
            expected_outputs,
            variables,
            "expected: {:?}, actual: {:?}",
            expected_outputs.values(),
            variables.values()
        );
        Ok(())
    };
    run_evaluator_pred(test_id, variables_file_id, &f)
}

#[rstest]
#[case::input_single("input_single", "default", vec![("my_output", 32)])] // [ 32 ]
#[case::addition_simple("addition_simple", "default", vec![("my_output", 113)])] // [ 32 + 81 ]
#[case::subtraction_simple("subtraction_simple", "default", vec![("my_output", 49)])] // [  81 - 32 ]
#[case::subtraction_simple_neg("subtraction_simple_neg", "default", vec![("my_output", - 21)])] // [ 81 - 102 ]
#[case::modulo_secret_secret("modulo_secret_secret", "default", vec![("my_output", - 221)])] // [ 32 % -253 ]
#[case::modulo_simple_neg("modulo_simple_neg", "default", vec![("my_output", - 151)])] // [ 102 % -253 ]
#[case::division_simple("division_simple", "default", vec![("my_output", 6)])] // [ 32 / 5 ]
#[case::division_simple_neg("division_simple_neg", "default", vec![("my_output", - 8)])] // [ -253 / 32 ]
#[case::multiplication_simple("multiplication_simple", "default", vec![("my_output", 2592)])] // [ 32 * 81 ]
#[case::circuit_simple("circuit_simple", "default", vec![("my_output", 3102)])] // [ 32 * 81 + 5 * 102 ]
#[case::circuit_simple_2("circuit_simple_2", "default", vec![("my_output", 38188)])] // [ 79 * 55 * 7 + 34 * 64 + 110 * 5 * 10 + 97 ]
#[case::import_file("import_file", "default", vec![("my_output", 113)])] // [ 32 + 81 ]
#[case::complex_operation_mix("complex_operation_mix", "default", vec![("my_output", 52209794)])]
#[case::if_else("if_else", "default", vec![("my_output", 32)])] // if 32 < 81 then { 32 } else { 81 }
#[case::if_else_public_public("if_else_public_public", "default", vec![("my_output", 32)])] // if Secret(1) < Public(81) then { Public(32) } else { Public(81) }
#[case::if_else_public_literal_public_literal("if_else_public_literal_public_literal", "default", vec![("my_output", 2)])] // if Secret(32) < Public(10) then { Public(1) } else { Public(2) }
#[case::if_else_secret_public_literal("if_else_secret_public_literal", "default", vec![("my_output", 2)])] // if 32 < 10 then { 81 } else { 2 }
#[case::if_else_public_cond_secret_branches("if_else_public_cond_secret_branches", "default", vec![("my_output", 5)])]
#[case::if_else_reveal_secret("if_else_reveal_secret", "default", vec![("my_output", 32)])]
#[case::shift_left_literal("shift_left_literal", "default", vec![("my_output", 128)])] // 32 << 2
#[case::shift_right_literal("shift_right_literal", "default", vec![("my_output", 8)])] // 32 >> 2
#[case::trunc_pr_literal("trunc_pr_literal", "default", vec![("my_output", 8)])] // 32.trunc_pr(2)
#[case::shift_left_after_add("shift_left_after_add", "default", vec![("my_output", 226)])] // (32 + 81) << 1
#[case::shift_right_after_add("shift_right_after_add", "default", vec![("my_output", 56)])] // (32 + 81) >> 1
#[case::array_inner_product("array_inner_product", "default", vec![("out", 20)])] // (1,2,3)*(2,3,4)
fn test_evaluator_integer_secrets(
    #[case] test_id: &str,
    #[case] variables_file_id: &str,
    #[case] expected_secrets: Vec<(&str, i64)>,
) -> Result<()> {
    let expected_outputs = expected_secrets
        .into_iter()
        .map(|(name, secret)| (String::from(name), NadaValue::new_secret_integer(secret)))
        .collect();
    run_evaluator(test_id, variables_file_id, expected_outputs)
}

#[rstest]
#[case::if_else_unsigned("if_else_unsigned", "default", vec![("my_output", 32)])] // if 32 < 81 then { 32 } else { 81 }
#[case::if_else_unsigned_public_public("if_else_unsigned_public_public", "default", vec![("my_output", 32)])] // if Secret(1) < Public(81) then { Public(32) } else { Public(81) }
#[case::if_else_unsigned_secret_public_literal("if_else_unsigned_secret_public_literal", "default", vec![("my_output", 1)])] // if 32 < 81 then { 1 } else { 2 }
#[case::modulo_unsigned_secret_public("modulo_unsigned_secret_public", "default", vec![("my_output", 32)])] // [ 32 % 81 ]
#[case::modulo_unsigned_public_secret("modulo_unsigned_public_secret", "default", vec![("my_output", 32)])] // [ 32 % 81 ]
#[case::modulo_unsigned_secret_secret("modulo_unsigned_secret_secret", "default", vec![("my_output", 32)])] // [ 32 % 81 ]
#[case::shift_left_unsigned_literal("shift_left_unsigned_literal", "default", vec![("my_output", 128)])] // 32 << 2
#[case::shift_right_unsigned_literal("shift_right_unsigned_literal", "default", vec![("my_output", 8)])] // 32 >> 2
#[case::trunc_pr_unsigned_literal("trunc_pr_unsigned_literal", "default", vec![("my_output", 8)])] // 32.trunc_pr(2)
fn test_evaluator_unsigned_integer_secrets(
    #[case] test_id: &str,
    #[case] variables_file_id: &str,
    #[case] expected_secrets: Vec<(&str, u64)>,
) -> Result<()> {
    let expected_outputs: HashMap<_, _> = expected_secrets
        .into_iter()
        .map(|(name, secret)| (String::from(name), NadaValue::new_secret_unsigned_integer(secret)))
        .collect();
    run_evaluator(test_id, variables_file_id, expected_outputs)
}

#[rstest]
#[case::addition_simple_public_secret("addition_simple_public_secret", "default", vec![("my_output", 113)])] // [ 32 + 81 ]
#[case::addition_simple_secret_public("addition_simple_secret_public", "default", vec![("my_output", 113)])] // [ 32 + 81 ]
#[case::addition_simple_literal_secret("addition_simple_literal_secret", "default", vec![("my_output", 45)])] // [ 32 + 81 ]
#[case::modulo_public_secret("modulo_public_secret", "default", vec![("my_output", 32)])] // [ 32 % 81 ]
#[case::modulo_secret_public("modulo_secret_public", "default", vec![("my_output", 32)])] // [ 32 % 81 ]
#[case::if_else_public_secret("if_else_public_secret", "default", vec![("my_output", 32)])] // if Public(32) < Secret(81) then { Public(32) } else { Secret(81) }
fn test_evaluator_integer_secret_public(
    #[case] test_id: &str,
    #[case] variables_file_id: &str,
    #[case] expected_secrets: Vec<(&str, i64)>,
) -> Result<()> {
    let expected_outputs = expected_secrets
        .into_iter()
        .map(|(name, secret)| (String::from(name), NadaValue::new_secret_integer(secret)))
        .collect();
    run_evaluator(test_id, variables_file_id, expected_outputs)
}

#[rstest]
#[case::less_than("less_than", "default", vec![("my_output", false)])] // [ 79 * 55 + 7 < 55 * 34 ]
#[case::less_than_simple_neg("less_than_simple_neg", "default", vec![("my_output", true)])] // [ - 79 < - 55 ]
#[case::less_than_addition_neg("less_than_addition_neg", "default", vec![("my_output", true)])] // [ - 79 < (- 55 + 7 )]
#[case::greater_than("greater_than", "default", vec![("my_output", true)])] // [ 79 * 55 + 7 > 55 * 34 ]
#[case::less_or_equal_than("less_or_equal_than", "default", vec![("my_output", false)])] // [ 79 * 55 + 7 <= 55 * 34 ]
#[case::less_or_equal_than_simple("less_or_equal_than_simple", "default", vec![("my_output", false)])] // [ 79 <= 55 ]
#[case::greater_or_equal_than("greater_or_equal_than", "default", vec![("my_output", true)])] // [ 79 * 55 + 7 >= 55 * 34 ]
#[case::greater_or_equal_mul("greater_equal_mul", "default", vec![("my_output", true)])] // [ 81 >= 32 * -42]
#[case::equals_private_output("equals", "default", vec![("my_output", false)])] // [ (79 * 55 + 7) == (55 * 34) ]
#[case::boolean_and("boolean_and", "default", vec![("my_output", true)])] // [ - 79 < (- 55 + 7 ) & -79 < 7]
#[case::boolean_or("boolean_or", "default", vec![("my_output", true)])] // [ - 79 < (- 55 + 7 ) | -79 < 7]
#[case::boolean_xor("boolean_xor", "default", vec![("my_output", false)])] // [ - 79 < (- 55 + 7 ) ^ -79 < 7]
fn test_evaluator_boolean_secrets(
    #[case] test_id: &str,
    #[case] variables_file_id: &str,
    #[case] expected_secrets: Vec<(&str, bool)>,
) -> Result<()> {
    let expected_outputs: HashMap<_, _> = expected_secrets
        .into_iter()
        .map(|(name, secret)| (String::from(name), NadaValue::new_secret_boolean(secret)))
        .collect();
    run_evaluator(test_id, variables_file_id, expected_outputs)
}

#[rstest]
#[case::addition_simple_literal_literal("addition_simple_literal_literal", "default", vec![("my_output", 26)])]
#[case::if_else_public_cond_public_branches("if_else_public_cond_public_branches", "default", vec![("my_output", 2)])]
#[case::if_else_reveal("if_else_reveal", "default", vec![("my_output", 1)])]
#[case::modulo_public_public("modulo_public_public", "default", vec![("my_output", 32)])] // [ 32 % 81 ]
#[case::reveal("reveal", "default", vec![("my_output", 7776)])]
// prod = (32*81) = 2592, sum = (32+81).to_public() = 113, mod = (32%3) = 2,
// tmp_1 = prod.to_public() / 2 = 1296, tmp_2 = sum.to_public() + mod.to_public() = 115
// output = tmp_1 + tmp_2 = 1411
#[case::reveal_many_operations("reveal_many_operations", "default", vec![("my_output", 1411)])]
fn test_evaluator_integer_public_variables(
    #[case] test_id: &str,
    #[case] variables_file_id: &str,
    #[case] expected_public_variables: Vec<(&str, i64)>,
) -> Result<()> {
    let expected_outputs: HashMap<_, _> = expected_public_variables
        .into_iter()
        .map(|(name, public_variable)| (String::from(name), NadaValue::new_integer(public_variable)))
        .collect();
    run_evaluator(test_id, variables_file_id, expected_outputs)
}

#[rstest]
#[case::modulo_unsigned_literal_public("modulo_unsigned_literal_public", "default", vec![("my_output", 1)])]
#[case::reveal_unsigned("reveal_unsigned", "default", vec![("my_output", 7776)])] // (32*81).to_public() * 3
#[case::modulo_unsigned_public_public("modulo_unsigned_public_public", "default", vec![("my_output", 32)])] // [ 32 % 81 ]
fn test_evaluator_unsigned_integer_public_variables(
    #[case] test_id: &str,
    #[case] variables_file_id: &str,
    #[case] expected_public_variables: Vec<(&str, u64)>,
) -> Result<()> {
    let expected_outputs: HashMap<_, _> = expected_public_variables
        .into_iter()
        .map(|(name, public_variable)| (String::from(name), NadaValue::new_unsigned_integer(public_variable)))
        .collect();
    run_evaluator(test_id, variables_file_id, expected_outputs)
}

#[rstest]
#[case::less_than_public_variables("less_than_public_variables", "default", vec![("my_output", false)])] // [ 79 * 55 + 7 < 55 * 34 ]
#[case::greater_than_public_variables("greater_than_public_variables", "default", vec![("my_output", true)])] // [ 79 * 55 + 7 > 55 * 34 ]
#[case::less_or_equal_than_public_variables("less_or_equal_than_public_variables", "default", vec![("my_output", false)])] // [ 79 * 55 + 7 <= 55 * 34 ]
#[case::greater_or_equal_than_public_variables("greater_or_equal_than_public_variables", "default", vec![("my_output", true)])] // [ 79 * 55 + 7 >= 55 * 34 ]
#[case::less_or_equal_than_literals("less_or_equal_than_literals", "default", vec![("my_output", true)])]
#[case::equals_public_output("public_output_equality", "default", vec![("my_output", false)])] // [ (79 * 55 + 7).public_equals(55 * 34) ]
#[case::equals_public_output_public_variables("public_output_equality_public_variables", "default", vec![("my_output", false)])] // [ (79 * 55 + 7).equals_public_output(55 * 34) ]
#[case::equals_public_variables("equals_public", "default", vec![("my_output", false)])] // [ (79 * 55 + 7) == (55 * 34) ]
fn test_evaluator_boolean_public_variables(
    #[case] test_id: &str,
    #[case] variables_file_id: &str,
    #[case] expected_public_variables: Vec<(&str, bool)>,
) -> Result<()> {
    let expected_outputs: HashMap<_, _> = expected_public_variables
        .into_iter()
        .map(|(name, public_variable)| (String::from(name), NadaValue::new_boolean(public_variable)))
        .collect();
    run_evaluator(test_id, variables_file_id, expected_outputs)
}

#[rstest]
#[case::input_array("input_array", "default", vec![("my_output", vec![10, - 100, 21, - 121, 84])])]
#[case::map_simple("map_simple", "default", vec![("my_output", vec![2, 3, 4])])]
#[case::map_simple_mul("map_simple_mul", "default", vec![("my_output", vec![1, 2, 3])])]
#[case::array_chaining_map_map("array_chaining_map_map", "default", vec![("my_output", vec![3, 4, 5])])]
#[ignore = "functions are broken in MIR preprocessing"]
#[case::array_product("array_product", "default", vec![("my_output", vec![2, 6, 12])])]
fn test_evaluator_array_secrets(
    #[case] test_id: &str,
    #[case] variables_file_id: &str,
    #[case] expected_secrets: Vec<(&str, Vec<i64>)>,
) -> Result<()> {
    let expected_outputs = expected_secrets
        .into_iter()
        .map(|(name, secrets)| {
            (
                String::from(name),
                NadaValue::new_array_non_empty(
                    secrets.into_iter().map(|value| NadaValue::new_secret_integer(value)).collect(),
                )
                .unwrap(),
            )
        })
        .collect();
    run_evaluator(test_id, variables_file_id, expected_outputs)
}

#[rstest]
#[case::map_simple_le("map_simple_le", "default", vec![("my_output", vec![true, false, false])])]
fn test_evaluator_array_boolean(
    #[case] test_id: &str,
    #[case] variables_file_id: &str,
    #[case] expected_secrets: Vec<(&str, Vec<bool>)>,
) -> Result<()> {
    let expected_outputs: HashMap<_, _> = expected_secrets
        .into_iter()
        .map(|(name, secrets)| {
            (
                String::from(name),
                NadaValue::new_array_non_empty(
                    secrets.into_iter().map(|value| NadaValue::new_secret_boolean(value)).collect(),
                )
                .unwrap(),
            )
        })
        .collect();
    run_evaluator(test_id, variables_file_id, expected_outputs)
}

#[rstest]
#[case::random_value("random_value_simple", "default")]
fn test_evaluator_random(#[case] test_id: &str, #[case] variables_file_id: &str) -> Result<()> {
    let f = |outputs: HashMap<String, NadaValue<Clear>>| {
        assert_eq!(outputs.len(), 1);
        Ok(())
    };
    run_evaluator_pred(test_id, variables_file_id, &f)
}

/// Array operations whose output is an integer reduction
#[rstest]
#[case::reduce_simple("reduce_simple", "default", vec![("my_output", 6)])]
#[case::reduce_simple_mul("reduce_simple_mul", "default", vec![("my_output", 6)])]
#[case::array_chaining_map_reduce("array_chaining_map_reduce", "default", vec![("my_output", 7)])] // (1 * 1) + (2 + 1) + (3 * 1) + 1
#[ignore = "functions are broken in MIR preprocessing"]
#[case::inner_product("inner_product", "default", vec![("out", 20)])]
#[ignore = "functions are broken in MIR preprocessing"]
#[case::euclidean_distance("euclidean_distance", "default", vec![("out", 3)])]
fn test_arrays_integer_reduction_operations(
    #[case] test_id: &str,
    #[case] variables_file_id: &str,
    #[case] expected_secrets: Vec<(&str, i64)>,
) -> Result<()> {
    test_evaluator_integer_secrets(test_id, variables_file_id, expected_secrets)
}

/// Tests that if the provided inputs types are incorrect the Bytecode Evaluator will return an error.
#[test]
fn test_invalid_input_types() -> Result<(), Error> {
    // def nada_main():
    //  party1 = Party(name="Party1")
    //  my_int1 = SecretUnsignedInteger(Input(name="my_uint1", party=party1))
    //  public_my_int2 = PublicUnsignedInteger(Input(name="my_uint2", party=party1))
    //  new_int = my_int1 % public_my_int2
    //  return [Output(new_int, "my_output", party1)]
    let program_mir = &PROGRAMS.mir("modulo_unsigned_secret_public").expect("program not found");
    let bytecode: ProgramBytecode = MIR2Bytecode::transform(program_mir).expect("transformation failed");
    let secrets = vec![
        ("my_uint1".to_string(), NadaValue::new_secret_integer(1)),
        ("my_uint2".to_string(), NadaValue::new_integer(1)),
    ]
    .into_iter()
    .collect();

    let outputs = Evaluator::<Prime>::run(&bytecode, secrets);
    assert!(outputs.is_err());
    Ok(())
}

#[test]
fn test_read_memory_element_array() -> Result<(), Error> {
    let mut base_dir = current_dir()?;
    if !base_dir.ends_with("bytecode-evaluator") {
        base_dir.push("nada-lang/bytecode-evaluator");
    }
    let base_dir = base_dir.to_str().unwrap();
    let program_mir = &PROGRAMS.mir("array_inner_product").expect("program not found");
    let bytecode: ProgramBytecode = MIR2Bytecode::transform(program_mir).expect("transformation failed");
    let values_file_path = format!("{base_dir}/../tests/resources/values/default.json");
    let values: HashMap<String, NadaValue<Clear>> = read_json(values_file_path)?;
    let mut evaluator: Evaluator<Prime> = Evaluator::<Prime>::default();

    evaluator.store_literals(&bytecode)?;

    evaluator.store_inputs(&bytecode, values)?;

    evaluator.simulate(&bytecode)?;
    let first_array =
        evaluator.read_memory_element(BytecodeAddress::new(0, jit_compiler::models::memory::AddressType::Heap))?;
    assert_eq!(NadaType::Array { inner_type: Box::new(NadaType::SecretInteger), size: 3 }, first_array.to_type());
    assert_eq!(
        NadaValue::new_array(
            NadaType::SecretInteger,
            vec![
                NadaValue::new_secret_integer(ModularNumber::from_u32(1)),
                NadaValue::new_secret_integer(ModularNumber::from_u32(2)),
                NadaValue::new_secret_integer(ModularNumber::from_u32(3))
            ]
        )?,
        first_array
    );
    Ok(())
}

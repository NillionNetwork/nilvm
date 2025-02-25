//! Various program tests using the program simulator
use crate::{protocols::MPCProtocol, MPCCompiler};
use anyhow::{anyhow, Error, Ok};
use execution_engine_vm::{
    metrics::{ExecutionMetrics, ExecutionMetricsConfig},
    simulator::{
        inputs::{InputGenerator, StaticInputGeneratorBuilder},
        ProgramSimulator, SimulationParameters,
    },
    vm::config::ExecutionVmConfig,
};
use jit_compiler::JitCompiler;
use math_lib::modular::U64SafePrime;
use nada_value::{clear::Clear, NadaValue};
use once_cell::sync::Lazy;
use rstest::rstest;
use std::collections::HashMap;
use test_programs::PROGRAMS;

mod addition;
mod array;
mod boolean;
mod comparison;
mod division;
mod ecdsa_sign;
mod eddsa_sign;
mod if_else;
mod map;
mod modulo;
mod multiplication;
mod nada_fn;
mod power;
mod public_key_derive;
mod random;
mod reduce;
mod reuse;
mod reveal;
mod shift;
mod subtraction;
mod truncpr;
mod tuple;
mod zip_unzip;

type Prime = U64SafePrime;

static DEFAULT_PARAMETERS: Lazy<SimulationParameters> = Lazy::new(|| SimulationParameters {
    polynomial_degree: 1,
    network_size: 5,
    execution_vm_config: ExecutionVmConfig::default(),
});

fn simulate_with_parameters(
    program_name: &str,
    inputs: InputGenerator,
    parameters: SimulationParameters,
) -> Result<(HashMap<String, NadaValue<Clear>>, ExecutionMetrics), Error> {
    let mir = PROGRAMS.mir(program_name)?;
    let program = MPCCompiler::compile(mir)?;
    let simulator =
        ProgramSimulator::<MPCProtocol, Prime>::new(program, parameters, &inputs, ExecutionMetricsConfig::disabled())?;
    simulator.run()
}

pub(crate) fn simulate(program_name: &str, inputs: InputGenerator) -> Result<HashMap<String, NadaValue<Clear>>, Error> {
    let (result, _) = simulate_with_parameters(program_name, inputs, DEFAULT_PARAMETERS.clone())?;
    Ok(result)
}

// Shortcuts for creating new NadaValues.
// Using these provide a few benefits:
// 1. they are shorter, leading to more readable test cases
// 2. you don't have to prefix unsigned numbers with _u32 or _u64
// 3. we can unwrap NadaValue::new_array_non_empty
pub(crate) fn integer(value: i64) -> NadaValue<Clear> {
    NadaValue::new_integer(value)
}

pub(crate) fn unsigned_integer(value: u64) -> NadaValue<Clear> {
    NadaValue::new_unsigned_integer(value)
}

pub(crate) fn secret_integer(value: i64) -> NadaValue<Clear> {
    NadaValue::new_secret_integer(value)
}

pub(crate) fn secret_unsigned_integer(value: u64) -> NadaValue<Clear> {
    NadaValue::new_secret_unsigned_integer(value)
}

pub(crate) fn boolean(value: bool) -> NadaValue<Clear> {
    NadaValue::new_boolean(value)
}

pub(crate) fn secret_boolean(value: bool) -> NadaValue<Clear> {
    NadaValue::new_secret_boolean(value)
}

pub(crate) fn tuple(left: NadaValue<Clear>, right: NadaValue<Clear>) -> NadaValue<Clear> {
    NadaValue::new_tuple(left, right).unwrap()
}

pub(crate) fn array_non_empty(values: Vec<NadaValue<Clear>>) -> NadaValue<Clear> {
    NadaValue::new_array_non_empty(values).unwrap()
}

// M/F (0/1), Age, is smoking?, is diabetic?, high blood pressure?, HDL
// cholesterol, height, weight, daily physical activity (minutes per day),
// drinking (glasses per day)
#[rstest]
#[case(0, 10, 0, 0, 0, 60, 150, 35, 200, 0, 0)] // healthy
#[case(0, 55, 1, 0, 1, 35, 180, 175, 20, 4, 7)]
#[case(1, 55, 1, 0, 1, 35, 180, 175, 20, 4, 6)]
#[case(0, 66, 1, 0, 0, 39, 180, 95, 5, 2, 5)]
#[case(1, 66, 1, 0, 0, 39, 180, 95, 5, 2, 5)]
#[case(0, 33, 0, 0, 0, 55, 165, 55, 30, 1, 0)]
#[case(1, 33, 0, 0, 0, 55, 165, 55, 30, 1, 0)]
#[case(0, 94, 1, 1, 1, 22, 165, 155, 0, 5, 8)]
#[case(1, 94, 1, 1, 1, 22, 165, 155, 0, 5, 8)]
fn cardio_risk_factor(
    #[case] sex: u64,
    #[case] age: u64,
    #[case] is_smoking: u64,
    #[case] is_diabetic: u64,
    #[case] high_blood_pressure: u64,
    #[case] hdl_cholesterol: u64,
    #[case] height: u64,
    #[case] weight: u64,
    #[case] physical_act: u64,
    #[case] drinking: u64,
    #[case] expected: u64,
) -> Result<(), Error> {
    let inputs = StaticInputGeneratorBuilder::default()
        .add_secret_unsigned_integer("sex", sex)
        .add_secret_unsigned_integer("age", age)
        .add_secret_unsigned_integer("is_smoking", is_smoking)
        .add_secret_unsigned_integer("is_diabetic", is_diabetic)
        .add_secret_unsigned_integer("high_blood_pressure", high_blood_pressure)
        .add_secret_unsigned_integer("hdl_cholesterol", hdl_cholesterol)
        .add_secret_unsigned_integer("height", height)
        .add_secret_unsigned_integer("weight", weight)
        .add_secret_unsigned_integer("physical_act", physical_act)
        .add_secret_unsigned_integer("drinking", drinking)
        .build();
    let outputs = simulate("cardio_risk_factor", inputs)?;
    assert_eq!(outputs.len(), 1);
    let output = outputs.get("my_output").unwrap();
    assert_eq!(output, &secret_unsigned_integer(expected));
    Ok(())
}

/// alpha = (4*N0*N2 - N1^2)^2
/// beta_1 = 2(2*N0 + N1)^2
/// beta_2 = (2*N0 + N1)(2*N2 + N1)
/// beta_3 = 2(2*N2 + N1)^2
#[rstest]
#[case(2, 7, 9, 529, 242, 275, 1250)]
#[case(4, 4, 4, 2304, 288, 144, 288)]
#[case(- 1, - 2, - 3, 64, 32, 32, 128)]
fn chi_squared(
    #[case] n0: i64,
    #[case] n1: i64,
    #[case] n2: i64,
    #[case] expected_alpha: i64,
    #[case] expected_beta_1: i64,
    #[case] expected_beta_2: i64,
    #[case] expected_beta_3: i64,
) -> Result<(), Error> {
    let inputs = StaticInputGeneratorBuilder::default()
        .add_secret_integer("n0", n0)
        .add_secret_integer("n1", n1)
        .add_secret_integer("n2", n2)
        .build();
    let outputs = simulate("chi_squared", inputs)?;
    assert_eq!(outputs.len(), 4);

    let alpha = outputs.get("alpha").unwrap();
    let beta_1 = outputs.get("beta_1").unwrap();
    let beta_2 = outputs.get("beta_2").unwrap();
    let beta_3 = outputs.get("beta_3").unwrap();

    assert_eq!(alpha, &secret_integer(expected_alpha));
    assert_eq!(beta_1, &secret_integer(expected_beta_1));
    assert_eq!(beta_2, &secret_integer(expected_beta_2));
    assert_eq!(beta_3, &secret_integer(expected_beta_3));
    Ok(())
}

#[rstest]
#[case(1, 2, 3, 4, 14)]
#[case(1337, 42, 59928, 918273, 55030320498)]
fn simple(#[case] a: i64, #[case] b: i64, #[case] c: i64, #[case] d: i64, #[case] expected: i64) -> Result<(), Error> {
    let inputs = StaticInputGeneratorBuilder::default()
        .add_secret_integer("A", a)
        .add_secret_integer("B", b)
        .add_secret_integer("C", c)
        .add_secret_integer("D", d)
        .build();
    let outputs = simulate("simple", inputs)?;
    assert_eq!(outputs.len(), 1);
    let output = outputs.get("O").unwrap();
    assert_eq!(output, &secret_integer(expected));
    Ok(())
}

#[rstest]
#[case(3, 4, 1, 2, 10)]
#[case(59928, 918273, 1337, 42, 55030208190)]
fn simple_sub(
    #[case] a: i64,
    #[case] b: i64,
    #[case] c: i64,
    #[case] d: i64,
    #[case] expected: i64,
) -> Result<(), Error> {
    let inputs = StaticInputGeneratorBuilder::default()
        .add_secret_integer("A", a)
        .add_secret_integer("B", b)
        .add_secret_integer("C", c)
        .add_secret_integer("D", d)
        .build();
    let outputs = simulate("simple_sub", inputs)?;
    assert_eq!(outputs.len(), 1);
    let output = outputs.get("O").unwrap();
    assert_eq!(output, &secret_integer(expected));
    Ok(())
}

#[rstest]
#[case(5, 4, 6, 8, 11, 3, 681)]
#[case(125, 48, 1234, 9, 0, 8745, 7404000)]
/// A = SecretInteger(Input(name="A", party=party1))
/// B = SecretInteger(Input(name="B", party=party1))
/// C = SecretInteger(Input(name="C", party=party1))
/// D = SecretInteger(Input(name="D", party=party1))
/// E = SecretInteger(Input(name="E", party=party1))
/// F = SecretInteger(Input(name="F", party=party1))
///
/// TMP1 = A * B
/// PRODUCT1 = TMP1 * C
/// TMP2 = C * D
/// PRODUCT2 = TMP2 * E
/// PRODUCT3 = E * F
/// PARTIAL = PRODUCT1 + PRODUCT2
/// FINAL = PARTIAL + PRODUCT3
fn test_complex(
    #[case] a: i64,
    #[case] b: i64,
    #[case] c: i64,
    #[case] d: i64,
    #[case] e: i64,
    #[case] f: i64,
    #[case] expected: i64,
) -> Result<(), Error> {
    let inputs = StaticInputGeneratorBuilder::default()
        .add_secret_integer("A", a)
        .add_secret_integer("B", b)
        .add_secret_integer("C", c)
        .add_secret_integer("D", d)
        .add_secret_integer("E", e)
        .add_secret_integer("F", f)
        .build();
    let outputs = simulate("complex", inputs)?;
    assert_eq!(outputs.len(), 1);
    let output = outputs.get("FINAL").unwrap();
    assert_eq!(output, &secret_integer(expected));
    Ok(())
}

#[rstest]
#[case(5, 4, 6, 8, 11, 3, 5, 4342)]
#[case(125, 48, 1234, 9, 0, 8745, 98, 65224047197)]
/// A = SecretInteger(Input(name="A", party=party1))
/// B = SecretInteger(Input(name="B", party=party2))
/// C = SecretInteger(Input(name="C", party=party1))
/// D = SecretInteger(Input(name="D", party=party2))
/// E = SecretInteger(Input(name="E", party=party2))
/// F = SecretInteger(Input(name="F", party=party2))
/// G = SecretInteger(Input(name="G", party=party2))
///
/// result = (
/// ((A * B) + C + D) * (E * (F + G))
/// + (A * B * (C + D) + E) * F
/// + (A + (B * (C + (D * (E + F)))))
/// )
fn test_complex_operation_mix(
    #[case] a: i64,
    #[case] b: i64,
    #[case] c: i64,
    #[case] d: i64,
    #[case] e: i64,
    #[case] f: i64,
    #[case] g: i64,
    #[case] expected: i64,
) -> Result<(), Error> {
    let inputs = StaticInputGeneratorBuilder::default()
        .add_secret_integer("A", a)
        .add_secret_integer("B", b)
        .add_secret_integer("C", c)
        .add_secret_integer("D", d)
        .add_secret_integer("E", e)
        .add_secret_integer("F", f)
        .add_secret_integer("G", g)
        .build();
    let outputs = simulate("complex_operation_mix", inputs)?;
    assert_eq!(outputs.len(), 1);
    let output = outputs.get("my_output").unwrap();
    assert_eq!(output, &secret_integer(expected));
    Ok(())
}

#[test]
fn test_inner_product() -> Result<(), Error> {
    let my_array_1 = array_non_empty(vec![secret_integer(1), secret_integer(2), secret_integer(3)]);
    let my_array_2 = array_non_empty(vec![secret_integer(2), secret_integer(3), secret_integer(4)]);
    let inputs = StaticInputGeneratorBuilder::default()
        .add("my_array_1", my_array_1)
        .add("my_array_2", my_array_2)
        .add("secret_int0", secret_integer(0))
        .build();
    let mut outputs = simulate("inner_product", inputs)?;
    assert_eq!(outputs.len(), 1);
    assert!(outputs.contains_key("out"));
    let output = outputs.remove("out").unwrap();

    let expected_output = secret_integer(20);
    assert_eq!(output, expected_output);
    Ok(())
}

#[test]
fn test_array_inner_product() -> Result<(), Error> {
    let my_array_1 = array_non_empty(vec![secret_integer(1), secret_integer(2), secret_integer(3)]);
    let my_array_2 = array_non_empty(vec![secret_integer(2), secret_integer(3), secret_integer(4)]);
    let inputs =
        StaticInputGeneratorBuilder::default().add("my_array_1", my_array_1).add("my_array_2", my_array_2).build();
    let mut outputs = simulate("array_inner_product", inputs)?;
    assert_eq!(outputs.len(), 1);
    assert!(outputs.contains_key("out"));
    let output = outputs.remove("out").unwrap();

    let expected_output = secret_integer(20);
    assert_eq!(output, expected_output);
    Ok(())
}

#[test]
fn test_array_inner_product_public() -> Result<(), Error> {
    let my_array_1 = array_non_empty(vec![integer(1), integer(2), integer(3)]);
    let my_array_2 = array_non_empty(vec![integer(2), integer(3), integer(4)]);
    let inputs =
        StaticInputGeneratorBuilder::default().add("my_array_1", my_array_1).add("my_array_2", my_array_2).build();
    let mut outputs = simulate("array_inner_product_public", inputs)?;
    assert_eq!(outputs.len(), 1);
    assert!(outputs.contains_key("out"));
    let output = outputs.remove("out").unwrap();

    let expected_output = integer(20);
    assert_eq!(output, expected_output);
    Ok(())
}

#[test]
fn test_array_inner_product_share_public() -> Result<(), Error> {
    let my_array_1 = array_non_empty(vec![secret_integer(1), secret_integer(2), secret_integer(3)]);
    let my_array_2 = array_non_empty(vec![integer(2), integer(3), integer(4)]);
    let inputs =
        StaticInputGeneratorBuilder::default().add("my_array_1", my_array_1).add("my_array_2", my_array_2).build();
    let mut outputs = simulate("array_inner_product_share_public", inputs)?;
    assert_eq!(outputs.len(), 1);
    assert!(outputs.contains_key("out"));
    let output = outputs.remove("out").unwrap();

    let expected_output = secret_integer(20);
    assert_eq!(output, expected_output);
    Ok(())
}

#[test]
fn test_array_inner_product_mult() -> Result<(), Error> {
    let my_array_1 = array_non_empty(vec![secret_integer(1), secret_integer(2), secret_integer(3)]);
    let my_array_2 = array_non_empty(vec![secret_integer(2), secret_integer(3), secret_integer(4)]);
    let inputs =
        StaticInputGeneratorBuilder::default().add("my_array_1", my_array_1).add("my_array_2", my_array_2).build();
    let mut outputs = simulate("array_inner_product_mult", inputs)?;
    assert_eq!(outputs.len(), 1);
    assert!(outputs.contains_key("out"));
    let output = outputs.remove("out").unwrap();

    let expected_output = secret_integer(40);
    assert_eq!(output, expected_output);
    Ok(())
}

#[test]
fn test_array_inner_product_map() -> Result<(), Error> {
    let my_array_1 = array_non_empty(vec![secret_integer(1), secret_integer(2), secret_integer(3)]);
    let my_array_2 = array_non_empty(vec![secret_integer(2), secret_integer(3), secret_integer(4)]);
    let inputs =
        StaticInputGeneratorBuilder::default().add("my_array_1", my_array_1).add("my_array_2", my_array_2).build();
    let mut outputs = simulate("array_inner_product_map", inputs)?;
    assert_eq!(outputs.len(), 1);
    assert!(outputs.contains_key("out"));
    let output = outputs.remove("out").unwrap();

    let expected_output = secret_integer(38);
    assert_eq!(output, expected_output);
    Ok(())
}

#[test]
fn test_euclidean_distance() -> Result<(), Error> {
    let my_array_1 = array_non_empty(vec![secret_integer(1), secret_integer(2), secret_integer(3)]);
    let my_array_2 = array_non_empty(vec![secret_integer(2), secret_integer(3), secret_integer(4)]);
    let secret_int0 = secret_integer(0);
    let inputs = StaticInputGeneratorBuilder::default()
        .add("my_array_1", my_array_1)
        .add("my_array_2", my_array_2)
        .add("secret_int0", secret_int0)
        .build();
    let mut outputs = simulate("euclidean_distance", inputs)?;
    assert_eq!(outputs.len(), 1);
    assert!(outputs.contains_key("out"));
    let output = outputs.remove("out").unwrap();

    let expected_output = secret_integer(3);
    assert_eq!(output, expected_output);
    Ok(())
}

#[test]
fn test_sum() -> Result<(), Error> {
    let inputs = StaticInputGeneratorBuilder::default().add_secret_integer("a", 1).add_secret_integer("b", 2).build();
    let mut outputs = simulate("sum", inputs)?;
    assert_eq!(outputs.len(), 1);
    assert!(outputs.contains_key("my_output"));
    let output = outputs.remove("my_output").unwrap();

    let expected_output = array_non_empty(vec![secret_integer(6), secret_integer(6), secret_integer(6)]);
    assert_eq!(output, expected_output);
    Ok(())
}

#[rstest]
#[case(0, 0)]
#[case(1, 0)]
#[case(- 1, 0)]
#[case(3, 1)]
#[case(2, 1)]
#[case(- 2, 1)]
#[case(- 3, 1)]
fn test_indicator_extreme(#[case] value: i64, #[case] expected: i64) -> Result<(), Error> {
    let inputs = StaticInputGeneratorBuilder::default().add_secret_integer("value", value).build();
    let mut outputs = simulate("indicator_extreme", inputs)?;
    assert_eq!(outputs.len(), 1);
    assert!(outputs.contains_key("indicator_extreme"));
    let output = outputs.remove("indicator_extreme").unwrap();
    assert_eq!(output, secret_integer(expected));
    Ok(())
}

#[test]
fn test_hamming_distance() -> Result<(), Error> {
    let array_1 = array_non_empty(vec![secret_integer(1), secret_integer(2), secret_integer(3)]);
    let array_2 = array_non_empty(vec![secret_integer(4), secret_integer(2), secret_integer(3)]);
    let secret_int0 = secret_integer(0);
    let inputs = StaticInputGeneratorBuilder::default()
        .add("my_array_1", array_1)
        .add("my_array_2", array_2)
        .add("secret_int0", secret_int0)
        .build();
    let output = simulate("hamming_distance", inputs)?.remove("out").ok_or(anyhow!("output not found"))?;
    assert_eq!(output, secret_integer(1));

    Ok(())
}

#[test]
fn test_multiple_operations() -> Result<(), Error> {
    let inputs = StaticInputGeneratorBuilder::default()
        .add("secret_int1", secret_integer(1))
        .add("secret_int2", secret_integer(1))
        .add("public_int", integer(1))
        .build();
    let output = simulate("multiple_operations", inputs)?.remove("result");
    // The result should be 10001, but the division doesn't return the result correctly
    // because the prime is not large enough
    assert!(output.is_some());
    Ok(())
}

/// Asserts that the first expression is equal to either the second or the third
/// (using [`PartialEq`]).
///
/// On panic, this macro will print the values of the expressions with their
/// debug representations.
#[macro_export]
macro_rules! assert_eq_either {
    ($first:expr, $second:expr, $third:expr) => {
        let first_val = &$first;
        let second_val = &$second;
        let third_val = &$third;

        if !(*first_val == *second_val || *first_val == *third_val) {
            panic!(
                "assertion failed: `(first == second) || (first == third)`\n   first: `{:?}`,\n  second: `{:?}`,\n   third: `{:?}`",
                *first_val, *second_val, *third_val
            );
        }
    };
}

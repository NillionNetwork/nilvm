use crate::vm::tests::{array_non_empty, secret_integer, simulate};
use anyhow::{Error, Ok};
use execution_engine_vm::simulator::inputs::StaticInputGeneratorBuilder;
use nada_value::{clear::Clear, NadaValue};
use rstest::rstest;

#[rstest]
#[case("reduce_simple", vec![("my_array_1", array_non_empty(vec![
        secret_integer(1),
        secret_integer(2),
        secret_integer(3),
    ])), ("secret_int0", secret_integer(0))],
    secret_integer(6))]
#[case("reduce_simple_mul", vec![("my_array_1", array_non_empty(vec![
        secret_integer(1),
        secret_integer(2),
        secret_integer(3),
    ])), ("my_int", secret_integer(3))],
    secret_integer(18))]
#[case("reduce_array_sum", vec![("my_array", array_non_empty(vec![
        secret_integer(10),
        secret_integer(20),
        secret_integer(30),
        secret_integer(40),
    ])), ("secret_int0", secret_integer(0))],
    secret_integer(100))]
#[case("reduce_array_max", vec![("my_array", array_non_empty(vec![
        secret_integer(10),
        secret_integer(42),
        secret_integer(40),
        secret_integer(30),
    ])), ("secret_int0", secret_integer(0))],
    secret_integer(42))]
#[case("reduce_array_min", vec![("my_array", array_non_empty(vec![
        secret_integer(10),
        secret_integer(42),
        secret_integer(40),
        secret_integer(30),
    ])), ("secret_int100000", secret_integer(100000))],
    secret_integer(10))]
fn reduce_tests(
    #[case] program_name: &str,
    #[case] inputs: Vec<(&str, NadaValue<Clear>)>,
    #[case] expected: NadaValue<Clear>,
) -> Result<(), Error> {
    println!("Program: {:?}", program_name);

    let inputs = StaticInputGeneratorBuilder::default().add_all(inputs).build();
    let outputs = simulate(program_name, inputs)?;
    assert_eq!(outputs.len(), 1);
    let output = outputs.get("my_output").unwrap();
    assert_eq!(output, &expected);

    Ok(())
}

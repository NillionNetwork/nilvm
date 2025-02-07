use crate::vm::tests::{array_non_empty, secret_boolean, secret_integer, secret_unsigned_integer, simulate};
use anyhow::{Error, Ok};
use execution_engine_vm::simulator::inputs::StaticInputGeneratorBuilder;
use nada_value::{clear::Clear, NadaValue};
use rstest::rstest;

#[rstest]
#[case("map_max", vec![("my_array_1", array_non_empty(vec![
        secret_integer(1),
        secret_integer(2),
        secret_integer(18),
    ])), ("my_int", secret_integer(15))],
    array_non_empty(vec![
        secret_integer(15),
        secret_integer(15),
        secret_integer(18)
    ]))]
#[case("map_simple", vec![("my_array_1", array_non_empty(vec![
        secret_integer(1),
        secret_integer(2),
        secret_integer(3),
    ])), ("my_int", secret_integer(1))],
    array_non_empty(vec![
        secret_integer(2),
        secret_integer(3),
        secret_integer(4)
    ]))]
#[case("map_simple_unsigned", vec![("my_array_1", array_non_empty(vec![
        secret_unsigned_integer(1),
        secret_unsigned_integer(2),
        secret_unsigned_integer(3),
    ])), ("my_int", secret_unsigned_integer(1))],
    array_non_empty(vec![
        secret_unsigned_integer(2),
        secret_unsigned_integer(3),
        secret_unsigned_integer(4)
    ]))]
#[case("map_simple_mul", vec![("my_array_1", array_non_empty(vec![
        secret_integer(1),
        secret_integer(2),
        secret_integer(3),
    ])), ("my_int", secret_integer(3))],
    array_non_empty(vec![
        secret_integer(3),
        secret_integer(6),
        secret_integer(9)
    ]))]
#[case("map_simple_div", vec![("my_array_1", array_non_empty(vec![
        secret_integer(2),
        secret_integer(4),
        secret_integer(6),
    ])), ("my_int", secret_integer(2))],
    array_non_empty(vec![
        secret_integer(1),
        secret_integer(2),
        secret_integer(3)
    ]))]
#[case("map_simple_le", vec![("my_array_1", array_non_empty(vec![
        secret_integer(1),
        secret_integer(2),
        secret_integer(3),
    ])), ("my_int", secret_integer(1))],
    array_non_empty(vec![
        secret_boolean(true),
        secret_boolean(false),
        secret_boolean(false)
    ]))]
fn map_tests(
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

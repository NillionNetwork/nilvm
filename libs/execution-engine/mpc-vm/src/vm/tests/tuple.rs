use crate::vm::tests::{array_non_empty, integer, secret_integer, secret_unsigned_integer, simulate, tuple};
use anyhow::{Error, Ok};
use execution_engine_vm::simulator::inputs::StaticInputGeneratorBuilder;
use nada_value::{clear::Clear, NadaValue};
use rstest::rstest;

#[rstest]
#[case("tuple_new", vec![("a", secret_integer(1)), ("b", secret_integer(2))], tuple(secret_integer(1), secret_integer(2)))]
#[case("tuple_new_public", vec![("a", integer(1)), ("b", integer(2))], tuple(integer(1), integer(2)))]
#[case("tuple_new_after_operation", vec![("a", secret_integer(1)), ("b", secret_integer(2)), ("c", secret_integer(4))], tuple(secret_integer(4), secret_integer(3)))]
#[case("tuple_new_complex", vec![("a", secret_integer(1)), ("b", secret_integer(2)), ("c", secret_integer(3)), ("d", secret_integer(4))], tuple(secret_integer(3), tuple(secret_integer(3), secret_integer(4))))]
#[case("tuple_new_unzip", vec![("a", secret_integer(1)), ("b", secret_integer(2)), ("c", secret_integer(3)), ("d", secret_integer(4))], tuple(array_non_empty(vec![secret_integer(1), secret_integer(3)]), array_non_empty(vec![secret_integer(2), secret_integer(4)])))]
#[case("tuple_new_map_secret_secret", vec![("a", secret_integer(1)), ("b", secret_integer(2)), ("c", secret_integer(3)), ("d", secret_integer(4))], array_non_empty(vec![secret_integer(3), secret_integer(7)]))]
#[case("tuple_new_map_public_public", vec![("a", integer(1)), ("b", integer(2)), ("c", integer(3)), ("d", integer(4))], array_non_empty(vec![integer(3), integer(7)]))]
#[case("tuple_new_map_public_secret", vec![("a", integer(1)), ("b", secret_integer(2)), ("c", integer(3)), ("d", secret_integer(4))], array_non_empty(vec![secret_integer(3), secret_integer(7)]))]
#[case("tuple_new_if_else", vec![("a", secret_integer(1)), ("b", secret_integer(2)), ("c", secret_unsigned_integer(3)), ("d", secret_unsigned_integer(4))], tuple(secret_unsigned_integer(42), integer(20)))]
fn tuple_tests(
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

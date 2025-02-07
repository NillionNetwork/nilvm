use anyhow::{Error, Ok};
use rstest::rstest;

use nada_value::{clear::Clear, NadaValue};

use crate::vm::tests::{boolean, integer, secret_boolean, secret_integer, simulate};
use execution_engine_vm::simulator::inputs::StaticInputGeneratorBuilder;

#[rstest]
// 5 <= 6
#[case("less_than", vec![("A", secret_integer(1)), ("B", secret_integer(2)), ("C", secret_integer(3)), ("D", secret_integer(3))], secret_boolean(true))]
// 5 <= 5
#[case("less_than", vec![("A", secret_integer(1)), ("B", secret_integer(2)), ("C", secret_integer(4)), ("D", secret_integer(3))], secret_boolean(false))]
#[case("less_than", vec![("A", secret_integer(1337)), ("B", secret_integer(42)), ("C", secret_integer(59928)), ("D", secret_integer(918273))], secret_boolean(true))]
#[case("less_than", vec![("A", secret_integer(1337)), ("B", secret_integer(42)), ("C", secret_integer(59928)), ("D", secret_integer(100))], secret_boolean(false))]
#[case("less_or_equal_than", vec![("A", secret_integer(1)), ("B", secret_integer(2)), ("C", secret_integer(3)), ("D", secret_integer(3))], secret_boolean(true))]
#[case("less_or_equal_than", vec![("A", secret_integer(1)), ("B", secret_integer(2)), ("C", secret_integer(4)), ("D", secret_integer(3))], secret_boolean(true))]
#[case("less_or_equal_than", vec![("A", secret_integer(1337)), ("B", secret_integer(42)), ("C", secret_integer(59928)), ("D", secret_integer(918273))], secret_boolean(true))]
#[case("less_or_equal_than", vec![("A", secret_integer(1337)), ("B", secret_integer(42)), ("C", secret_integer(59928)), ("D", secret_integer(100))], secret_boolean(false))]
#[case("less_or_equal_than_literals", vec![], boolean(true))]
// 5 >= 6
#[case("greater_or_equal_than", vec![("A", secret_integer(1)), ("B", secret_integer(2)), ("C", secret_integer(3)), ("D", secret_integer(3))], secret_boolean(false))]
// 5 >= 5
#[case("greater_or_equal_than", vec![("A", secret_integer(1)), ("B", secret_integer(2)), ("C", secret_integer(4)), ("D", secret_integer(3))], secret_boolean(true))]
#[case("greater_or_equal_than", vec![("A", secret_integer(1337)), ("B", secret_integer(42)), ("C", secret_integer(59928)), ("D", secret_integer(918273))], secret_boolean(false))]
#[case("greater_or_equal_than", vec![("A", secret_integer(1337)), ("B", secret_integer(42)), ("C", secret_integer(59928)), ("D", secret_integer(100))], secret_boolean(true))]
// 5 >= 6
#[case("greater_or_equal_than_public_variables", vec![("public_A", integer(1)), ("public_B", integer(2)), ("public_C", integer(3)), ("public_D", integer(3))], boolean(false))]
// 5 >= 5
#[case("greater_or_equal_than_public_variables", vec![("public_A", integer(1)), ("public_B", integer(2)), ("public_C", integer(4)), ("public_D", integer(-3))], boolean(true))]
// 81 >= 32*-42
#[case("greater_equal_mul", vec![("my_int1", secret_integer(32)), ("public_my_int2", integer(81))], secret_boolean(true))]
// (1 * 2 + 3).public_equals(2 * 3)
// (5).public_equals(6)
#[case("public_output_equality", vec![("A", secret_integer(1)), ("B", secret_integer(2)), ("C", secret_integer(3)), ("D", secret_integer(3))], boolean(false))]
// (1 * 2 + 4).public_equals(2 * 3)
// (6).public_equals(6)
#[case("public_output_equality", vec![("A", secret_integer(1)), ("B", secret_integer(2)), ("C", secret_integer(4)), ("D", secret_integer(3))], boolean(true))]
fn comparison_tests(
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

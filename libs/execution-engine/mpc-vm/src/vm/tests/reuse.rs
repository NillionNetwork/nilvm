use crate::vm::tests::{integer, secret_integer, simulate};
use anyhow::{Error, Ok};
use execution_engine_vm::simulator::inputs::StaticInputGeneratorBuilder;
use nada_value::{clear::Clear, NadaValue};
use rstest::rstest;

#[rstest]
#[case("reuse_public_variables", vec![("b", integer(4))], integer(8))]
#[case("reuse_literals", vec![("b", integer(4))], integer(4))]
/// (a * b) + (a * b)
#[case("reuse_simple_1", vec![("A", secret_integer(5)), ("B", secret_integer(3))], secret_integer(30))]
#[case("reuse_simple_1", vec![("A", secret_integer(59928)), ("B", secret_integer(18273))], secret_integer(2190128688))]
/// (a * b) - (a * b)
#[case("reuse_simple_sub", vec![("A", secret_integer(5)), ("B", secret_integer(3))], secret_integer(16))]
#[case("reuse_simple_sub", vec![("A", secret_integer(59928)), ("B", secret_integer(18273))], secret_integer(3257462655))]
fn reuse_tests(
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

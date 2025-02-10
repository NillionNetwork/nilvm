use crate::vm::tests::{integer, secret_integer, secret_unsigned_integer, simulate, unsigned_integer};
use anyhow::{Error, Ok};
use execution_engine_vm::simulator::inputs::StaticInputGeneratorBuilder;
use nada_value::{clear::Clear, NadaValue};
use rstest::rstest;

#[rstest]
#[case("multiplication_simple", vec![("my_int1", secret_integer(4)), ("my_int2", secret_integer(5))], secret_integer(20))]
#[case("multiplication_simple_secret_literal", vec![("my_int1", secret_integer(4))], secret_integer(52))]
#[case("multiplication_simple_public_public", vec![("my_int1", integer(4)), ("my_int2", integer(5))], integer(20))]
#[case("multiplication_simple_public_literal", vec![("my_int1", integer(4))], integer(52))]
#[case("multiplication_simple_literal_secret", vec![("my_int1", secret_integer(4))], secret_integer(52))]
#[case("multiplication_simple_literal_public", vec![("my_int1", integer(4))], integer(52))]
#[case("multiplication_simple_literal_literal", vec![], integer(169))]
/// (a + b) * 1
#[case("multiplication_addition", vec![("A", secret_integer(64)), ("B", secret_integer(24))], secret_integer(88))]
/// 1 * (a - b)
#[case("multiplication_subtraction", vec![("A", secret_integer(64)), ("B", secret_integer(24))], secret_integer(40))]
/// a * (b / Integer(2))
#[case("multiplication_division", vec![("A", secret_integer(64)), ("B", secret_integer(24))], secret_integer(768))]
/// (a + b) * (b / UnsignedInteger(2)) * (c ** d) * (a % UnsignedInteger(5)) - a & b are secrets
#[case("multiplication_mix_operations", vec![("A", secret_unsigned_integer(64)), ("B", secret_unsigned_integer(24)), ("C", unsigned_integer(12)), ("D", unsigned_integer(2))], secret_unsigned_integer(608256))]
/// (a + b) * (b / UnsignedInteger(2)) * (c ** d) * (a % UnsignedInteger(5)) - a & b are publics
#[case("multiplication_mix_operations_public", vec![("A", unsigned_integer(64)), ("B", unsigned_integer(24)), ("C", unsigned_integer(12)), ("D", unsigned_integer(2))], unsigned_integer(608256))]
#[case("multiplication_modulo", vec![("A", secret_unsigned_integer(64)), ("B", secret_unsigned_integer(24))], secret_unsigned_integer(256))]
/// a * (b % UnsignedInteger(5))
#[case("multiplication_power", vec![("A", secret_unsigned_integer(64)), ("B", unsigned_integer(5)), ("C", unsigned_integer(2))], secret_unsigned_integer(1600))]
fn multiplication_tests(
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

use crate::vm::tests::{integer, secret_integer, secret_unsigned_integer, simulate, unsigned_integer};
use anyhow::{Error, Ok};
use execution_engine_vm::simulator::inputs::StaticInputGeneratorBuilder;
use nada_value::{clear::Clear, NadaValue};
use rstest::rstest;

#[rstest]
#[case("addition_simple", vec![("my_int1", secret_integer(4)), ("my_int2", secret_integer(5))], secret_integer(9))]
#[case("addition_simple_secret_public", vec![("my_int1", secret_integer(4)), ("public_my_int2", integer(5))], secret_integer(9))]
#[case("addition_simple_public_secret", vec![("public_my_int1", integer(4)), ("my_int2", secret_integer(5))], secret_integer(9))]
#[case("addition_simple_secret_literal", vec![("my_int1", secret_integer(4))], secret_integer(17))]
#[case("addition_simple_literal_secret", vec![("my_int1", secret_integer(4))], secret_integer(17))]
#[case("addition_simple_public_public", vec![("public_my_int1", integer(4)), ("public_my_int2", integer(5))], integer(9))]
#[case("addition_simple_public_literal", vec![("public_my_int1", integer(4))], integer(17))]
#[case("addition_simple_literal_public", vec![("public_my_int1", integer(4))], integer(17))]
#[case("addition_simple_literal_literal", vec![], integer(26))]
#[case("addition_literal_literal_neg", vec![], integer(-26))]
/// a + (b / 2)
#[case("addition_division", vec![("A", secret_integer(64)), ("B", secret_integer(24))], secret_integer(76))]
/// a + (b / 2)
#[case("add_literal_integer_truediv_public_integer_secret_integer", vec![("A", integer(-42)), ("B", secret_integer(-42))], secret_integer(-41))]
/// -42 * (-4 / c)
#[case("addition_division_public", vec![("C", integer(2))], integer(-43))]
/// a * b + (b / UnsignedInteger(2)) + (c ** d) + (a % UnsignedInteger(5)) - a & b are secrets
#[case("addition_mix_operations", vec![("A", secret_unsigned_integer(64)), ("B", secret_unsigned_integer(24)), ("C", unsigned_integer(12)), ("D", unsigned_integer(2))], secret_unsigned_integer(1696))]
/// a * b + (b / UnsignedInteger(2)) + (c ** d) + (a % UnsignedInteger(5)) - a & b are publics
#[case("addition_mix_operations_public", vec![("A", unsigned_integer(64)), ("B", unsigned_integer(24)), ("C", unsigned_integer(12)), ("D", unsigned_integer(2))], unsigned_integer(1696))]
/// a + (b % UnsignedInteger(5))
#[case("addition_modulo", vec![("A", secret_unsigned_integer(64)), ("B", secret_unsigned_integer(24))], secret_unsigned_integer(68))]
/// a + (b ** c)
#[case("addition_power", vec![("A", secret_unsigned_integer(64)), ("B", unsigned_integer(5)), ("C", unsigned_integer(2))], secret_unsigned_integer(89))]
fn addition_tests(
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

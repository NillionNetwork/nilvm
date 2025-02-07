use crate::vm::tests::{integer, secret_integer, secret_unsigned_integer, simulate, unsigned_integer};
use anyhow::{Error, Ok};
use execution_engine_vm::simulator::inputs::StaticInputGeneratorBuilder;
use nada_value::{clear::Clear, NadaValue};
use rstest::rstest;

#[rstest]
#[case("division_public", vec![("my_int1", integer(12)), ("my_int2", integer(42))], integer(0))]
#[case("division_public", vec![("my_int1", integer(-542)), ("my_int2", integer(34))], integer(-16))]
#[case("division_public", vec![("my_int1", integer(782)), ("my_int2", integer(-65))], integer(-13))]
#[case("division_unsigned_public", vec![("my_int1", unsigned_integer(12)), ("my_int2", unsigned_integer(42))], unsigned_integer(0))]
#[case("division_unsigned_public", vec![("my_int1", unsigned_integer(542)), ("my_int2", unsigned_integer(34))], unsigned_integer(15))]
#[case("division_unsigned_public", vec![("my_int1", unsigned_integer(782)), ("my_int2", unsigned_integer(65))], unsigned_integer(12))]
#[case("division_public_divisor", vec![("my_int2", secret_integer(4)), ("my_int1", integer(2))], secret_integer(2))]
#[case("division_public_divisor", vec![("my_int2", secret_integer(2)), ("my_int1", integer(1))], secret_integer(2))]
#[case("division_public_divisor", vec![("my_int2", secret_integer(36)), ("my_int1", integer(4))], secret_integer(9))]
#[case("division_public_divisor", vec![("my_int2", secret_integer(31)), ("my_int1", integer(5))], secret_integer(6))]
#[case("division_public_divisor", vec![("my_int2", secret_integer(27)), ("my_int1", integer(4))], secret_integer(6))]
#[case("division_public_divisor", vec![("my_int2", secret_integer(25)), ("my_int1", integer(5))], secret_integer(5))]
#[case("division_public_divisor", vec![("my_int2", secret_integer(19)), ("my_int1", integer(3))], secret_integer(6))]
#[case("division_public_divisor", vec![("my_int2", secret_integer(75)), ("my_int1", integer(5))], secret_integer(15))]
#[case("division_public_divisor", vec![("my_int2", secret_integer(4)), ("my_int1", integer(-2))], secret_integer(-2))]
#[case("division_public_divisor", vec![("my_int2", secret_integer(2)), ("my_int1", integer(-1))], secret_integer(-2))]
#[case("division_public_divisor", vec![("my_int2", secret_integer(-27)), ("my_int1", integer(-4))], secret_integer(6))]
#[case("division_public_divisor", vec![("my_int2", secret_integer(-25)), ("my_int1", integer(5))], secret_integer(-5))]
#[case("division_public_divisor", vec![("my_int2", secret_integer(19)), ("my_int1", integer(-3))], secret_integer(-7))]
#[case("division_public_divisor", vec![("my_int2", secret_integer(-75)), ("my_int1", integer(-5))], secret_integer(15))]
#[case("division_secret_secret", vec![("my_int1", secret_integer(12)), ("my_int2", secret_integer(0))], secret_integer(0))]
#[case("division_secret_secret", vec![("my_int1", secret_integer(12)), ("my_int2", secret_integer(42))], secret_integer(0))]
#[case("division_secret_secret", vec![("my_int1", secret_integer(31)), ("my_int2", secret_integer(5))], secret_integer(6))]
#[case("division_secret_secret", vec![("my_int1", secret_integer(-27)), ("my_int2", secret_integer(-4))], secret_integer(6))]
#[case("division_secret_secret", vec![("my_int1", secret_integer(-25)), ("my_int2", secret_integer(5))], secret_integer(-5))]
#[case("division_secret_secret", vec![("my_int1", secret_integer(19)), ("my_int2", secret_integer(-3))], secret_integer(-7))]
#[case("division_secret_secret", vec![("my_int1", secret_integer(-75)), ("my_int2", secret_integer(-5))], secret_integer(15))]
#[case("division_secret_secret_unsigned", vec![("my_int1", secret_unsigned_integer(12)), ("my_int2", secret_unsigned_integer(0))], secret_unsigned_integer(0))]
#[case("division_secret_secret_unsigned", vec![("my_int1", secret_unsigned_integer(12)), ("my_int2", secret_unsigned_integer(42))], secret_unsigned_integer(0))]
#[case("division_secret_secret_unsigned", vec![("my_int1", secret_unsigned_integer(31)), ("my_int2", secret_unsigned_integer(5))], secret_unsigned_integer(6))]
#[case("division_public_secret", vec![("my_int2", secret_integer(0)), ("my_int1", integer(12))], secret_integer(0))]
#[case("division_public_secret", vec![("my_int2", secret_integer(42)), ("my_int1", integer(12))], secret_integer(0))]
#[case("division_public_secret", vec![("my_int2", secret_integer(5)), ("my_int1", integer(31))], secret_integer(6))]
#[case("division_public_secret", vec![("my_int2", secret_integer(-4)), ("my_int1", integer(-27))], secret_integer(6))]
#[case("division_public_secret", vec![("my_int2", secret_integer(5)), ("my_int1", integer(-25))], secret_integer(-5))]
#[case("division_public_secret", vec![("my_int2", secret_integer(-3)), ("my_int1", integer(19))], secret_integer(-7))]
#[case("division_public_secret", vec![("my_int2", secret_integer(-5)), ("my_int1", integer(-75))], secret_integer(15))]
fn division_tests(
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

#[test]
fn division_public_by_zero() -> Result<(), Error> {
    let inputs = StaticInputGeneratorBuilder::default().add_integer("my_int1", 11).add_integer("my_int2", 0).build();
    let outputs = simulate("division_public", inputs);
    assert_eq!(outputs.err().unwrap().to_string(), "division by zero");
    Ok(())
}

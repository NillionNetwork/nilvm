use crate::vm::tests::{integer, secret_integer, secret_unsigned_integer, simulate, unsigned_integer};
use anyhow::{Error, Ok};
use execution_engine_vm::simulator::inputs::StaticInputGeneratorBuilder;
use nada_value::{clear::Clear, NadaValue};
use rstest::rstest;

#[rstest]
/// (x * y).to_public() * 3
#[case("reveal", vec![("my_int1", secret_integer(3)), ("my_int2", secret_integer(4))], integer(36))]
#[case("reveal", vec![("my_int1", secret_integer(4)), ("my_int2", secret_integer(3))], integer(36))]
#[case("reveal", vec![("my_int1", secret_integer(-1)), ("my_int2", secret_integer(-1))], integer(3))]
/// (x * y).to_public() * 3
#[case("reveal_unsigned", vec![("my_uint1_secret", secret_unsigned_integer(3)), ("my_uint2_secret", secret_unsigned_integer(4))], unsigned_integer(36))]
#[case("reveal_unsigned", vec![("my_uint1_secret", secret_unsigned_integer(4)), ("my_uint2_secret", secret_unsigned_integer(3))], unsigned_integer(36))]
#[case("reveal_unsigned", vec![("my_uint1_secret", secret_unsigned_integer(1)), ("my_uint2_secret", secret_unsigned_integer(1))], unsigned_integer(3))]
/// prod = (x*y), sum = (x+y).to_public(), mod = (x%3),
/// tmp_1 = prod.to_public() / 2
/// tmp_2 = sum.to_public() + mod.to_public()
/// output = tmp_1 + tmp_2
#[case("reveal_many_operations", vec![("my_int1", secret_integer(32)), ("my_int2", secret_integer(81))], integer(1411))]
#[case("reveal_many_operations", vec![("my_int1", secret_integer(1)), ("my_int2", secret_integer(2))], integer(5))]
#[case("reveal_many_operations", vec![("my_int1", secret_integer(10)), ("my_int2", secret_integer(10))], integer(71))]
fn reveal_tests(
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

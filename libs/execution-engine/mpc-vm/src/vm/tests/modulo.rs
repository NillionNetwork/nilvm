use crate::vm::tests::{integer, secret_boolean, secret_integer, secret_unsigned_integer, simulate, unsigned_integer};
use anyhow::{Error, Ok};
use execution_engine_vm::simulator::inputs::StaticInputGeneratorBuilder;
use nada_value::{clear::Clear, NadaValue};
use rstest::rstest;

#[rstest]
#[case("modulo_simple_neg", vec![("my_int4", secret_integer(-15)), ("my_neg_int1", secret_integer(12))], secret_integer(9))]
#[case("modulo_public_public", vec![("public_my_int1", integer(42)), ("public_my_int2", integer(12))], integer(6))]
#[case("modulo_public_public", vec![("public_my_int1", integer(-52)), ("public_my_int2", integer(34))], integer(16))]
#[case("modulo_public_public", vec![("public_my_int1", integer(52)), ("public_my_int2", integer(-34))], integer(-16))]
#[case("modulo_public_public", vec![("public_my_int1", integer(-52)), ("public_my_int2", integer(-34))], integer(-18))]
#[case("modulo_public_secret", vec![("public_my_int1", integer(30)), ("my_int2", secret_integer(12))], secret_integer(6))]
#[case("modulo_public_secret", vec![("public_my_int1", integer(-15)), ("my_int2", secret_integer(12))], secret_integer(9))]
#[case("modulo_public_secret", vec![("public_my_int1", integer(15)), ("my_int2", secret_integer(-12))], secret_integer(-9))]
#[case("modulo_public_secret", vec![("public_my_int1", integer(-15)), ("my_int2", secret_integer(-12))], secret_integer(-3))]
#[case("modulo_secret_public", vec![("my_int1", secret_integer(30)), ("public_my_int2", integer(12))], secret_integer(6))]
#[case("modulo_secret_public", vec![("my_int1", secret_integer(-15)), ("public_my_int2", integer(12))], secret_integer(9))]
#[case("modulo_secret_public", vec![("my_int1", secret_integer(15)), ("public_my_int2", integer(-12))], secret_integer(-9))]
#[case("modulo_secret_public", vec![("my_int1", secret_integer(-15)), ("public_my_int2", integer(-12))], secret_integer(-3))]
#[case("modulo_secret_secret", vec![("my_int1", secret_integer(30)), ("my_neg_int1", secret_integer(12))], secret_integer(6))]
#[case("modulo_secret_secret", vec![("my_int1", secret_integer(-15)), ("my_neg_int1", secret_integer(12))], secret_integer(9))]
#[case("modulo_secret_secret", vec![("my_int1", secret_integer(15)), ("my_neg_int1", secret_integer(-12))], secret_integer(-9))]
#[case("modulo_secret_secret", vec![("my_int1", secret_integer(-15)), ("my_neg_int1", secret_integer(-12))], secret_integer(-3))]
#[case("modulo_unsigned_public_public", vec![("my_uint1_public", unsigned_integer(42)), ("my_uint2_public", unsigned_integer(12))], unsigned_integer(6))]
#[case("modulo_unsigned_public_public", vec![("my_uint1_public", unsigned_integer(12)), ("my_uint2_public", unsigned_integer(42))], unsigned_integer(12))]
#[case("modulo_unsigned_public_secret", vec![("my_uint1_public", unsigned_integer(30)), ("my_uint2_secret", secret_unsigned_integer(12))], secret_unsigned_integer(6))]
#[case("modulo_unsigned_public_secret", vec![("my_uint1_public", unsigned_integer(12)), ("my_uint2_secret", secret_unsigned_integer(30))], secret_unsigned_integer(12))]
#[case("modulo_unsigned_secret_public", vec![("my_uint1", secret_unsigned_integer(30)), ("my_uint2_public", unsigned_integer(12))], secret_unsigned_integer(6))]
#[case("modulo_unsigned_secret_public", vec![("my_uint1", secret_unsigned_integer(12)), ("my_uint2_public", unsigned_integer(30))], secret_unsigned_integer(12))]
#[case("modulo_unsigned_secret_secret", vec![("my_uint1", secret_unsigned_integer(30)), ("my_uint2", secret_unsigned_integer(12))], secret_unsigned_integer(6))]
#[case("modulo_unsigned_secret_secret", vec![("my_uint1", secret_unsigned_integer(12)), ("my_uint2", secret_unsigned_integer(30))], secret_unsigned_integer(12))]
#[case("modulo_composition_v1", vec![("my_int1", secret_integer(11)), ("my_int2", secret_integer(7))], secret_integer(12))]
#[case("modulo_composition_v1", vec![("my_int1", secret_integer(7)), ("my_int2", secret_integer(9))], secret_integer(9))]
#[case("modulo_composition_v2", vec![("my_int1", secret_integer(-11)), ("my_int2", secret_integer(7)), ("my_int3", secret_integer(-2))], secret_integer(-1))]
#[case("modulo_composition_v2", vec![("my_int1", secret_integer(7)), ("my_int2", secret_integer(-9)), ("my_int3", secret_integer(3))], secret_integer(-5))]
#[case("modulo_composition_v3", vec![("my_int1", secret_integer(-11)), ("my_int2", secret_integer(7)), ("my_int3", secret_integer(-2))], secret_boolean(true))]
#[case("modulo_composition_v3", vec![("my_int1", secret_integer(7)), ("my_int2", secret_integer(-9)), ("my_int3", secret_integer(4))], secret_boolean(false))]
#[case("modulo_unsigned_literal_public", vec![("my_uint1_public", unsigned_integer(32))], unsigned_integer(1))]
#[case("modulo_unsigned_public_literal", vec![("my_uint1", unsigned_integer(32))], unsigned_integer(32))]
fn modulo_tests(
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

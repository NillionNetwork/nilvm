use crate::vm::tests::{array_non_empty, integer, secret_integer, secret_unsigned_integer, simulate, unsigned_integer};
use anyhow::{Error, Ok};
use execution_engine_vm::simulator::inputs::StaticInputGeneratorBuilder;
use nada_value::{clear::Clear, NadaValue};
use rstest::rstest;

#[rstest]
/// If x < y { return x } else { return y }
#[case("if_else", vec![("my_int1", secret_integer(3)), ("my_int2", secret_integer(4))], secret_integer(3))]
#[case("if_else", vec![("my_int1", secret_integer(4)), ("my_int2", secret_integer(3))], secret_integer(3))]
/// If Pub(x) < y { return x } else { return y }
#[case("if_else_public_secret", vec![("public_my_int1", integer(3)), ("my_int2", secret_integer(4))], secret_integer(3))]
#[case("if_else_public_secret", vec![("public_my_int1", integer(4)), ("my_int2", secret_integer(3))], secret_integer(3))]
/// If x < 10 { return y } else { return 2 }
#[case("if_else_secret_public_literal", vec![("my_int1", secret_integer(3)), ("my_int2", secret_integer(4))], secret_integer(4))]
#[case("if_else_secret_public_literal", vec![("my_int1", secret_integer(13)), ("my_int2", secret_integer(4))], secret_integer(2))]
/// If x < 10 { return 1 } else { return 2 }
#[case("if_else_public_literal_public_literal", vec![("my_int1", secret_integer(3))], secret_integer(1))]
#[case("if_else_public_literal_public_literal", vec![("my_int1", secret_integer(13))], secret_integer(2))]
/// If z < Pub(y) { return x } else { return y }
#[case("if_else_public_public", vec![("my_int", secret_integer(10)), ("public_my_int1", integer(3)), ("public_my_int2", integer(4))], secret_integer(4))]
#[case("if_else_public_public", vec![("my_int", secret_integer(1)), ("public_my_int1", integer(3)), ("public_my_int2", integer(4))], secret_integer(3))]
/// If x < y { return x } else { return y }
#[case("if_else_unsigned", vec![("my_uint1_secret", secret_unsigned_integer(3)), ("my_uint2_secret", secret_unsigned_integer(4))], secret_unsigned_integer(3))]
#[case("if_else_unsigned", vec![("my_uint1_secret", secret_unsigned_integer(4)), ("my_uint2_secret", secret_unsigned_integer(3))], secret_unsigned_integer(3))]
/// If x < y { return 1 } else { return 2 }
#[case("if_else_unsigned_secret_public_literal", vec![("my_uint1", secret_unsigned_integer(3)), ("my_uint2", secret_unsigned_integer(4))], secret_unsigned_integer(1))]
#[case("if_else_unsigned_secret_public_literal", vec![("my_uint1", secret_unsigned_integer(13)), ("my_uint2", secret_unsigned_integer(4))], secret_unsigned_integer(2))]
/// Compare a secret and a literal, output secret or literal
/// If x <= 10u64 return y else return 1
#[case("if_else_unsigned_comp_secret_literal_output_secret_literal", vec![("my_uint1", secret_unsigned_integer(3)), ("my_uint2", secret_unsigned_integer(4))], secret_unsigned_integer(4))]
#[case("if_else_unsigned_comp_secret_literal_output_secret_literal", vec![("my_uint1", secret_unsigned_integer(13)), ("my_uint2", secret_unsigned_integer(4))], secret_unsigned_integer(1))]
/// If z < Pub(y) { return x } else { return y }
#[case("if_else_unsigned_public_public", vec![("my_uint_secret", secret_unsigned_integer(10)), ("my_uint1_public", unsigned_integer(3)), ("my_uint2_public", unsigned_integer(4))], secret_unsigned_integer(4))]
#[case("if_else_unsigned_public_public", vec![("my_uint_secret", secret_unsigned_integer(1)), ("my_uint1_public", unsigned_integer(3)), ("my_uint2_public", unsigned_integer(4))], secret_unsigned_integer(3))]
/// If x < 10 { 1 } else { 2 }
#[case("if_else_public_cond_public_branches", vec![("public_my_int1", integer(3))], integer(1))]
#[case("if_else_public_cond_public_branches", vec![("public_my_int1", integer(13))], integer(2))]
/// If x < 10 { return y } else { return z }
#[case("if_else_public_cond_secret_branches", vec![("public_my_int1", integer(3)), ("my_int2", secret_integer(1)), ("my_int3", secret_integer(2))], secret_integer(1))]
#[case("if_else_public_cond_secret_branches", vec![("public_my_int1", integer(13)), ("my_int2", secret_integer(1)), ("my_int3", secret_integer(2))], secret_integer(2))]
/// If x < 10 { return y } else { return 100 }
#[case("if_else_public_cond_public_secret_branches", vec![("my_int1", integer(3)), ("my_int2", secret_integer(1))], secret_integer(1))]
#[case("if_else_public_cond_public_secret_branches", vec![("my_int1", integer(13)), ("my_int2", secret_integer(1))], secret_integer(100))]
/// If (x < y).to_public() { return 1 } else { return 2 }
#[case("if_else_reveal", vec![("my_int1", secret_integer(10)), ("my_int2", secret_integer(20))], integer(1))]
#[case("if_else_reveal", vec![("my_int1", secret_integer(20)), ("my_int2", secret_integer(10))], integer(2))]
/// If (x < y).to_public() { return x } else { return y }
#[case("if_else_reveal_secret", vec![("my_int1", secret_integer(10)), ("my_int2", secret_integer(20))], secret_integer(10))]
#[case("if_else_reveal_secret", vec![("my_int1", secret_integer(20)), ("my_int2", secret_integer(10))], secret_integer(10))]
/// If x < y { return x } else { return y } + x
#[case("if_else_secret_addition", vec![("my_int1", secret_integer(3)), ("my_int2", secret_integer(4))], secret_integer(6))]
#[case("if_else_secret_addition", vec![("my_int1", secret_integer(4)), ("my_int2", secret_integer(3))], secret_integer(7))]
/// If x < y { return x } else { return y } * x
#[case("if_else_secret_multiplication", vec![("my_int1", secret_integer(3)), ("my_int2", secret_integer(4))], secret_integer(9))]
#[case("if_else_secret_multiplication", vec![("my_int1", secret_integer(4)), ("my_int2", secret_integer(3))], secret_integer(12))]
/// If x < y { return x } else { return y } + x
#[case("if_else_public_addition", vec![("my_int1", integer(3)), ("my_int2", integer(4))], integer(6))]
#[case("if_else_public_addition", vec![("my_int1", integer(4)), ("my_int2", integer(3))], integer(7))]
/// If x < y { return x } else { return y } * x
#[case("if_else_public_multiplication", vec![("my_int1", integer(3)), ("my_int2", integer(4))], integer(9))]
#[case("if_else_public_multiplication", vec![("my_int1", integer(4)), ("my_int2", integer(3))], integer(12))]
/// Array.new(
///   If x < y { return x } else { return y } + 4,
///   If y < x { return x } else { return y } + 4,
///   If x < x { return x } else { return y } + 4
/// )
#[ignore = "functions are broken in MIR preprocessing"]
#[case("if_else_map", vec![("my_int1", secret_integer(3)), ("my_int2", secret_integer(4))], array_non_empty(vec![secret_integer(7), secret_integer(8), secret_integer(7)]))]
#[ignore = "functions are broken in MIR preprocessing"]
#[case("if_else_map", vec![("my_int1", secret_integer(4)), ("my_int2", secret_integer(3))], array_non_empty(vec![secret_integer(7), secret_integer(8), secret_integer(8)]))]
/// output = 0
/// output = If x[0] >= 0 { output = output + 1 } else { output }
/// output = If x[1] >= 0 { output = output + 1 } else { output }
/// output = If x[2] >= 0 { output = output + 1 } else { output }
/// return output
#[case("if_else_reduce", vec![("my_array", array_non_empty(vec![secret_integer(-3), secret_integer(4), secret_integer(0)])), ("public_int0", integer(0))], integer(1))]
#[case("if_else_reduce", vec![("my_array", array_non_empty(vec![secret_integer(-3), secret_integer(4), secret_integer(-8)])), ("public_int0", integer(0))], integer(2))]
#[case("if_else_reduce", vec![("my_array", array_non_empty(vec![secret_integer(5), secret_integer(4), secret_integer(0)])), ("public_int0", integer(0))], integer(0))]
/// output = 0
/// output = If x[0] >= 0 { output = output + 1 } else { output }
/// output = If x[1] >= 0 { output = output + 1 } else { output }
/// output = If x[2] >= 0 { output = output + 1 } else { output }
/// return output
#[case("if_else_complex", vec![("my_int1", secret_integer(1)), ("my_int2", secret_integer(4))], secret_integer(8))]
#[case("if_else_complex", vec![("my_int1", secret_integer(2)), ("my_int2", secret_integer(4))], secret_integer(12))]
#[case("if_else_complex", vec![("my_int1", secret_integer(3)), ("my_int2", secret_integer(4))], secret_integer(14))]
/// (If x < y { x } else { y }).to_public()
#[case("if_else_reveal_result", vec![("my_int1", secret_integer(1)), ("my_int2", secret_integer(4))], integer(1))]
#[case("if_else_reveal_result", vec![("my_int1", secret_integer(5)), ("my_int2", secret_integer(4))], integer(4))]
/// temp = (If x < y { x } else { y }).to_public()
/// output = If temp < 5 { temp } else { 5 }
#[case("if_else_reveal_if_else", vec![("my_int1", secret_integer(1)), ("my_int2", secret_integer(4))], integer(1))]
#[case("if_else_reveal_if_else", vec![("my_int1", secret_integer(4)), ("my_int2", secret_integer(2))], integer(2))]
#[case("if_else_reveal_if_else", vec![("my_int1", secret_integer(8)), ("my_int2", secret_integer(6))], integer(5))]
/// min = If 0 < x { 0 } else { x })
/// min = If min < y { min } else { y }
#[case("if_else_public_if_else_secret", vec![("my_int1", integer(1)), ("my_int2", secret_integer(5))], secret_integer(0))]
#[case("if_else_public_if_else_secret", vec![("my_int1", integer(-4)), ("my_int2", secret_integer(2))], secret_integer(-4))]
#[case("if_else_public_if_else_secret", vec![("my_int1", integer(-4)), ("my_int2", secret_integer(-6))], secret_integer(-6))]
/// If not(x < y) { return x } else { return y }
#[case("not_simple", vec![("my_int1", secret_integer(3)), ("my_int2", secret_integer(4))], secret_integer(4))]
#[case("not_simple", vec![("my_int1", secret_integer(4)), ("my_int2", secret_integer(3))], secret_integer(4))]
fn if_else_tests(
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

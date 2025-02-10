use crate::vm::tests::{integer, secret_integer, secret_unsigned_integer, simulate, unsigned_integer};
use anyhow::{Error, Ok};
use execution_engine_vm::simulator::inputs::StaticInputGeneratorBuilder;
use nada_value::{clear::Clear, NadaValue};
use rstest::rstest;

#[rstest]
/// x << y
/// x >> y
#[case("shift_left", vec![("my_int1", secret_integer(3)), ("amount", unsigned_integer(2))], secret_integer(12))]
#[case("shift_left", vec![("my_int1", secret_integer(2)), ("amount", unsigned_integer(1))], secret_integer(4))]
#[case("shift_left", vec![("my_int1", secret_integer(10)), ("amount", unsigned_integer(3))], secret_integer(80))]
#[case("shift_right", vec![("my_int1", secret_integer(20)), ("amount", unsigned_integer(1))], secret_integer(10))]
#[case("shift_right", vec![("my_int1", secret_integer(2)), ("amount", unsigned_integer(1))], secret_integer(1))]
#[case("shift_right", vec![("my_int1", secret_integer(12)), ("amount", unsigned_integer(2))], secret_integer(3))]
/// (x + y) << 1
/// (x + y) >> 1
#[case("shift_left_after_add", vec![("my_int1", secret_integer(20)), ("my_int2", secret_integer(1))], secret_integer(42))]
#[case("shift_left_after_add", vec![("my_int1", secret_integer(2)), ("my_int2", secret_integer(1))], secret_integer(6))]
#[case("shift_left_after_add", vec![("my_int1", secret_integer(12)), ("my_int2", secret_integer(2))], secret_integer(28))]
#[case("shift_right_after_add", vec![("my_int1", secret_integer(20)), ("my_int2", secret_integer(1))], secret_integer(10))]
#[case("shift_right_after_add", vec![("my_int1", secret_integer(10)), ("my_int2", secret_integer(10))], secret_integer(10))]
#[case("shift_right_after_add", vec![("my_int1", secret_integer(12)), ("my_int2", secret_integer(2))], secret_integer(7))]
/// x << 2
/// x >> 2
#[case("shift_left_literal", vec![("my_int1", secret_integer(3))], secret_integer(12))]
#[case("shift_left_literal", vec![("my_int1", secret_integer(2))], secret_integer(8))]
#[case("shift_left_literal", vec![("my_int1", secret_integer(10))], secret_integer(40))]
#[case("shift_right_literal", vec![("my_int1", secret_integer(12))], secret_integer(3))]
#[case("shift_right_literal", vec![("my_int1", secret_integer(8))], secret_integer(2))]
#[case("shift_right_literal", vec![("my_int1", secret_integer(40))], secret_integer(10))]
/// x << y
/// x >> y
#[case("shift_left_unsigned", vec![("my_int1", secret_unsigned_integer(3)), ("amount", unsigned_integer(2))], secret_unsigned_integer(12))]
#[case("shift_left_unsigned", vec![("my_int1", secret_unsigned_integer(2)), ("amount", unsigned_integer(1))], secret_unsigned_integer(4))]
#[case("shift_left_unsigned", vec![("my_int1", secret_unsigned_integer(10)), ("amount", unsigned_integer(3))], secret_unsigned_integer(80))]
#[case("shift_right_unsigned", vec![("my_int1", secret_unsigned_integer(12)), ("amount", unsigned_integer(2))], secret_unsigned_integer(3))]
#[case("shift_right_unsigned", vec![("my_int1", secret_unsigned_integer(4)), ("amount", unsigned_integer(1))], secret_unsigned_integer(2))]
#[case("shift_right_unsigned", vec![("my_int1", secret_unsigned_integer(80)), ("amount", unsigned_integer(3))], secret_unsigned_integer(10))]
/// x << 2
/// x >> 2
#[case("shift_left_unsigned_literal", vec![("my_uint1", secret_unsigned_integer(3))], secret_unsigned_integer(12))]
#[case("shift_left_unsigned_literal", vec![("my_uint1", secret_unsigned_integer(2))], secret_unsigned_integer(8))]
#[case("shift_left_unsigned_literal", vec![("my_uint1", secret_unsigned_integer(10))], secret_unsigned_integer(40))]
#[case("shift_right_unsigned_literal", vec![("my_uint1", secret_unsigned_integer(12))], secret_unsigned_integer(3))]
#[case("shift_right_unsigned_literal", vec![("my_uint1", secret_unsigned_integer(8))], secret_unsigned_integer(2))]
#[case("shift_right_unsigned_literal", vec![("my_uint1", secret_unsigned_integer(40))], secret_unsigned_integer(10))]
fn shift_tests(
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

#[rstest]
/// out_1 = y + (x << z)
/// out_2 = y * (x << z)
/// out_1 = y + (x >> z)
/// out_2 = y * (x >> z)
#[case("shift_left_complex", vec![("my_int1", secret_integer(3)), ("my_int2", secret_integer(2)), ("amount", unsigned_integer(1))], vec![("out_1", secret_integer(8)), ("out_2", secret_integer(12))])]
#[case("shift_left_complex", vec![("my_int1", secret_integer(2)), ("my_int2", secret_integer(1)), ("amount", unsigned_integer(1))], vec![("out_1", secret_integer(5)), ("out_2", secret_integer(4))])]
#[case("shift_left_complex", vec![("my_int1", secret_integer(10)), ("my_int2", secret_integer(3)), ("amount", unsigned_integer(2))], vec![("out_1", secret_integer(43)), ("out_2", secret_integer(120))])]
#[case("shift_left_complex", vec![("my_int1", secret_integer(10)), ("my_int2", secret_integer(3)), ("amount", unsigned_integer(0))], vec![("out_1", secret_integer(13)), ("out_2", secret_integer(30))])]
#[case("shift_right_complex", vec![("my_int1", secret_integer(8)), ("my_int2", secret_integer(2)), ("amount", unsigned_integer(1))], vec![("out_1", secret_integer(6)), ("out_2", secret_integer(8))])]
#[case("shift_right_complex", vec![("my_int1", secret_integer(2)), ("my_int2", secret_integer(1)), ("amount", unsigned_integer(1))], vec![("out_1", secret_integer(2)), ("out_2", secret_integer(1))])]
#[case("shift_right_complex", vec![("my_int1", secret_integer(20)), ("my_int2", secret_integer(3)), ("amount", unsigned_integer(2))], vec![("out_1", secret_integer(8)), ("out_2", secret_integer(15))])]
#[case("shift_right_complex", vec![("my_int1", secret_integer(40)), ("my_int2", secret_integer(3)), ("amount", unsigned_integer(0))], vec![("out_1", secret_integer(43)), ("out_2", secret_integer(120))])]
/// out_1 = y + (x << 1)
/// out_2 = y * (x << 1)
/// out_3 = (y + y) * (x << 1)
/// out_1 = y + (x >> 1)
/// out_2 = y * (x >> 1)
/// out_3 = (y + y) * (x >> 1)
#[case("shift_left_complex_public", vec![("my_int1", integer(3)), ("my_int2", integer(2))], vec![("out_1", integer(8)), ("out_2", integer(12)), ("out_3", integer(24))])]
#[case("shift_left_complex_public", vec![("my_int1", integer(2)), ("my_int2", integer(1))], vec![("out_1", integer(5)), ("out_2", integer(4)), ("out_3", integer(8))])]
#[case("shift_left_complex_public", vec![("my_int1", integer(10)), ("my_int2", integer(3))], vec![("out_1", integer(23)), ("out_2", integer(60)), ("out_3", integer(120))])]
#[case("shift_right_complex_public", vec![("my_int1", integer(8)), ("my_int2", integer(2))], vec![("out_1", integer(6)), ("out_2", integer(8)), ("out_3", integer(16))])]
#[case("shift_right_complex_public", vec![("my_int1", integer(20)), ("my_int2", integer(1))], vec![("out_1", integer(11)), ("out_2", integer(10)), ("out_3", integer(20))])]
#[case("shift_right_complex_public", vec![("my_int1", integer(40)), ("my_int2", integer(3))], vec![("out_1", integer(23)), ("out_2", integer(60)), ("out_3", integer(120))])]
fn shift_complex_tests(
    #[case] program_name: &str,
    #[case] inputs: Vec<(&str, NadaValue<Clear>)>,
    #[case] expected: Vec<(&str, NadaValue<Clear>)>,
) -> Result<(), Error> {
    println!("Program: {:?}", program_name);

    let inputs = StaticInputGeneratorBuilder::default().add_all(inputs).build();
    let outputs = simulate(program_name, inputs)?;
    assert_eq!(outputs.len(), expected.len());

    for (key, expected_value) in expected {
        let output = outputs.get(key).unwrap();
        assert_eq!(output, &expected_value, "{} output failed", key);
    }

    Ok(())
}

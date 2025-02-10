use crate::vm::tests::{secret_integer, secret_unsigned_integer, simulate};
use anyhow::{Error, Ok};
use execution_engine_vm::simulator::inputs::StaticInputGeneratorBuilder;
use rstest::rstest;

#[rstest]
#[case(1, 1, 1)]
#[case(2, 1, 1)]
#[case(1, - 1, 1)]
#[case(1, 2, 1)]
#[case(4, 2, 5)]
fn distributivity_subtraction(#[case] x: i64, #[case] y: i64, #[case] z: i64) -> Result<(), Error> {
    let inputs = StaticInputGeneratorBuilder::default()
        .add_secret_integer("x", x)
        .add_secret_integer("y", y)
        .add_secret_integer("z", z)
        .build();
    let outputs = simulate("distributivity_subtractions", inputs)?;

    assert_eq!(outputs.len(), 6);
    let out_1 = outputs.get("out_1").unwrap();
    let out_2 = outputs.get("out_2").unwrap();
    let out_3 = outputs.get("out_3").unwrap();
    let out_4 = outputs.get("out_4").unwrap();
    let out_5 = outputs.get("out_5").unwrap();
    let out_6 = outputs.get("out_6").unwrap();

    let out_1_res = x - y + z;
    let out_2_res = x + y - z;
    let out_3_res = x - (y + z);
    let out_4_res = x * (x - (y + z));
    let out_5_res = x - (y + z) + x;
    let out_6_res = x + y - x * y;

    assert_eq!(out_1, &secret_integer(out_1_res));
    assert_eq!(out_2, &secret_integer(out_2_res));
    assert_eq!(out_3, &secret_integer(out_3_res));
    assert_eq!(out_4, &secret_integer(out_4_res));
    assert_eq!(out_5, &secret_integer(out_5_res));
    assert_eq!(out_6, &secret_integer(out_6_res));
    Ok(())
}

#[rstest]
#[case(2, 3, 5, 4, 6, 6)]
fn simple_subtraction_public_variables(
    #[case] i00: u64,
    #[case] i01: u64,
    #[case] i02: u64,
    #[case] i03: u64,
    #[case] i04: u64,
    #[case] expected: u64,
) -> Result<(), Error> {
    let inputs = StaticInputGeneratorBuilder::default()
        .add_secret_unsigned_integer("I00", i00)
        .add_unsigned_integer("I01", i01)
        .add_unsigned_integer("I02", i02)
        .add_secret_unsigned_integer("I03", i03)
        .add_unsigned_integer("I04", i04)
        .build();
    let outputs = simulate("simple_subtraction_public_variables", inputs)?;
    assert_eq!(outputs.len(), 1);
    let output = outputs.get("Sub0").unwrap();
    assert_eq!(output, &secret_unsigned_integer(expected));
    Ok(())
}

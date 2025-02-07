use crate::{
    assert_eq_either,
    vm::tests::{secret_integer, secret_unsigned_integer, simulate},
};
use anyhow::{Error, Ok};
use execution_engine_vm::simulator::inputs::StaticInputGeneratorBuilder;
use rstest::rstest;

// x.trunc_pr(y)
// Since this is probabilistic, we're checking +0 and +1 of the correct value.
#[rstest]
#[case(20, 1, 10)]
#[case(2, 1, 1)]
#[case(12, 2, 3)]
fn trunc_pr(#[case] x: i64, #[case] y: u64, #[case] expected: i64) -> Result<(), Error> {
    let inputs = StaticInputGeneratorBuilder::default()
        .add_secret_integer("my_int1", x)
        .add_unsigned_integer("amount", y)
        .build();
    let outputs = simulate("trunc_pr", inputs)?;
    assert_eq!(outputs.len(), 1);
    let output = outputs.get("my_output").unwrap();
    assert_eq_either!(output, &secret_integer(expected), &secret_integer(expected - 1));
    Ok(())
}

// x.trunc_pr(2)
// Since this is probabilistic, we're checking +0 and +1 of the correct value.
#[rstest]
#[case(12, 3)]
#[case(8, 2)]
#[case(40, 10)]
fn trunc_pr_literal(#[case] x: i64, #[case] expected: i64) -> Result<(), Error> {
    let inputs = StaticInputGeneratorBuilder::default().add_secret_integer("my_int1", x).build();
    let outputs = simulate("trunc_pr_literal", inputs)?;
    assert_eq!(outputs.len(), 1);
    let output = outputs.get("my_output").unwrap();
    assert_eq_either!(output, &secret_integer(expected), &secret_integer(expected - 1));
    Ok(())
}

// (x + y) >> 1
#[rstest]
#[case(20, 1, 10)]
#[case(10, 10, 10)]
#[case(12, 2, 7)]
fn trunc_pr_after_add(#[case] x: i64, #[case] y: i64, #[case] expected: i64) -> Result<(), Error> {
    let inputs = StaticInputGeneratorBuilder::default()
        .add_secret_integer("my_int1", x)
        .add_secret_integer("my_int2", y)
        .build();
    let outputs = simulate("trunc_pr_after_add", inputs)?;
    assert_eq!(outputs.len(), 1);
    let output = outputs.get("my_output").unwrap();
    assert_eq_either!(output, &secret_integer(expected), &secret_integer(expected - 1));
    Ok(())
}

// out_1 = y + (x.trunc_pr(z))
// out_2 = y * (x.trunc_pr(z))
// Since this is probabilistic, we're checking +0 and +1 of the correct value.
#[rstest]
#[case(8, 1, 1, 5, 4)]
#[case(2, 1, 1, 2, 1)]
#[case(40, 1, 0, 41, 40)]
fn trunc_pr_complex(
    #[case] x: i64,
    #[case] y: i64,
    #[case] z: u64,
    #[case] expected_1: i64,
    #[case] expected_2: i64,
) -> Result<(), Error> {
    let inputs = StaticInputGeneratorBuilder::default()
        .add_secret_integer("my_int1", x)
        .add_secret_integer("my_int2", y)
        .add_unsigned_integer("amount", z)
        .build();
    let outputs = simulate("trunc_pr_complex", inputs)?;
    assert_eq!(outputs.len(), 2);
    let output_1 = outputs.get("out_1").unwrap();
    let output_2 = outputs.get("out_2").unwrap();
    assert_eq_either!(output_1, &secret_integer(expected_1), &secret_integer(expected_1 - 1));
    assert_eq_either!(output_2, &secret_integer(expected_2), &secret_integer(expected_2 - 1));
    Ok(())
}

// x.trunc_pr(y)
// Since this is probabilistic, we're checking +0 and +1 of the correct value.
#[rstest]
#[case(12, 2, 3)]
#[case(4, 1, 2)]
#[case(80, 3, 10)]
fn trunc_pr_unsigned(#[case] x: u64, #[case] y: u64, #[case] expected: u64) -> Result<(), Error> {
    let inputs = StaticInputGeneratorBuilder::default()
        .add_secret_unsigned_integer("my_int1", x)
        .add_unsigned_integer("amount", y)
        .build();
    let outputs = simulate("trunc_pr_unsigned", inputs)?;
    assert_eq!(outputs.len(), 1);
    let output = outputs.get("my_output").unwrap();
    assert_eq_either!(output, &secret_unsigned_integer(expected), &secret_unsigned_integer(expected - 1));
    Ok(())
}

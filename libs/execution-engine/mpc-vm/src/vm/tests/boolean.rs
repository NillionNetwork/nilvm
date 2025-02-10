use crate::vm::tests::{boolean, integer, secret_boolean, secret_integer, simulate, unsigned_integer};
use anyhow::{Ok, Result};
use execution_engine_vm::simulator::inputs::StaticInputGeneratorBuilder;
use nada_value::{clear::Clear, NadaValue};
use rstest::rstest;

#[rstest]
#[case(false, false)]
#[case(false, true)]
#[case(true, false)]
#[case(true, true)]
fn boolean_gates(#[case] x: bool, #[case] y: bool) -> Result<()> {
    let inputs = StaticInputGeneratorBuilder::default()
        .add_unsigned_integer("x", x as u64)
        .add_unsigned_integer("y", y as u64)
        .build();
    let outputs = simulate("boolean_gates", inputs)?;
    assert_eq!(outputs.len(), 7);

    let and_gate = outputs.get("and_gate").unwrap();
    let or_gate = outputs.get("or_gate").unwrap();
    let xor_gate = outputs.get("xor_gate").unwrap();
    let not_gate = outputs.get("not_gate").unwrap();
    let nand_gate = outputs.get("nand_gate").unwrap();
    let nor_gate = outputs.get("nor_gate").unwrap();
    let xnor_gate = outputs.get("xnor_gate").unwrap();

    let expected_and = x & y;
    let expected_or = x | y;
    let expected_xor = x ^ y;
    let expected_not = !x;
    let expected_nand = !(x & y);
    let expected_nor = !(x | y);
    let expected_xnor = !(x ^ y);

    assert_eq!(and_gate, &unsigned_integer(expected_and.into()), "AND({}, {})", x, y);
    assert_eq!(or_gate, &unsigned_integer(expected_or.into()), "OR({}, {})", x, y);
    assert_eq!(xor_gate, &unsigned_integer(expected_xor.into()), "XOR({}, {})", x, y);
    assert_eq!(not_gate, &unsigned_integer(expected_not.into()), "NOT({})", x);
    assert_eq!(nand_gate, &unsigned_integer(expected_nand.into()), "NAND({}, {})", x, y);
    assert_eq!(nor_gate, &unsigned_integer(expected_nor.into()), "NOR({}, {})", x, y);
    assert_eq!(xnor_gate, &unsigned_integer(expected_xnor.into()), "XNOR({}, {})", x, y);
    Ok(())
}

#[rstest]
#[case("boolean_and", vec![("A_neg", secret_integer(-4)), ("B_neg", secret_integer(-5)), ("C", secret_integer(2))], secret_boolean(true))] // ( -4 < (-5 + 2)) & (-4 < 2)
#[case("boolean_and_public", vec![("public_A", integer(-4)), ("public_B", integer(-5)), ("public_C", integer(2))], boolean(true))]
#[case("boolean_or", vec![("A_neg", secret_integer(-4)), ("B_neg", secret_integer(-5)), ("C", secret_integer(2))], secret_boolean(true))] // ( -4 < (-5 + 2)) | (-4 < 2)
#[case("boolean_or", vec![("A_neg", secret_integer(10)), ("B_neg", secret_integer(-5)), ("C", secret_integer(2))], secret_boolean(false))]
#[case("boolean_xor", vec![("A_neg", secret_integer(-4)), ("B_neg", secret_integer(-5)), ("C", secret_integer(2))], secret_boolean(false))] // ( -4 < (-5 + 2)) ^ (-4 < 2)
#[case("boolean_xor", vec![("A_neg", secret_integer(10)), ("B_neg", secret_integer(-5)), ("C", secret_integer(2))], secret_boolean(false))]
fn test_boolean_operations(
    #[case] program_name: &str,
    #[case] inputs: Vec<(&str, NadaValue<Clear>)>,
    #[case] expected: NadaValue<Clear>,
) -> Result<()> {
    let inputs = StaticInputGeneratorBuilder::default().add_all(inputs).build();
    let outputs = simulate(program_name, inputs)?;
    assert_eq!(outputs.len(), 1);
    let output = outputs.get("my_output").unwrap();
    assert_eq!(output, &expected);
    Ok(())
}

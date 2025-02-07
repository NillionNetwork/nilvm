use crate::vm::tests::simulate;
use anyhow::{Error, Ok};
use execution_engine_vm::simulator::inputs::StaticInputGeneratorBuilder;
use rstest::rstest;

// SecretInteger.random()
// Since this is probabilistic, we're not checking the output value.
#[rstest]
fn random_signed() -> Result<(), Error> {
    let inputs = StaticInputGeneratorBuilder::default().build();
    let outputs = simulate("random_value_simple", inputs)?;
    assert_eq!(outputs.len(), 1);
    let _output = outputs.get("my_output").unwrap();
    Ok(())
}

// SecretUnsignedInteger.random()
// Since this is probabilistic, we're not checking the output value.
#[rstest]
fn random_unsigned() -> Result<(), Error> {
    let inputs = StaticInputGeneratorBuilder::default().build();
    let outputs = simulate("random_value_unsigned", inputs)?;
    assert_eq!(outputs.len(), 1);
    let _output = outputs.get("my_output").unwrap();
    Ok(())
}

// SecretInteger.random() + secret value
// Since this is probabilistic, we're not checking the output value.
#[rstest]
#[case(8)]
fn random_compound(#[case] x: i64) -> Result<(), Error> {
    let inputs = StaticInputGeneratorBuilder::default().add_integer("my_int1", x).build();
    let outputs = simulate("random_value_unsigned", inputs)?;
    assert_eq!(outputs.len(), 1);
    let _output = outputs.get("my_output").unwrap();
    Ok(())
}

// SecretBoolean.random()
// Since this is probabilistic, we're not checking the output value.
#[rstest]
fn random_boolean() -> Result<(), Error> {
    let inputs = StaticInputGeneratorBuilder::default().build();
    let outputs = simulate("random_boolean", inputs)?;
    assert_eq!(outputs.len(), 1);
    let _output = outputs.get("my_output").unwrap();
    Ok(())
}

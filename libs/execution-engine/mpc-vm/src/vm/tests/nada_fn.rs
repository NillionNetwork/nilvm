use crate::vm::tests::{array_non_empty, secret_integer, secret_unsigned_integer, simulate};
use anyhow::{Error, Ok};
use execution_engine_vm::simulator::inputs::StaticInputGeneratorBuilder;
use nada_value::{clear::Clear, NadaValue};
use rstest::rstest;

#[rstest]
/// max(x, y)
#[case("nada_fn_max", vec![("my_int1", secret_integer(32)), ("my_int2", secret_integer(81))], secret_integer(81))]
#[case("nada_fn_max", vec![("my_int1", secret_integer(81)), ("my_int2", secret_integer(32))], secret_integer(81))]
#[case("nada_fn_compound", vec![("my_int1", secret_integer(81)), ("my_int2", secret_integer(32))], secret_integer(9153))]
#[case("nada_fn_compound_triple", vec![("my_int1", secret_integer(32)), ("my_int2", secret_integer(81))], secret_integer(3697))]
/// min(x, y)
#[case("nada_fn_min_unsigned", vec![("my_int1", secret_unsigned_integer(32)), ("my_int2", secret_unsigned_integer(81))], secret_unsigned_integer(32))]
#[case("nada_fn_min_unsigned", vec![("my_int1", secret_unsigned_integer(81)), ("my_int2", secret_unsigned_integer(32))], secret_unsigned_integer(32))]
/// Tests function reuse
#[case("nada_fn_reuse", vec![("my_int", secret_integer(7))], array_non_empty(vec![
        secret_integer(14),
        secret_integer(14),
        secret_integer(14)
    ]))]
fn nada_fn_tests(
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

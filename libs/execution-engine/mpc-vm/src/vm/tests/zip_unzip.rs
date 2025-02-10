use crate::vm::tests::{array_non_empty, integer, secret_integer, simulate, tuple};
use anyhow::{Error, Ok};
use execution_engine_vm::simulator::inputs::StaticInputGeneratorBuilder;
use nada_value::{clear::Clear, NadaValue};
use rstest::rstest;

#[rstest]
#[case("zip_simple", vec![("my_array_1", array_non_empty(vec![
        secret_integer(1),
        secret_integer(2),
        secret_integer(3),
    ])), ("my_array_2", array_non_empty(vec![
        secret_integer(2),
        secret_integer(3),
        secret_integer(4),
    ]))], array_non_empty(
    vec![
        tuple(secret_integer(1), secret_integer(2)),
        tuple(secret_integer(2), secret_integer(3)),
        tuple(secret_integer(3), secret_integer(4)),
    ],
))]
// my_array_1.zip(my_array_2).map(|(left, right)| left + right)
#[ignore = "functions are broken in MIR preprocessing"]
#[case("zip_map", vec![("my_array_1", array_non_empty(vec![
        secret_integer(1),
        secret_integer(2),
        secret_integer(3),
    ])), ("my_array_2", array_non_empty(vec![
        secret_integer(2),
        secret_integer(3),
        secret_integer(4),
    ]))], array_non_empty(
    vec![secret_integer(3), secret_integer(5), secret_integer(7)],
))]
#[case("unzip_simple", vec![("my_array_1", array_non_empty(vec![
        secret_integer(1),
        secret_integer(2),
        secret_integer(3),
    ])), ("my_array_2", array_non_empty(vec![
        secret_integer(2),
        secret_integer(3),
        secret_integer(4),
    ]))], tuple(
    array_non_empty(vec![
        secret_integer(1), secret_integer(2), secret_integer(3)
    ]),
    array_non_empty(vec![
        secret_integer(2), secret_integer(3), secret_integer(4)
    ])
))]
// additions = left.zip(right).map(add)
// additions = additions.zip(additions).map(add)
#[ignore = "functions are broken in MIR preprocessing"]
#[case("zip_additions_zip_additions", vec![("a", secret_integer(1)), ("b", secret_integer(2))], array_non_empty(
    vec![secret_integer(6), secret_integer(6), secret_integer(6)]
))]
// additions = left.zip(right).map(add)
// additions = additions.zip(additions).map(add)
#[ignore = "functions are broken in MIR preprocessing"]
#[case("zip_additions_zip_additions_public", vec![("a", integer(1)), ("b", integer(2))], array_non_empty(
    vec![integer(6), integer(6), integer(6)]
))]
// additions = left.zip(right).map(add)
// multiplications = additions.zip(additions).map(mul)
#[ignore = "functions are broken in MIR preprocessing"]
#[case("zip_additions_zip_multiplications", vec![("a", secret_integer(1)), ("b", secret_integer(2))], array_non_empty(
    vec![secret_integer(9), secret_integer(9), secret_integer(9)]
))]
// additions = left.zip(right).map(add)
// multiplications = additions.zip(additions).map(mul)
#[ignore = "functions are broken in MIR preprocessing"]
#[case("zip_additions_zip_multiplications_public", vec![("a", integer(1)), ("b", integer(2))], array_non_empty(
    vec![integer(9), integer(9), integer(9)]
))]
// multiplications = left.zip(right).map(mul)
// multiplications = multiplications.zip(multiplications).map(mul)
#[ignore = "functions are broken in MIR preprocessing"]
#[case("zip_multiplications_zip_multiplications", vec![("a", secret_integer(2)), ("b", secret_integer(3))], array_non_empty(
    vec![secret_integer(36), secret_integer(36), secret_integer(36)]
))]
// multiplications = left.zip(right).map(mul)
// multiplications = multiplications.zip(multiplications).map(mul)
#[ignore = "functions are broken in MIR preprocessing"]
#[case("zip_multiplications_zip_multiplications_public", vec![("a", integer(2)), ("b", integer(3))], array_non_empty(
    vec![integer(36), integer(36), integer(36)]
))]
// multiplications = left.zip(right).map(mul)
// additions = multiplications.zip(multiplications).map(add)
#[ignore = "functions are broken in MIR preprocessing"]
#[case("zip_multiplications_zip_additions", vec![("a", secret_integer(1)), ("b", secret_integer(3))], array_non_empty(
    vec![secret_integer(6), secret_integer(6), secret_integer(6)]
))]
// multiplications = left.zip(right).map(mul)
// additions = multiplications.zip(multiplications).map(add)
#[ignore = "functions are broken in MIR preprocessing"]
#[case("zip_multiplications_zip_additions_public", vec![("a", integer(1)), ("b", integer(3))], array_non_empty(
    vec![integer(6), integer(6), integer(6)]
))]
fn zip_tests(
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

use anyhow::{Error, Ok};
use rstest::rstest;

use nada_value::{clear::Clear, NadaValue};

use crate::vm::tests::{array_non_empty, integer, secret_integer, simulate, tuple};
use execution_engine_vm::simulator::inputs::StaticInputGeneratorBuilder;

#[rstest]
#[case::input_array("input_array", vec![("my_integer_array", array_non_empty(vec![
        secret_integer(1),
        secret_integer(2),
        secret_integer(3),
        secret_integer(4),
        secret_integer(5),
    ]))],
    array_non_empty(vec![
        secret_integer(1),
        secret_integer(2),
        secret_integer(3),
        secret_integer(4),
        secret_integer(5)
    ]))]
#[case::array_chaining_map_map("array_chaining_map_map", vec![("my_array_1", array_non_empty(vec![
        secret_integer(1),
        secret_integer(2),
        secret_integer(3),
    ])), ("my_int", secret_integer(2))],
    array_non_empty(vec![
        secret_integer(5),
        secret_integer(6),
        secret_integer(7)
    ]))]
#[case::array_chaining_map_reduce("array_chaining_map_reduce", vec![("my_array_1", array_non_empty(vec![
        secret_integer(1),
        secret_integer(2),
        secret_integer(3),
    ])), ("my_int", secret_integer(3))],
    secret_integer(21))]
#[case::array_chaining_zip_zip("array_chaining_zip_zip", vec![("my_array_1", array_non_empty(vec![
        secret_integer(1),
        secret_integer(2),
        secret_integer(3),
    ])), ("my_array_2", array_non_empty(vec![
        secret_integer(2),
        secret_integer(3),
        secret_integer(4),
    ])), ("my_array_3", array_non_empty(vec![
        secret_integer(3),
        secret_integer(4),
        secret_integer(5),
    ]))],
    array_non_empty(
        vec![
            tuple(
                tuple(secret_integer(1), secret_integer(2)),
                secret_integer(3),
            ),
            tuple(
                tuple(secret_integer(2), secret_integer(3)),
                secret_integer(4),
            ),
            tuple(
                tuple(secret_integer(3), secret_integer(4)),
                secret_integer(5),
            ),
        ],
    ))]
#[case::array_new("array_new", vec![("a", secret_integer(1)), ("b", secret_integer(2))],
    array_non_empty(vec![
        secret_integer(1),
        secret_integer(2)
    ]))]
#[case::array_new_public("array_new_public", vec![("a", integer(1)), ("b", integer(2))],
    array_non_empty(vec![
        integer(1),
        integer(2)
    ]))]
#[case::array_new_before_operation("array_new_before_operation", vec![("a", secret_integer(1)), ("b", secret_integer(2)), ("my_int", secret_integer(3))],
    secret_integer(6))]
#[case::array_new_after_operation("array_new_after_operation", vec![("a", secret_integer(1)), ("b", secret_integer(2))],
    array_non_empty(vec![
        secret_integer(3)
    ]))]
#[case::array_new_complex("array_new_complex", vec![("a", secret_integer(1)), ("b", secret_integer(2)), ("c", secret_integer(3)), ("my_int", secret_integer(4))],
    secret_integer(10))]
#[case::array_new_2_dimensional("array_new_2_dimensional", vec![("a", secret_integer(1)), ("b", secret_integer(2)), ("c", secret_integer(3)), ("d", secret_integer(4))],
    array_non_empty(vec![
        array_non_empty(vec![secret_integer(1), secret_integer(2)]),
        array_non_empty(vec![secret_integer(3), secret_integer(4)])
    ]))]
#[case::array_product("array_product", vec![("my_array_1", array_non_empty(vec![
        secret_integer(1),
        secret_integer(2),
        secret_integer(3),
    ])), ("my_array_2", array_non_empty(vec![
        secret_integer(2),
        secret_integer(3),
        secret_integer(4),
    ]))],
    array_non_empty(vec![
        secret_integer(2),
        secret_integer(6),
        secret_integer(12)
    ]))]
fn array_tests(
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

/// Test that it fails when the provided array's length are different from the expected array's length.
/// `reduce_simple` expects an array of length 3, but we provide an array of length 4.
#[test]
fn test_array_type_mismatch() -> Result<(), Error> {
    let array_secret =
        array_non_empty(vec![secret_integer(1), secret_integer(2), secret_integer(3), secret_integer(3)]);
    let inputs = StaticInputGeneratorBuilder::default().add("my_array_1", array_secret).build();
    let output = simulate("reduce_simple", inputs);
    assert!(output.is_err());
    Ok(())
}

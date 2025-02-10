//! Operations: lists all possible operations through a combination of input types.
//! Allows generation of Nada types, tests, and a summary table.

#![deny(missing_docs)]
#![forbid(unsafe_code)]
#![deny( // We are a bit more lax here since this code is not meant to be used directly in production.
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::iterator_step_by_zero,
    clippy::invalid_regex,
    clippy::string_slice
)]

use types::{
    BinaryOperation, BuiltOperations, ClassMethod, InstanceMethod, InstanceMethodVariants, Operations, Reason,
    UnaryOperation,
};

use crate::types::{Identifier, Literal, PythonShape};

pub mod output;
/// Contains all types used in the operations crate.
pub mod types;

/// Returns a list of allowed an disallowed operations.
pub fn build() -> BuiltOperations {
    use crate::types::{DataType, OperationType::*};

    // We create type groups to avoid repetition.
    let boolean_types = vec![
        DataType::Literal(Literal::Boolean),
        DataType::Identifier(Identifier::Boolean),
        DataType::Identifier(Identifier::SecretBoolean),
    ];

    let signed_types = vec![
        DataType::Literal(Literal::Integer),
        DataType::Identifier(Identifier::Integer),
        DataType::Identifier(Identifier::SecretInteger),
    ];

    let unsigned_types = vec![
        DataType::Literal(Literal::UnsignedInteger),
        DataType::Identifier(Identifier::UnsignedInteger),
        DataType::Identifier(Identifier::SecretUnsignedInteger),
    ];

    let numeric_types = vec![
        DataType::Literal(Literal::Integer),
        DataType::Identifier(Identifier::Integer),
        DataType::Identifier(Identifier::UnsignedInteger),
        DataType::Literal(Literal::UnsignedInteger),
        DataType::Identifier(Identifier::SecretInteger),
        DataType::Identifier(Identifier::SecretUnsignedInteger),
        DataType::Identifier(Identifier::ShamirShareInteger),
        DataType::Identifier(Identifier::ShamirShareUnsignedInteger),
    ];

    let compound_types = vec![
        DataType::Identifier(Identifier::Tuple),
        DataType::Identifier(Identifier::Array),
        DataType::Identifier(Identifier::NTuple),
        DataType::Identifier(Identifier::Object),
    ];

    let shamir_types = vec![
        DataType::Identifier(Identifier::ShamirShareInteger),
        DataType::Identifier(Identifier::ShamirShareUnsignedInteger),
        DataType::Identifier(Identifier::ShamirShareBoolean),
    ];

    let secret_types = vec![
        DataType::Identifier(Identifier::SecretInteger),
        DataType::Identifier(Identifier::SecretUnsignedInteger),
        DataType::Identifier(Identifier::SecretBoolean),
    ];

    let literal_types = vec![
        DataType::Literal(Literal::Integer),
        DataType::Literal(Literal::UnsignedInteger),
        DataType::Literal(Literal::Boolean),
    ];

    let non_secret_types = vec![
        DataType::Literal(Literal::Integer),
        DataType::Literal(Literal::UnsignedInteger),
        DataType::Literal(Literal::Boolean),
        DataType::Identifier(Identifier::Integer),
        DataType::Identifier(Identifier::UnsignedInteger),
        DataType::Identifier(Identifier::Boolean),
    ];

    let blob_types = vec![DataType::Identifier(Identifier::SecretBlob)];

    // By default all types are allowed for all operations.
    // The following list of operations declares all forbidden and allowed combinations of types.

    Operations::default()
        // Arithmetic
        .add_binary(
            BinaryOperation::new(Arithmetic, "Addition", PythonShape::operator("add", "+"))
                .forbid(&boolean_types, Reason::type_error().with_description("boolean arithmetic"))
                .forbid(&compound_types, Reason::not_yet_implemented())
                .forbid(&shamir_types, Reason::impossible_math())
                .forbid(&blob_types, Reason::type_error())
                .build(),
        )
        .add_binary(
            BinaryOperation::new(Arithmetic, "Subtraction", PythonShape::operator("sub", "-"))
                .forbid(&boolean_types, Reason::type_error().with_description("boolean arithmetic"))
                .forbid(&compound_types, Reason::not_yet_implemented())
                .forbid(&shamir_types, Reason::impossible_math())
                .forbid(&blob_types, Reason::type_error())
                .build(),
        )
        .add_binary(
            BinaryOperation::new(Arithmetic, "Multiplication", PythonShape::operator("mul", "*"))
                .forbid(&boolean_types, Reason::type_error().with_description("boolean arithmetic"))
                .forbid(&compound_types, Reason::not_yet_implemented())
                .forbid(&shamir_types, Reason::impossible_math())
                .forbid(&blob_types, Reason::type_error())
                .build(),
        )
        .add_binary(
            BinaryOperation::new(Arithmetic, "Division", PythonShape::operator("truediv", "/"))
                .forbid(&boolean_types, Reason::type_error().with_description("boolean arithmetic"))
                .forbid(&compound_types, Reason::not_yet_implemented())
                .forbid(&shamir_types, Reason::impossible_math())
                .forbid(&blob_types, Reason::type_error())
                .forbid_zero_right()
                .build(),
        )
        .add_binary(
            BinaryOperation::new(Arithmetic, "Modulo", PythonShape::operator("mod", "%"))
                .forbid(&boolean_types, Reason::type_error().with_description("boolean arithmetic"))
                .forbid(&compound_types, Reason::not_yet_implemented())
                .forbid(&shamir_types, Reason::impossible_math())
                .forbid(&blob_types, Reason::type_error())
                .forbid_zero_right()
                .build(),
        )
        .add_binary(
            BinaryOperation::new(Arithmetic, "Power", PythonShape::operator("pow", "**"))
                .forbid(&boolean_types, Reason::type_error().with_description("boolean arithmetic"))
                .forbid(&compound_types, Reason::not_yet_implemented())
                .forbid(&shamir_types, Reason::impossible_math())
                .forbid(&blob_types, Reason::type_error())
                .forbid_left(&secret_types, Reason::not_yet_implemented().with_description("secret exponent"))
                .forbid_right(&secret_types, Reason::type_error().with_description("secret exponent"))
                .forbid_zero_left()
                .build(),
        )
        .add_binary(
            BinaryOperation::new(Arithmetic, "LeftShift", PythonShape::operator("lshift", "<<"))
                .allow_multiple(
                    &[
                        DataType::Identifier(Identifier::SecretInteger),
                        DataType::Identifier(Identifier::SecretUnsignedInteger),
                        DataType::Identifier(Identifier::Integer),
                        DataType::Identifier(Identifier::UnsignedInteger),
                    ],
                    &[DataType::Literal(Literal::UnsignedInteger), DataType::Identifier(Identifier::UnsignedInteger)],
                )
                .allow_with_output(
                    DataType::Literal(Literal::Integer),
                    DataType::Literal(Literal::UnsignedInteger),
                    DataType::Literal(Literal::Integer),
                )
                .forbid(&boolean_types, Reason::type_error().with_description("boolean arithmetic"))
                .forbid(&compound_types, Reason::not_yet_implemented())
                .forbid(&shamir_types, Reason::impossible_math())
                .forbid(&blob_types, Reason::type_error())
                .forbid_right(&shamir_types, Reason::impossible_math())
                .forbid_right(&secret_types, Reason::type_error().with_description("secret left shift amount"))
                .forbid_right(&signed_types, Reason::not_yet_implemented().with_description("negative shift amount"))
                .build(),
        )
        .add_binary(
            BinaryOperation::new(Arithmetic, "RightShift", PythonShape::operator("rshift", ">>"))
                .allow_multiple(
                    &[
                        DataType::Identifier(Identifier::SecretInteger),
                        DataType::Identifier(Identifier::SecretUnsignedInteger),
                        DataType::Identifier(Identifier::Integer),
                        DataType::Identifier(Identifier::UnsignedInteger),
                    ],
                    &[DataType::Literal(Literal::UnsignedInteger), DataType::Identifier(Identifier::UnsignedInteger)],
                )
                .allow_with_output(
                    DataType::Literal(Literal::Integer),
                    DataType::Literal(Literal::UnsignedInteger),
                    DataType::Literal(Literal::Integer),
                )
                .forbid(&boolean_types, Reason::type_error().with_description("boolean arithmetic"))
                .forbid(&compound_types, Reason::not_yet_implemented())
                .forbid(&shamir_types, Reason::impossible_math())
                .forbid(&blob_types, Reason::type_error())
                .forbid_right(&shamir_types, Reason::impossible_math())
                .forbid_right(&secret_types, Reason::type_error().with_description("secret right shift amount"))
                .forbid_right(&signed_types, Reason::type_error().with_description("negative shift amount"))
                .build(),
        )
        // Logical
        .add_binary(
            BinaryOperation::new(Logical, "LessThan", PythonShape::operator("lt", "<"))
                .forbid(&boolean_types, Reason::type_error().with_description("boolean has no order"))
                .forbid(&compound_types, Reason::not_yet_implemented())
                .forbid(&shamir_types, Reason::impossible_math())
                .forbid(&blob_types, Reason::type_error())
                .build(),
        )
        .add_binary(
            BinaryOperation::new(Logical, "GreaterThan", PythonShape::operator("gt", ">"))
                .forbid(&boolean_types, Reason::type_error().with_description("boolean has no order"))
                .forbid(&compound_types, Reason::not_yet_implemented())
                .forbid(&shamir_types, Reason::impossible_math())
                .forbid(&blob_types, Reason::type_error())
                .build(),
        )
        .add_binary(
            BinaryOperation::new(Logical, "LessOrEqualThan", PythonShape::operator("le", "<="))
                .forbid(&boolean_types, Reason::type_error().with_description("boolean has no order"))
                .forbid(&compound_types, Reason::not_yet_implemented())
                .forbid(&shamir_types, Reason::impossible_math())
                .forbid(&blob_types, Reason::type_error())
                .build(),
        )
        .add_binary(
            BinaryOperation::new(Logical, "GreaterOrEqualThan", PythonShape::operator("ge", ">="))
                .forbid(&boolean_types, Reason::type_error().with_description("boolean has no order"))
                .forbid(&compound_types, Reason::not_yet_implemented())
                .forbid(&shamir_types, Reason::impossible_math())
                .forbid(&blob_types, Reason::type_error())
                .build(),
        )
        .add_binary(
            BinaryOperation::new(Logical, "Equals", PythonShape::operator("eq", "=="))
                .forbid(&compound_types, Reason::not_yet_implemented())
                .forbid(&shamir_types, Reason::impossible_math())
                .forbid(&blob_types, Reason::type_error())
                .build(),
        )
        .add_binary(
            BinaryOperation::new(Logical, "NotEquals", PythonShape::operator("ne", "!="))
                .forbid(&compound_types, Reason::not_yet_implemented())
                .forbid(&shamir_types, Reason::impossible_math())
                .forbid(&blob_types, Reason::type_error())
                .build(),
        )
        .add_binary(
            BinaryOperation::new(Logical, "PublicOutputEquality", PythonShape::instance_method("public_equals"))
                .forbid(&boolean_types, Reason::not_yet_implemented())
                .forbid(&compound_types, Reason::not_yet_implemented())
                .forbid(&shamir_types, Reason::impossible_math())
                .forbid(&literal_types, Reason::not_yet_implemented())
                .forbid(&blob_types, Reason::type_error())
                .force_public_output_override()
                .build(),
        )
        .add_class_method(
            ClassMethod::new("Random", "random")
                .add_type(DataType::Identifier(Identifier::SecretInteger))
                .add_type(DataType::Identifier(Identifier::SecretUnsignedInteger))
                .add_type(DataType::Identifier(Identifier::SecretBoolean))
                .build(),
        )
        .add_instance_method(
            InstanceMethod::new("Reveal", "to_public", &[])
                // Reveal can only be called on secret types
                .add_type(
                    InstanceMethodVariants::new(DataType::Identifier(Identifier::SecretBoolean), 0)
                        .with_parameters(vec![], Some(DataType::Identifier(Identifier::Boolean)))
                        .build(),
                )
                .add_type(
                    InstanceMethodVariants::new(DataType::Identifier(Identifier::SecretInteger), 0)
                        .with_parameters(vec![], Some(DataType::Identifier(Identifier::Integer)))
                        .build(),
                )
                .add_type(
                    InstanceMethodVariants::new(DataType::Identifier(Identifier::SecretUnsignedInteger), 0)
                        .with_parameters(vec![], Some(DataType::Identifier(Identifier::UnsignedInteger)))
                        .build(),
                )
                .build(),
        )
        .add_binary(
            BinaryOperation::new(Arithmetic, "TruncPr", PythonShape::instance_method("trunc_pr"))
                .allow_multiple(
                    &[
                        DataType::Identifier(Identifier::SecretInteger),
                        DataType::Identifier(Identifier::SecretUnsignedInteger),
                    ],
                    &[DataType::Literal(Literal::UnsignedInteger), DataType::Identifier(Identifier::UnsignedInteger)],
                )
                .forbid(&boolean_types, Reason::type_error().with_description("boolean arithmetic"))
                .forbid(&compound_types, Reason::not_yet_implemented())
                .forbid(&shamir_types, Reason::impossible_math())
                .forbid(&blob_types, Reason::type_error())
                .forbid_right(&shamir_types, Reason::impossible_math())
                .forbid_right(&secret_types, Reason::type_error().with_description("secret truncation amount"))
                .forbid_right(&signed_types, Reason::type_error().with_description("negative truncation amount"))
                .forbid_left(
                    &non_secret_types,
                    Reason::type_error().with_description("probabilistic truncation only applies to secrets"),
                )
                .build(),
        )
        .add_binary(
            BinaryOperation::new(Logical, "BooleanAnd", PythonShape::operator("and", "&"))
                .forbid(&numeric_types, Reason::type_error())
                .forbid(&compound_types, Reason::type_error())
                .forbid(&shamir_types, Reason::type_error())
                .forbid(&blob_types, Reason::type_error())
                .build(),
        )
        .add_binary(
            BinaryOperation::new(Logical, "BooleanOr", PythonShape::operator("or", "|"))
                .forbid(&numeric_types, Reason::type_error())
                .forbid(&compound_types, Reason::type_error())
                .forbid(&shamir_types, Reason::type_error())
                .forbid(&blob_types, Reason::type_error())
                .build(),
        )
        .add_binary(
            BinaryOperation::new(Logical, "BooleanXor", PythonShape::operator("xor", "^"))
                .forbid(&numeric_types, Reason::type_error())
                .forbid(&compound_types, Reason::type_error())
                .forbid(&shamir_types, Reason::type_error())
                .forbid(&blob_types, Reason::type_error())
                .build(),
        )
        .add_instance_method(
            InstanceMethod::new("IfElse", "if_else", &[])
                .add_type(
                    InstanceMethodVariants::new(DataType::Identifier(Identifier::SecretBoolean), 2)
                        .with_parameters_permutations_with_repetition(
                            signed_types,
                            Some(DataType::Identifier(Identifier::SecretInteger)),
                        )
                        .with_parameters_permutations_with_repetition(
                            unsigned_types,
                            Some(DataType::Identifier(Identifier::SecretUnsignedInteger)),
                        )
                        .build(),
                )
                .add_type(
                    InstanceMethodVariants::new(DataType::Identifier(Identifier::Boolean), 2)
                        // Secret operations (secret with secret).
                        .with_parameters_permutations_with_repetition(
                            vec![DataType::Identifier(Identifier::SecretInteger)],
                            Some(DataType::Identifier(Identifier::SecretInteger)),
                        )
                        .with_parameters_permutations_with_repetition(
                            vec![DataType::Identifier(Identifier::SecretUnsignedInteger)],
                            Some(DataType::Identifier(Identifier::SecretUnsignedInteger)),
                        )
                        // Mixed operations (secret with public or literals).
                        .with_parameters_permutations(
                            vec![
                                DataType::Identifier(Identifier::SecretInteger),
                                DataType::Identifier(Identifier::Integer),
                            ],
                            Some(DataType::Identifier(Identifier::SecretInteger)),
                        )
                        .with_parameters_permutations(
                            vec![
                                DataType::Identifier(Identifier::SecretUnsignedInteger),
                                DataType::Identifier(Identifier::UnsignedInteger),
                            ],
                            Some(DataType::Identifier(Identifier::SecretUnsignedInteger)),
                        )
                        .with_parameters_permutations(
                            vec![
                                DataType::Identifier(Identifier::SecretUnsignedInteger),
                                DataType::Literal(Literal::UnsignedInteger),
                            ],
                            Some(DataType::Identifier(Identifier::SecretUnsignedInteger)),
                        )
                        // Public operations (public or literal with public or
                        // literal) are automatically converted to secret.
                        .with_parameters_permutations_with_repetition(
                            vec![
                                DataType::Identifier(Identifier::Integer), // Comment to keep formatting.
                                DataType::Literal(Literal::Integer),
                            ],
                            Some(DataType::Identifier(Identifier::Integer)),
                        )
                        .with_parameters_permutations_with_repetition(
                            vec![
                                DataType::Identifier(Identifier::UnsignedInteger), // Comment to keep formatting.
                                DataType::Literal(Literal::UnsignedInteger),
                            ],
                            Some(DataType::Identifier(Identifier::UnsignedInteger)),
                        )
                        .build(),
                )
                .build(),
        )
        .add_unary(
            UnaryOperation::new(Logical, "Not", PythonShape::operator("invert", "~"))
                .forbid(&numeric_types, Reason::type_error())
                .forbid(&compound_types, Reason::type_error())
                .forbid(&blob_types, Reason::type_error())
                .build(),
        )
        .build()
}

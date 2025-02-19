use super::*;
use crate::mir2bytecode::MIR2Bytecode;
use anyhow::Error;
use nada_type::NadaType;
use rstest::rstest;
use test_programs::PROGRAMS;

type AssertBinaryOperation = fn(&ProgramBytecode, usize, NadaType, usize, usize) -> Result<(), Error>;

#[test]
fn input_integer() -> Result<(), Error> {
    let mir = PROGRAMS.mir("input_integer")?;
    let bytecode = MIR2Bytecode::transform(&mir)?;
    assert_parties(&bytecode, vec!["Party1"]);
    assert_inputs(&bytecode, vec![(0, "a", NadaType::SecretInteger)])?;

    assert_eq!(bytecode.operations_count(), 1);
    assert_load(&bytecode, 0, NadaType::SecretInteger, 0)?;

    assert_outputs(&bytecode, vec![(0, "my_output", NadaType::SecretInteger, 0)]);
    Ok(())
}

#[test]
fn input_array() -> Result<(), Error> {
    let size = 5usize;
    let mir = PROGRAMS.mir("input_array")?;
    let bytecode = MIR2Bytecode::transform(&mir)?;
    let array_type = NadaType::Array { inner_type: Box::new(NadaType::SecretInteger), size };
    assert_parties(&bytecode, vec!["Party1"]);
    assert_inputs(&bytecode, vec![(0, "my_integer_array", array_type.clone())])?;

    assert_eq!(bytecode.operations_count(), size + 1);
    assert_new(&bytecode, 0, array_type.clone())?;
    for address in 1..=size {
        assert_load(&bytecode, address, NadaType::SecretInteger, address)?;
    }

    assert_outputs(&bytecode, vec![(0, "my_output", array_type, 0)]);
    Ok(())
}

#[test]
fn reveal_operation() -> Result<(), Error> {
    let mir = PROGRAMS.mir("reveal")?;
    let bytecode = MIR2Bytecode::transform(&mir)?;
    assert_parties(&bytecode, vec!["Party2", "Party1"]);
    let (my_int1_addr, my_int2_addr) = (0, 1);
    assert_inputs(
        &bytecode,
        vec![(my_int2_addr, "my_int2", NadaType::SecretInteger), (my_int1_addr, "my_int1", NadaType::SecretInteger)],
    )?;

    assert_eq!(bytecode.operations_count(), 6);
    assert_load(&bytecode, 0, NadaType::SecretInteger, 0)?;
    assert_load(&bytecode, 1, NadaType::SecretInteger, 1)?;
    assert_multiplication(&bytecode, 2, NadaType::SecretInteger, my_int1_addr, my_int2_addr)?;
    assert_reveal(&bytecode, 3, NadaType::Integer, 2)?;
    assert_literal_ref(&bytecode, 4, NadaType::Integer, 0)?;
    assert_multiplication(&bytecode, 5, NadaType::Integer, 3, 4)?;

    assert_outputs(&bytecode, vec![(0, "my_output", NadaType::Integer, 5)]);
    Ok(())
}

#[rstest]
#[case::addition("addition_simple", assert_addition)] // my_int1 + my_int2
#[case::multiplication("multiplication_simple", assert_multiplication)] // my_int1 * my_int2
#[case::division("division_secret_secret", assert_division)] // my_int1 / my_int2
fn binary_operation(
    #[case] program: &'static str,
    #[case] assert_operation: AssertBinaryOperation,
) -> Result<(), Error> {
    let mir = PROGRAMS.mir(program)?;
    let bytecode = MIR2Bytecode::transform(&mir)?;
    assert_parties(&bytecode, vec!["Party1"]);
    assert_inputs(&bytecode, vec![(0, "my_int1", NadaType::SecretInteger), (1, "my_int2", NadaType::SecretInteger)])?;

    assert_eq!(bytecode.operations_count(), 3);
    assert_load(&bytecode, 0, NadaType::SecretInteger, 0)?;
    assert_load(&bytecode, 1, NadaType::SecretInteger, 1)?;
    assert_operation(&bytecode, 2, NadaType::SecretInteger, 0, 1)?;

    assert_outputs(&bytecode, vec![(0, "my_output", NadaType::SecretInteger, 2)]);
    Ok(())
}

#[test]
// my_int2 - my_int1
fn subtraction() -> Result<(), Error> {
    let mir = PROGRAMS.mir("subtraction_simple")?;
    let bytecode = MIR2Bytecode::transform(&mir)?;
    assert_parties(&bytecode, vec!["Party1"]);
    assert_inputs(&bytecode, vec![(0, "my_int1", NadaType::SecretInteger), (1, "my_int2", NadaType::SecretInteger)])?;

    assert_eq!(bytecode.operations_count(), 3);
    assert_load(&bytecode, 0, NadaType::SecretInteger, 0)?;
    assert_load(&bytecode, 1, NadaType::SecretInteger, 1)?;
    assert_subtraction(&bytecode, 2, NadaType::SecretInteger, 1, 0)?;

    assert_outputs(&bytecode, vec![(0, "my_output", NadaType::SecretInteger, 2)]);
    Ok(())
}

#[test]
// my_int1 % my_int2
fn modulo() -> Result<(), Error> {
    let mir = PROGRAMS.mir("modulo_secret_secret")?;
    let bytecode = MIR2Bytecode::transform(&mir)?;
    assert_parties(&bytecode, vec!["Party1"]);
    assert_inputs(
        &bytecode,
        vec![(1, "my_neg_int1", NadaType::SecretInteger), (0, "my_int1", NadaType::SecretInteger)],
    )?;

    assert_eq!(bytecode.operations_count(), 3);
    assert_load(&bytecode, 0, NadaType::SecretInteger, 0)?;
    assert_load(&bytecode, 1, NadaType::SecretInteger, 1)?;
    assert_modulo(&bytecode, 2, NadaType::SecretInteger, 0, 1)?;

    assert_outputs(&bytecode, vec![(0, "my_output", NadaType::SecretInteger, 2)]);
    Ok(())
}

#[test]
// a ** b
fn power() -> Result<(), Error> {
    let mir = PROGRAMS.mir("power_public_unsigned_integer_base_public_unsigned_integer_exponent")?;
    let bytecode = MIR2Bytecode::transform(&mir)?;
    assert_parties(&bytecode, vec!["Party1"]);
    assert_inputs(&bytecode, vec![(0, "a", NadaType::UnsignedInteger), (1, "b", NadaType::UnsignedInteger)])?;

    assert_eq!(bytecode.operations_count(), 3);
    assert_load(&bytecode, 0, NadaType::UnsignedInteger, 0)?;
    assert_load(&bytecode, 1, NadaType::UnsignedInteger, 1)?;
    assert_power(&bytecode, 2, NadaType::UnsignedInteger, 0, 1)?;

    assert_outputs(&bytecode, vec![(0, "my_output", NadaType::UnsignedInteger, 2)]);
    Ok(())
}

#[test]
// my_int1 << amount
fn left_shift() -> Result<(), Error> {
    let mir = PROGRAMS.mir("shift_left")?;
    let bytecode = MIR2Bytecode::transform(&mir)?;
    assert_parties(&bytecode, vec!["Party1"]);
    assert_inputs(&bytecode, vec![(0, "amount", NadaType::UnsignedInteger), (1, "my_int1", NadaType::SecretInteger)])?;

    assert_eq!(bytecode.operations_count(), 3);
    assert_load(&bytecode, 1, NadaType::SecretInteger, 1)?;
    assert_load(&bytecode, 0, NadaType::UnsignedInteger, 0)?;
    assert_left_shift(&bytecode, 2, NadaType::SecretInteger, 1, 0)?;

    assert_outputs(&bytecode, vec![(0, "my_output", NadaType::SecretInteger, 2)]);
    Ok(())
}

#[test]
// my_int1 >> amount
fn right_shift() -> Result<(), Error> {
    let mir = PROGRAMS.mir("shift_right")?;
    let bytecode = MIR2Bytecode::transform(&mir)?;
    assert_parties(&bytecode, vec!["Party1"]);
    assert_inputs(&bytecode, vec![(0, "amount", NadaType::UnsignedInteger), (1, "my_int1", NadaType::SecretInteger)])?;

    assert_eq!(bytecode.operations_count(), 3);
    assert_load(&bytecode, 1, NadaType::SecretInteger, 1)?;
    assert_load(&bytecode, 0, NadaType::UnsignedInteger, 0)?;
    assert_right_shift(&bytecode, 2, NadaType::SecretInteger, 1, 0)?;

    assert_outputs(&bytecode, vec![(0, "my_output", NadaType::SecretInteger, 2)]);
    Ok(())
}

#[test]
// A * B + C < B * D
fn less_than() -> Result<(), Error> {
    let mir = PROGRAMS.mir("less_than")?;
    let bytecode = MIR2Bytecode::transform(&mir)?;
    assert_parties(&bytecode, vec!["Party2", "Party1"]);
    let (a_addr, c_addr, b_addr, d_addr) = (0, 1, 2, 3);
    assert_inputs(
        &bytecode,
        vec![
            (a_addr, "A", NadaType::SecretInteger), // A
            (c_addr, "C", NadaType::SecretInteger), // C
            (b_addr, "B", NadaType::SecretInteger), // B
            (d_addr, "D", NadaType::SecretInteger), // D
        ],
    )?;

    assert_eq!(bytecode.operations_count(), 8);
    assert_load(&bytecode, a_addr, NadaType::SecretInteger, a_addr)?; // A
    assert_load(&bytecode, b_addr, NadaType::SecretInteger, b_addr)?; // B
    assert_load(&bytecode, c_addr, NadaType::SecretInteger, c_addr)?; // C
    assert_load(&bytecode, d_addr, NadaType::SecretInteger, d_addr)?; // D
    assert_multiplication(&bytecode, 4, NadaType::SecretInteger, a_addr, b_addr)?; // A * B
    assert_addition(&bytecode, 5, NadaType::SecretInteger, 4, c_addr)?; // A * B + C
    assert_multiplication(&bytecode, 6, NadaType::SecretInteger, b_addr, d_addr)?; // B * D 
    assert_less_than(&bytecode, 7, NadaType::SecretBoolean, 5, 6)?; // A * B + C < B * D

    assert_outputs(&bytecode, vec![(0, "my_output", NadaType::SecretBoolean, 7)]);
    Ok(())
}

#[test]
// A * B + C <= B * D
fn less_or_equal_than() -> Result<(), Error> {
    let mir = PROGRAMS.mir("less_or_equal_than")?;
    let bytecode = MIR2Bytecode::transform(&mir)?;
    assert_parties(&bytecode, vec!["Party2", "Party1"]);
    let (a_addr, c_addr, b_addr, d_addr) = (0, 1, 2, 3);
    assert_inputs(
        &bytecode,
        vec![
            (a_addr, "A", NadaType::SecretInteger), // A
            (c_addr, "C", NadaType::SecretInteger), // C
            (b_addr, "B", NadaType::SecretInteger), // B
            (d_addr, "D", NadaType::SecretInteger), // D
        ],
    )?;

    assert_eq!(bytecode.operations_count(), 9);
    assert_load(&bytecode, a_addr, NadaType::SecretInteger, a_addr)?; // A
    assert_load(&bytecode, b_addr, NadaType::SecretInteger, b_addr)?; // B
    assert_load(&bytecode, c_addr, NadaType::SecretInteger, c_addr)?; // C
    assert_load(&bytecode, d_addr, NadaType::SecretInteger, d_addr)?; // D
    assert_multiplication(&bytecode, 4, NadaType::SecretInteger, b_addr, d_addr)?; // B * D
    assert_multiplication(&bytecode, 5, NadaType::SecretInteger, a_addr, b_addr)?; // A * B
    assert_addition(&bytecode, 6, NadaType::SecretInteger, 5, c_addr)?; // A * B + C 
    assert_less_than(&bytecode, 7, NadaType::SecretBoolean, 4, 6)?; // B * D < A * B + C
    assert_not(&bytecode, 8, NadaType::SecretBoolean, 7)?; // !(B * D < A * B + C)

    assert_outputs(&bytecode, vec![(0, "my_output", NadaType::SecretBoolean, 8)]);
    Ok(())
}

#[test]
// A * B + C > B * D
fn greater_than() -> Result<(), Error> {
    let mir = PROGRAMS.mir("greater_than")?;
    let bytecode = MIR2Bytecode::transform(&mir)?;
    assert_parties(&bytecode, vec!["Party2", "Party1"]);
    let (a_addr, c_addr, b_addr, d_addr) = (0, 1, 2, 3);
    assert_inputs(
        &bytecode,
        vec![
            (a_addr, "A", NadaType::SecretInteger), // A
            (c_addr, "C", NadaType::SecretInteger), // C
            (b_addr, "B", NadaType::SecretInteger), // B
            (d_addr, "D", NadaType::SecretInteger), // D
        ],
    )?;

    assert_eq!(bytecode.operations_count(), 8);
    assert_load(&bytecode, a_addr, NadaType::SecretInteger, a_addr)?; // A
    assert_load(&bytecode, b_addr, NadaType::SecretInteger, b_addr)?; // B
    assert_load(&bytecode, c_addr, NadaType::SecretInteger, c_addr)?; // C
    assert_load(&bytecode, d_addr, NadaType::SecretInteger, d_addr)?; // D
    assert_multiplication(&bytecode, 4, NadaType::SecretInteger, b_addr, d_addr)?; // B * D
    assert_multiplication(&bytecode, 5, NadaType::SecretInteger, a_addr, b_addr)?; // A * B
    assert_addition(&bytecode, 6, NadaType::SecretInteger, 5, c_addr)?; // A * B + C 
    assert_less_than(&bytecode, 7, NadaType::SecretBoolean, 4, 6)?; // B * D < A * B + C

    assert_outputs(&bytecode, vec![(0, "my_output", NadaType::SecretBoolean, 7)]);
    Ok(())
}

#[test]
// A * B + C >= B * D
fn greater_or_equal_than() -> Result<(), Error> {
    let mir = PROGRAMS.mir("greater_or_equal_than")?;
    let bytecode = MIR2Bytecode::transform(&mir)?;
    assert_parties(&bytecode, vec!["Party1", "Party2"]);
    let (a_addr, c_addr, b_addr, d_addr) = (0, 1, 2, 3);
    assert_inputs(
        &bytecode,
        vec![
            (a_addr, "A", NadaType::SecretInteger), // A
            (c_addr, "C", NadaType::SecretInteger), // C
            (b_addr, "B", NadaType::SecretInteger), // B
            (d_addr, "D", NadaType::SecretInteger), // D
        ],
    )?;

    assert_eq!(bytecode.operations_count(), 9);
    assert_load(&bytecode, a_addr, NadaType::SecretInteger, a_addr)?; // A
    assert_load(&bytecode, b_addr, NadaType::SecretInteger, b_addr)?; // B
    assert_load(&bytecode, c_addr, NadaType::SecretInteger, c_addr)?; // C
    assert_load(&bytecode, d_addr, NadaType::SecretInteger, d_addr)?; // D
    assert_multiplication(&bytecode, 4, NadaType::SecretInteger, a_addr, b_addr)?; // A * B
    assert_addition(&bytecode, 5, NadaType::SecretInteger, 4, c_addr)?; // A * B + C
    assert_multiplication(&bytecode, 6, NadaType::SecretInteger, b_addr, d_addr)?; // B * D 
    assert_less_than(&bytecode, 7, NadaType::SecretBoolean, 5, 6)?; // A * B + C < B * D
    assert_not(&bytecode, 8, NadaType::SecretBoolean, 7)?;

    assert_outputs(&bytecode, vec![(0, "my_output", NadaType::SecretBoolean, 8)]);
    Ok(())
}

#[test]
// (A * B + C).public_equals(B * D)
fn public_output_equality() -> Result<(), Error> {
    let mir = PROGRAMS.mir("public_output_equality")?;
    let bytecode = MIR2Bytecode::transform(&mir)?;
    assert_parties(&bytecode, vec!["Party2", "Party1"]);
    let (a_addr, c_addr, b_addr, d_addr) = (0, 1, 2, 3);
    assert_inputs(
        &bytecode,
        vec![
            (a_addr, "A", NadaType::SecretInteger), // A
            (c_addr, "C", NadaType::SecretInteger), // C
            (b_addr, "B", NadaType::SecretInteger), // B
            (d_addr, "D", NadaType::SecretInteger), // D
        ],
    )?;

    assert_eq!(bytecode.operations_count(), 8);
    assert_load(&bytecode, a_addr, NadaType::SecretInteger, a_addr)?; // A
    assert_load(&bytecode, b_addr, NadaType::SecretInteger, b_addr)?; // B
    assert_load(&bytecode, c_addr, NadaType::SecretInteger, c_addr)?; // C
    assert_load(&bytecode, d_addr, NadaType::SecretInteger, d_addr)?; // D
    assert_multiplication(&bytecode, 4, NadaType::SecretInteger, a_addr, b_addr)?; // A * B
    assert_addition(&bytecode, 5, NadaType::SecretInteger, 4, c_addr)?; // A * B + C
    assert_multiplication(&bytecode, 6, NadaType::SecretInteger, b_addr, d_addr)?; // B * D 
    assert_public_output_equality(&bytecode, 7, NadaType::Boolean, 5, 6)?; // (A * B + C).public_equals(B * D)

    assert_outputs(&bytecode, vec![(0, "my_output", NadaType::Boolean, 7)]);
    Ok(())
}

#[test]
// private_key.ecdsa_sign(digest)
fn ecdsa_sign_with_public_key() -> Result<(), Error> {
    let mir = PROGRAMS.mir("ecdsa_sign_with_public_key")?;
    let bytecode = MIR2Bytecode::transform(&mir)?;
    assert_parties(&bytecode, vec!["tecdsa_key_party", "tecdsa_digest_message_party", "tecdsa_output_party"]);
    let (pubk_addr, sig_addr, privk_addr, d_addr) = (3, 2, 1, 0);
    assert_inputs(
        &bytecode,
        vec![
            (privk_addr, "tecdsa_private_key", NadaType::EcdsaPrivateKey),
            (d_addr, "tecdsa_digest_message", NadaType::EcdsaDigestMessage),
        ],
    )?;

    assert_eq!(bytecode.operations_count(), 4);
    assert_load(&bytecode, privk_addr, NadaType::EcdsaPrivateKey, privk_addr)?; // private_key
    assert_public_key_derive(&bytecode, pubk_addr, NadaType::EcdsaPublicKey, privk_addr)?; // private_key.public_key()
    assert_load(&bytecode, d_addr, NadaType::EcdsaDigestMessage, d_addr)?; // digest
    assert_ecdsa_sign(&bytecode, sig_addr, NadaType::EcdsaSignature, privk_addr, d_addr)?; // private_key.ecdsa_sign(digest)

    assert_outputs(
        &bytecode,
        vec![
            (0, "tecdsa_signature", NadaType::EcdsaSignature, sig_addr),
            (1, "tecdsa_digest_message", NadaType::EcdsaDigestMessage, d_addr),
            (2, "tecdsa_public_key", NadaType::EcdsaPublicKey, pubk_addr),
        ],
    );
    Ok(())
}

#[test]
// private_key.eddsa_sign(message)
fn eddsa_sign_with_public_key() -> Result<(), Error> {
    let mir = PROGRAMS.mir("eddsa_sign_with_public_key")?;
    let bytecode = MIR2Bytecode::transform(&mir)?;
    assert_parties(&bytecode, vec!["teddsa_key_party", "teddsa_message_party", "teddsa_output_party"]);
    let (pubk_addr, sig_addr, d_addr, privk_addr) = (3, 2, 1, 0);
    assert_inputs(
        &bytecode,
        vec![
            (privk_addr, "teddsa_private_key", NadaType::EddsaPrivateKey),
            (d_addr, "teddsa_message", NadaType::EddsaMessage),
        ],
    )?;

    assert_eq!(bytecode.operations_count(), 4);
    assert_load(&bytecode, privk_addr, NadaType::EddsaPrivateKey, privk_addr)?; // private_key
    assert_public_key_derive(&bytecode, pubk_addr, NadaType::EddsaPublicKey, privk_addr)?; // private_key.public_key()
    assert_load(&bytecode, d_addr, NadaType::EddsaMessage, d_addr)?; // message
    assert_eddsa_sign(&bytecode, sig_addr, NadaType::EddsaSignature, privk_addr, d_addr)?; // private_key.eddsa_sign(message)

    assert_outputs(
        &bytecode,
        vec![
            (0, "teddsa_signature", NadaType::EddsaSignature, sig_addr),
            (1, "teddsa_message", NadaType::EddsaMessage, d_addr),
            (2, "teddsa_public_key", NadaType::EddsaPublicKey, pubk_addr),
        ],
    );
    Ok(())
}

#[test]
// cond = my_int1 < my_int2
// output = cond.if_else(my_int1, my_int2)
fn if_else() -> Result<(), Error> {
    let mir = PROGRAMS.mir("if_else")?;
    let bytecode = MIR2Bytecode::transform(&mir)?;
    assert_parties(&bytecode, vec!["Party2", "Party1"]);
    let (my_int1_addr, my_int2_addr) = (0, 1);
    assert_inputs(
        &bytecode,
        vec![
            (my_int2_addr, "my_int2", NadaType::SecretInteger), // my_int2
            (my_int1_addr, "my_int1", NadaType::SecretInteger), // my_int1
        ],
    )?;

    assert_eq!(bytecode.operations_count(), 4);
    assert_load(&bytecode, 0, NadaType::SecretInteger, 0)?; // my_int2
    assert_load(&bytecode, 1, NadaType::SecretInteger, 1)?; // my_int1
    assert_less_than(&bytecode, 2, NadaType::SecretBoolean, my_int1_addr, my_int2_addr)?; // my_int1 < my_int2
    // (my_int1 < my_int2).if_else(my_int1, my_int2)
    assert_if_else(&bytecode, 3, NadaType::SecretInteger, 2, my_int1_addr, my_int2_addr)?;

    assert_outputs(&bytecode, vec![(0, "my_output", NadaType::SecretInteger, 3)]);
    Ok(())
}

#[test]
fn complex_operation_mix() -> Result<(), Error> {
    let mir = PROGRAMS.mir("complex_operation_mix")?;
    let bytecode = MIR2Bytecode::transform(&mir)?;
    assert_parties(&bytecode, vec!["Party1", "Party2"]);
    let (a_addr, c_addr, b_addr, d_addr, e_addr, f_addr, g_addr) = (0, 1, 2, 3, 4, 5, 6);
    assert_inputs(
        &bytecode,
        vec![
            (a_addr, "A", NadaType::SecretInteger), // A
            (c_addr, "C", NadaType::SecretInteger), // C
            (b_addr, "B", NadaType::SecretInteger), // B
            (d_addr, "D", NadaType::SecretInteger), // D
            (e_addr, "E", NadaType::SecretInteger), // E
            (f_addr, "F", NadaType::SecretInteger), // F
            (g_addr, "G", NadaType::SecretInteger), // G
        ],
    )?;

    assert_eq!(bytecode.operations_count(), 25);
    assert_load(&bytecode, a_addr, NadaType::SecretInteger, a_addr)?; // A
    assert_load(&bytecode, b_addr, NadaType::SecretInteger, b_addr)?; // B
    assert_load(&bytecode, c_addr, NadaType::SecretInteger, c_addr)?; // C
    assert_load(&bytecode, d_addr, NadaType::SecretInteger, d_addr)?; // D
    assert_load(&bytecode, e_addr, NadaType::SecretInteger, e_addr)?; // E
    assert_load(&bytecode, f_addr, NadaType::SecretInteger, f_addr)?; // F
    assert_load(&bytecode, g_addr, NadaType::SecretInteger, g_addr)?; // G

    // A * B
    assert_multiplication(&bytecode, 7, NadaType::SecretInteger, a_addr, b_addr)?;
    // (A * B) + C
    assert_addition(&bytecode, 8, NadaType::SecretInteger, 7, c_addr)?;
    // ((A * B) + C + D)
    assert_addition(&bytecode, 9, NadaType::SecretInteger, 8, d_addr)?;
    // F + G
    assert_addition(&bytecode, 10, NadaType::SecretInteger, f_addr, g_addr)?;
    // E * (F + G)
    assert_multiplication(&bytecode, 11, NadaType::SecretInteger, e_addr, 10)?;
    // ((A * B) + C + D) * (E * (F + G))
    assert_multiplication(&bytecode, 12, NadaType::SecretInteger, 9, 11)?;
    // A * B
    assert_multiplication(&bytecode, 13, NadaType::SecretInteger, a_addr, b_addr)?;
    // C + D
    assert_addition(&bytecode, 14, NadaType::SecretInteger, c_addr, d_addr)?;
    // A * B * (C + D)
    assert_multiplication(&bytecode, 15, NadaType::SecretInteger, 13, 14)?;
    // A * B * (C + D) + E
    assert_addition(&bytecode, 16, NadaType::SecretInteger, 15, e_addr)?;
    // (A * B * (C + D) + E) * F
    assert_multiplication(&bytecode, 17, NadaType::SecretInteger, 16, f_addr)?;
    // ((A * B) + C + D) * (E * (F + G)) + (A * B * (C + D) + E) * F
    assert_addition(&bytecode, 18, NadaType::SecretInteger, 12, 17)?;
    // E + F
    assert_addition(&bytecode, 19, NadaType::SecretInteger, e_addr, f_addr)?;
    // D * (E + F)
    assert_multiplication(&bytecode, 20, NadaType::SecretInteger, d_addr, 19)?;
    // C + (D * (E + F))
    assert_addition(&bytecode, 21, NadaType::SecretInteger, c_addr, 20)?;
    // B * (C + (D * (E + F)))
    assert_multiplication(&bytecode, 22, NadaType::SecretInteger, b_addr, 21)?;
    // A + (B * (C + (D * (E + F))))
    assert_addition(&bytecode, 23, NadaType::SecretInteger, a_addr, 22)?;
    // ((A * B) + C + D) * (E * (F + G)) + (A * B * (C + D) + E) * F + (A + (B * (C + (D * (E + F)))))
    assert_addition(&bytecode, 24, NadaType::SecretInteger, 18, 23)?;

    assert_outputs(&bytecode, vec![(0, "my_output", NadaType::SecretInteger, 24)]);
    Ok(())
}

#[test]
// my_array_1.zip(my_array_2).zip(my_array_3)
fn array_chaining_zip_zip() -> Result<(), Error> {
    let size = 3usize;
    let mir = PROGRAMS.mir("array_chaining_zip_zip")?;
    let bytecode = MIR2Bytecode::transform(&mir)?;
    assert_parties(&bytecode, vec!["Party1"]);
    let input_type = NadaType::Array { inner_type: Box::new(NadaType::SecretInteger), size };
    let my_array_1_address = 0;
    let my_array_2_address = my_array_1_address + size + 1;
    let my_array_3_address = my_array_2_address + size + 1;
    assert_inputs(
        &bytecode,
        vec![
            (my_array_1_address, "my_array_1", input_type.clone()), // my_array_1
            (my_array_2_address, "my_array_2", input_type.clone()), // my_array_2
            (my_array_3_address, "my_array_3", input_type.clone()), // my_array_3
        ],
    )?;

    // 3 array inputs -> 3 x (1 new + size x loads)
    let mut address = 0;
    for _ in 0..3 {
        assert_new(&bytecode, address, input_type.clone())?;
        address += 1;
        for _ in 0..size {
            assert_load(&bytecode, address, NadaType::SecretInteger, address)?;
            address += 1;
        }
    }

    // 1st zip operation
    // size x new tuples -> size x (1 new + 2 get)
    let inner_tuple_type =
        NadaType::Tuple { left_type: Box::new(NadaType::SecretInteger), right_type: Box::new(NadaType::SecretInteger) };
    let first_inner_tuple_address = address;
    for index in 0..size {
        assert_new(&bytecode, address, inner_tuple_type.clone())?;
        address += 1;
        assert_get(&bytecode, address, NadaType::SecretInteger, my_array_1_address + index + 1)?;
        address += 1;
        assert_get(&bytecode, address, NadaType::SecretInteger, my_array_2_address + index + 1)?;
        address += 1;
    }

    // new array -> 1 new + size x (1 get)
    let inner_array_type = NadaType::Array { inner_type: Box::new(inner_tuple_type.clone()), size };
    assert_new(&bytecode, address, inner_array_type)?;
    address += 1;
    for index in 0..size {
        assert_get(&bytecode, address, inner_tuple_type.clone(), first_inner_tuple_address + index * 3)?;
        address += 1;
    }

    // 2nd zip operation
    // size x new tuples -> size x (1 new + 2 get)
    let tuple_type = NadaType::Tuple {
        left_type: Box::new(inner_tuple_type.clone()),
        right_type: Box::new(NadaType::SecretInteger),
    };
    let first_tuple_address = address;
    for index in 0..size {
        assert_new(&bytecode, address, tuple_type.clone())?;
        address += 1;
        assert_get(&bytecode, address, inner_tuple_type.clone(), first_inner_tuple_address + index * 3)?;
        address += 1;
        assert_get(&bytecode, address, NadaType::SecretInteger, my_array_3_address + index + 1)?;
        address += 1;
    }

    // new array -> 1 new + size x (1 get)
    let inner_array_type = NadaType::Array { inner_type: Box::new(tuple_type.clone()), size };
    assert_new(&bytecode, address, inner_array_type.clone())?;
    let output_address = address;
    address += 1;
    for index in 0..size {
        assert_get(&bytecode, address, tuple_type.clone(), first_tuple_address + index * 3)?;
        address += 1;
    }

    assert_eq!(bytecode.operations_count(), address);
    assert_outputs(&bytecode, vec![(0, "my_output", inner_array_type, output_address)]);
    Ok(())
}

#[test]
// my_array_1.zip(my_array_2).zip(my_array_3)
fn unzip() -> Result<(), Error> {
    let size = 3usize;
    let mir = PROGRAMS.mir("unzip_simple")?;
    let bytecode = MIR2Bytecode::transform(&mir)?;
    assert_parties(&bytecode, vec!["Party1"]);
    let input_type = NadaType::Array { inner_type: Box::new(NadaType::SecretInteger), size };
    let my_array_1_address = 0;
    let my_array_2_address = my_array_1_address + size + 1;
    assert_inputs(
        &bytecode,
        vec![
            (my_array_1_address, "my_array_1", input_type.clone()), // my_array_1
            (my_array_2_address, "my_array_2", input_type.clone()), // my_array_2
        ],
    )?;

    // 2 array inputs -> 2 x (1 new + size x loads)
    let mut address = 0;
    for _ in 0..2 {
        assert_new(&bytecode, address, input_type.clone())?;
        address += 1;
        for _ in 0..size {
            assert_load(&bytecode, address, NadaType::SecretInteger, address)?;
            address += 1;
        }
    }

    // 1st zip operation
    // size x new tuples -> size x (1 new + 2 get)
    let inner_tuple_type =
        NadaType::Tuple { left_type: Box::new(NadaType::SecretInteger), right_type: Box::new(NadaType::SecretInteger) };
    let frist_inner_tuple_address = address;
    for index in 0..size {
        assert_new(&bytecode, address, inner_tuple_type.clone())?;
        address += 1;
        assert_get(&bytecode, address, NadaType::SecretInteger, my_array_1_address + index + 1)?;
        address += 1;
        assert_get(&bytecode, address, NadaType::SecretInteger, my_array_2_address + index + 1)?;
        address += 1;
    }

    // new array -> 1 new + size x (1 get)
    let inner_array_type = NadaType::Array { inner_type: Box::new(inner_tuple_type.clone()), size };
    assert_new(&bytecode, address, inner_array_type)?;
    address += 1;
    for index in 0..size {
        assert_get(&bytecode, address, inner_tuple_type.clone(), frist_inner_tuple_address + index * 3)?;
        address += 1;
    }

    // first inner array
    assert_new(&bytecode, address, input_type.clone())?;
    let first_inner_array_address = address;
    address += 1;
    for index in 0..size {
        assert_get(&bytecode, address, NadaType::SecretInteger, my_array_1_address + index + 1)?;
        address += 1;
    }

    // second inner array
    assert_new(&bytecode, address, input_type.clone())?;
    let second_inner_array_address = address;
    address += 1;
    for index in 0..size {
        assert_get(&bytecode, address, NadaType::SecretInteger, my_array_2_address + index + 1)?;
        address += 1;
    }

    // resultant tuple
    let result_type =
        NadaType::Tuple { left_type: Box::new(input_type.clone()), right_type: Box::new(input_type.clone()) };
    assert_new(&bytecode, address, result_type.clone())?;
    let output_address = address;
    address += 1;
    assert_get(&bytecode, address, input_type.clone(), first_inner_array_address)?;
    address += 1;
    assert_get(&bytecode, address, input_type, second_inner_array_address)?;
    address += 1;

    assert_eq!(bytecode.operations_count(), address);
    assert_outputs(&bytecode, vec![(0, "my_output", result_type, output_address)]);
    Ok(())
}

#[test]
fn array_inner_product() -> Result<(), Error> {
    let mir = PROGRAMS.mir("array_inner_product")?;
    let bytecode = MIR2Bytecode::transform(&mir)?;
    assert_eq!(bytecode.operations_count(), (3 + 1) * 2 + 1);
    Ok(())
}

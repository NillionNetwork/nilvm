use crate::generator::generate_program;
use anyhow::{anyhow, Context, Result};
use client_fixture::compute::{ClientsMode, ComputeValidator};
use mir_model::proto::{ConvertProto, Message};
use mpc_vm::{protocols::MPCProtocol, JitCompiler, MPCCompiler, Program, ProgramBytecode};
use nillion_client::vm::VmClient;
use nodes_fixtures::{
    nodes::{nodes, Nodes},
    programs::PROGRAMS,
};
use pynadac::Compiler;
use rstest::rstest;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::tempdir;
use tracing_fixture::{tracing, Tracing};

async fn test_nada_lang_feature(nodes: &Arc<Nodes>, type_names: Vec<&str>, template_id: &'static str) -> Result<()> {
    let mut context = tera::Context::new();
    let program_id = match type_names.len() {
        1 => {
            let type_name = type_names[0];
            context.insert("type", type_name);
            let program_id = format!("{template_id}_{type_name}");
            program_id
        }
        2 => {
            let type_name1 = type_names[0];
            let type_name2 = type_names[1];
            context.insert("type1", type_name1);
            context.insert("type2", type_name2);

            let program_id = format!("{template_id}_{type_name1}{type_name2}");
            program_id
        }
        len @ _ => return Err(anyhow!("Unsupported number of inputs: {len}")),
    };
    let path = generate_program(template_id, &program_id, &context)?;
    run_end_to_end_test(nodes, InputProgram::Filesystem { path }).await
}

/// Helper function to test templates where there is only on type of inputs.
///
/// In other words, there might be many inputs but all of them have the same type.
async fn test_one_type(nodes: &Arc<Nodes>, type_name: &str, template_id: &'static str) -> Result<()> {
    test_nada_lang_feature(nodes, vec![type_name], template_id).await
}

/// Helper function to test templates where there are two types of inputs.
///
/// In other words, there might be many inputs in the program but all of them are of any of the two types.
async fn test_two_types(nodes: &Arc<Nodes>, type_name1: &str, type_name2: &str, template_id: &'static str) -> Result<()> {
    test_nada_lang_feature(nodes, vec![type_name1, type_name2], template_id).await
}

#[rstest]
// SecretInteger tests
#[case::integer_addition("SecretInteger", "addition")]
#[case::integer_subtraction("SecretInteger", "subtraction")]
#[case::integer_multiplication("SecretInteger", "multiplication")]
#[case::integer_multiplication_of_additions("SecretInteger", "multiplication_of_additions")]
#[case::integer_multiplication_of_subtractions("SecretInteger", "multiplication_of_subtractions")]
#[case::integer_reuse_in_addition("SecretInteger", "reuse_in_addition")]
#[case::integer_reuse_in_subtraction("SecretInteger", "reuse_in_subtraction")]
#[case::integer_reuse_in_addition_of_multiplications("SecretInteger", "reuse_in_addition_of_multiplications")]
#[case::integer_reuse_in_subtraction_of_multiplications("SecretInteger", "reuse_in_subtraction_of_multiplications")]
#[case::integer_reuse_in_multiplication("SecretInteger", "reuse_in_multiplication")]
#[case::integer_reuse_in_multiplication_of_additions("SecretInteger", "reuse_in_multiplication_of_additions")]
#[case::integer_reuse_in_multiplication_of_subtractions("SecretInteger", "reuse_in_multiplication_of_subtractions")]
#[case::integer_complex("SecretInteger", "complex")]
#[case::integer_complex_add_sub_mult("SecretInteger", "complex_add_sub_mult")]
#[case::integer_shift_left("SecretInteger", "shift_left")]
#[case::integer_shift_right("SecretInteger", "shift_right")]
#[case::integer_shift_left_after_add("SecretInteger", "shift_left_after_add")]
#[case::integer_shift_right_after_add("SecretInteger", "shift_right_after_add")]
// #[case::integer_trunc_pr("SecretInteger", "trunc_pr")] // The
// test is correct but due to its probabilistic nature it might fail since the
// result doesn't always match the deterministic truncation.
// #[case::integer_random("SecretInteger", "random_value")] <- We need to figure out how to test this because both the simulator and the bytecode evaluator will generate different randoms independently
#[tokio::test(flavor = "multi_thread")]
async fn test_one_type_integers(
    nodes: &Arc<Nodes>,
    _tracing: &Tracing,
    #[case] type_name: &str,
    #[case] template_id: &'static str,
) -> Result<()> {
    test_one_type(nodes, type_name, template_id).await
}

#[rstest]
#[case::unsigned_integer_addition("SecretUnsignedInteger", "addition")]
#[case::unsigned_integer_subtraction("SecretUnsignedInteger", "subtraction")]
#[case::unsigned_integer_multiplication("SecretUnsignedInteger", "multiplication")]
#[case::unsigned_integer_multiplication_of_additions("SecretUnsignedInteger", "multiplication_of_additions")]
#[case::unsigned_integer_multiplication_of_subtractions("SecretUnsignedInteger", "multiplication_of_subtractions")]
#[case::unsigned_integer_reuse_in_addition("SecretUnsignedInteger", "reuse_in_addition")]
#[case::unsigned_integer_reuse_in_subtraction("SecretUnsignedInteger", "reuse_in_subtraction")]
#[case::unsigned_integer_reuse_in_addition_of_multiplications(
    "SecretUnsignedInteger",
    "reuse_in_addition_of_multiplications"
)]
#[case::unsigned_integer_reuse_in_subtraction_of_multiplications(
    "SecretUnsignedInteger",
    "reuse_in_subtraction_of_multiplications"
)]
#[case::unsigned_integer_reuse_in_multiplication("SecretUnsignedInteger", "reuse_in_multiplication")]
#[case::unsigned_integer_reuse_in_multiplication_of_additions(
    "SecretUnsignedInteger",
    "reuse_in_multiplication_of_additions"
)]
#[case::unsigned_integer_reuse_in_multiplication_of_subtractions(
    "SecretUnsignedInteger",
    "reuse_in_multiplication_of_subtractions"
)]
#[case::unsigned_integer_complex("SecretUnsignedInteger", "complex")]
#[case::unsigned_integer_complex_add_sub_mult("SecretUnsignedInteger", "complex_add_sub_mult")]
#[tokio::test(flavor = "multi_thread")]
async fn test_one_type_unsigned_integers(
    nodes: &Arc<Nodes>,
    _tracing: &Tracing,
    #[case] type_name: &str,
    #[case] template_id: &'static str,
) -> Result<()> {
    test_one_type(nodes, type_name, template_id).await
}

#[rstest]
// Integer
#[case::integer_less_than_public("PublicInteger", "SecretInteger", "less_than")]
#[case::integer_greater_than_public("PublicInteger", "SecretInteger", "greater_than")]
#[case::integer_less_or_equal_than_public("PublicInteger", "SecretInteger", "less_or_equal_than")]
#[case::integer_greater_or_equal_than_public("PublicInteger", "SecretInteger", "greater_or_equal_than")]
#[case::integer_less_than_public("SecretInteger", "PublicInteger", "less_than")]
#[case::integer_greater_than_public("SecretInteger", "PublicInteger", "greater_than")]
#[case::integer_less_or_equal_than_public("SecretInteger", "PublicInteger", "less_or_equal_than")]
#[case::integer_greater_or_equal_than_public("SecretInteger", "PublicInteger", "greater_or_equal_than")]
// The division fails, it should be reviewed when  the division has been fixed
// This is currently failing because the numbers are too big. If we decrease C it passes
#[case::integer_division_secret_public("SecretInteger", "PublicInteger", "division")]
#[case::integer_division_public_secret("PublicInteger", "SecretInteger", "division")]
#[case::integer_division_secret("SecretInteger", "SecretInteger", "division")]
#[case::integer_equals_secret_public("SecretInteger", "PublicInteger", "equals")]
#[case::integer_equals_secret_secret("SecretInteger", "SecretInteger", "equals")]
#[case::integer_equals_public_public("PublicInteger", "PublicInteger", "equals")]
#[case::integer_equals_boolean_resolution_if("SecretInteger", "PublicInteger", "equals_public_if")]
#[case::integer_equals_boolean_resolution_else("SecretInteger", "PublicInteger", "equals_public_else")]
#[case::integer_modulo_public_public("PublicInteger", "PublicInteger", "modulo")]
#[case::integer_modulo_secret_public("SecretInteger", "PublicInteger", "modulo")]
#[case::integer_modulo_public_secret("PublicInteger", "SecretInteger", "modulo")]
#[case::integer_modulo_secret_secret("SecretInteger", "SecretInteger", "modulo")]
#[case::integer_if_else("SecretInteger", "PublicInteger", "if_else")]
#[case::integer_public_output_equality_public("SecretInteger", "PublicInteger", "public_output_equality")]
#[tokio::test(flavor = "multi_thread")]
async fn test_two_types_integer(
    nodes: &Arc<Nodes>,
    _tracing: &Tracing,
    #[case] type_name1: &str,
    #[case] type_name2: &str,
    #[case] template_id: &'static str,
) -> Result<()> {
    test_two_types(nodes, type_name1, type_name2, template_id).await
}

#[rstest]
// UnsignedInteger
#[case::unsigned_integer_less_than_public("PublicUnsignedInteger", "SecretUnsignedInteger", "less_than")]
#[case::unsigned_integer_greater_than_public("PublicUnsignedInteger", "SecretUnsignedInteger", "greater_than")]
#[case::unsigned_integer_less_or_equal_than_public(
    "PublicUnsignedInteger",
    "SecretUnsignedInteger",
    "less_or_equal_than"
)]
#[case::unsigned_integer_greater_or_equal_than_public(
    "PublicUnsignedInteger",
    "SecretUnsignedInteger",
    "greater_or_equal_than"
)]
#[case::unsigned_integer_less_than_public("SecretUnsignedInteger", "PublicUnsignedInteger", "less_than")]
#[case::unsigned_integer_greater_than_public("SecretUnsignedInteger", "PublicUnsignedInteger", "greater_than")]
#[case::unsigned_integer_less_or_equal_than_public(
    "SecretUnsignedInteger",
    "PublicUnsignedInteger",
    "less_or_equal_than"
)]
#[case::unsigned_integer_greater_or_equal_than_public(
    "SecretUnsignedInteger",
    "PublicUnsignedInteger",
    "greater_or_equal_than"
)]
#[case::unsigned_integer_division_public("SecretUnsignedInteger", "PublicUnsignedInteger", "division")]
#[case::unsigned_integer_modulo_public_public("PublicUnsignedInteger", "PublicUnsignedInteger", "modulo")]
#[case::unsigned_integer_modulo_secret_public("SecretUnsignedInteger", "PublicUnsignedInteger", "modulo")]
#[case::unsigned_integer_modulo_public_secret("PublicUnsignedInteger", "SecretUnsignedInteger", "modulo")]
#[case::unsigned_integer_modulo_secret_secret("SecretUnsignedInteger", "SecretUnsignedInteger", "modulo")]
#[case::unsigned_integer_public_output_equality_public(
    "SecretUnsignedInteger",
    "PublicUnsignedInteger",
    "public_output_equality"
)]
#[tokio::test(flavor = "multi_thread")]
async fn test_two_types_unsigned_integer(
    nodes: &Arc<Nodes>,
    _tracing: &Tracing,
    #[case] type_name1: &str,
    #[case] type_name2: &str,
    #[case] template_id: &'static str,
) -> Result<()> {
    test_two_types(nodes, type_name1, type_name2, template_id).await
}

#[rstest]
#[case::import_library("programs/import.py")]
#[case::modulo_unsigned_integer("programs/modulo-simple-integer.py")]
#[case::division_unsigned_integer("programs/division-simple-integer.py")]
#[case::power_public_integer("programs/power-public-integer.py")]
// IGNORED: secrets are not yet supported for power.
// #[case::power_secret_integer("SecretInteger-power.inputs.yaml", Some("programs/power-secret-integer.py"))]
#[case::literals("programs/literals.py")]
#[tokio::test(flavor = "multi_thread")]
async fn test_individual_program(nodes: &Arc<Nodes>, _tracing: &Tracing, #[case] program_path: &str) -> Result<()> {
    let input_program = InputProgram::Filesystem { path: program_path.into() };
    run_end_to_end_test(nodes, input_program).await
}

#[rstest]
#[case::public_variables("public-variables")]
#[case::array_simple("array_simple_shares")]
#[case::array_new("array_new")]
#[case::tuple_new("tuple_new")]
#[case::map_simple("map_simple")]
#[case::reduce_simple("reduce_simple")]
#[case::reduce_simple_mul("reduce_simple_mul")]
#[case::array_chaining_map_map("array_chaining_map_map")]
#[case::array_chaining_map_reduce("array_chaining_map_reduce")]
#[tokio::test(flavor = "multi_thread")]
async fn test_individual_pre_uploaded_program(
    nodes: &Arc<Nodes>,
    _tracing: &Tracing,
    #[case] program_name: &str,
) -> Result<()> {
    let (program, bytecode) = PROGRAMS.program(program_name)?;
    let input_program =
        InputProgram::Preuploaded { program_id: nodes.uploaded_programs.program_id(program_name), program, bytecode };
    run_end_to_end_test(nodes, input_program).await
}

/// Compiles and stores the program in the network.
///
/// Modifies the [`TestDescriptor`], to store the program and the program identifier.
async fn compile_and_store_program(
    client: &VmClient,
    program_path: PathBuf,
) -> Result<(String, Program<MPCProtocol>, ProgramBytecode)> {
    let program_name = program_path.file_name().unwrap().to_str().unwrap().to_string();
    let temp_dir = tempdir()?;
    let compiler = Compiler::new(temp_dir.into_path());

    let mir = compiler
        .compile(program_path.to_str().unwrap())
        .with_context(|| format!("compilation of {program_path:?} failed"))?
        .mir;
    let raw_mir = mir.clone().into_proto().encode_to_vec();
    let program_id = client.store_program().name(program_name).program(raw_mir).build()?.invoke().await?;
    let (program, bytecode) = MPCCompiler::compile_with_bytecode(mir)?;
    Ok((program_id, program, bytecode))
}

/// Runs end to end tests
///
/// # Arguments
/// * `nodes` - The [`Nodes`] fixture injected by rstest
/// * `program_path` - The program to be tested.
async fn run_end_to_end_test(nodes: &Arc<Nodes>, input_program: InputProgram) -> Result<()> {
    let (program_id, program, bytecode, client) = match input_program {
        InputProgram::Preuploaded { program_id, program, bytecode } => (program_id, program, bytecode, None),
        InputProgram::Filesystem { path } => {
            let client = nodes.build_client().await;
            let (program_id, program, bytecode) = compile_and_store_program(&client, path).await?;
            (program_id, program, bytecode, Some(client))
        }
    };

    let mut builder = ComputeValidator::builder()
        .program_id(program_id)
        .program(program, bytecode)
        .clients_mode(ClientsMode::Single)
        .randomized_seed();
    // reuse the client we use to upload the program to save us from another nilchain transaction
    if let Some(client) = client {
        builder = builder.invoker_client(client);
    }

    builder.run(nodes).await;
    Ok(())
}

enum InputProgram {
    // A program that's pre-uploaded in the network by the `nodes` fixture.
    Preuploaded { program_id: String, program: Program<MPCProtocol>, bytecode: ProgramBytecode },

    // A program that needs to be read from the filesystem and uploaded.
    Filesystem { path: PathBuf },
}

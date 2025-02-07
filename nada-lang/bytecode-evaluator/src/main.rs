use anyhow::Error;
use bytecode_evaluator::EvaluatorRunner;
use clap::Parser;
use jit_compiler::models::bytecode::ProgramBytecode;
use log::info;
use math_lib::modular::EncodedModulo;
use nada_value::{clear::Clear, NadaValue};
use serde_files_utils::json::read_json;
use std::collections::HashMap;

#[derive(Parser, Debug)]
#[clap()]
struct Args {
    /// Program path
    #[clap(short, long)]
    program_path: String,
    /// Values file path
    #[clap(short, long, default_value = "../tests/resources/values/default.json")]
    values_file_path: String,
    /// Prime size in bits
    #[clap(long, default_value = "64")]
    prime_size: u32,
}

fn main() -> Result<(), Error> {
    env_logger::init();
    let Args { program_path, values_file_path: variables_file_path, prime_size } = Args::parse();
    let bytecode: ProgramBytecode = read_json(program_path)?;
    let values: HashMap<String, NadaValue<Clear>> = read_json(variables_file_path)?;

    let modulo = EncodedModulo::try_safe_prime_from_bits(prime_size)?;
    let runner = Box::<dyn EvaluatorRunner>::try_from(&modulo)?;
    let outputs = runner.run(&bytecode, values)?;

    for (key, value) in outputs {
        info!("[{key}] = {value:?}");
    }

    Ok(())
}

mod args;
mod io;

use crate::{
    args::{args, Args},
    io::{get_file_name, get_input_file_paths, write_binary_file, write_json_file},
};
use anyhow::{anyhow, Result};
use log::{error, info};
use mpc_vm::{
    JitCompiler, MIR2Bytecode, MPCCompiler, BYTECODE_FILE_EXTENSION_BIN, BYTECODE_FILE_EXTENSION_JSON,
    PROTOCOLS_BODY_FILE_EXTENSION_BIN, PROTOCOLS_BODY_FILE_EXTENSION_JSON,
};
use nada_compiler_backend::mir::{proto::ConvertProto, ProgramMIR, MIR_FILE_EXTENSION_BIN};
use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

fn read_program(path: &PathBuf) -> Result<ProgramMIR> {
    let mut program = vec![];
    File::open(path)
        .map_err(|e| anyhow!("failed to open program's MIR file: {e}"))?
        .read_to_end(&mut program)
        .map_err(|e| anyhow!("failed to read program's MIR file: {e}"))?;
    ProgramMIR::try_decode(&program).map_err(|e| anyhow!("failed to parse program's MIR: {e}"))
}
fn main() -> Result<()> {
    env_logger::init();
    let Args { directory_path, file_name, json_format } = args();

    match &file_name {
        Some(file) if !file.ends_with(MIR_FILE_EXTENSION_BIN) => {
            return Err(anyhow!("{} must be bytecode model in binary format", &file_name.unwrap_or_default()));
        }
        _ => {}
    }

    let mir_file_paths = get_input_file_paths(&directory_path, file_name, MIR_FILE_EXTENSION_BIN)?;
    let target_dir_path = Path::new(&directory_path);

    for mir_file_path in mir_file_paths {
        let file_name = get_file_name(&mir_file_path, &directory_path, MIR_FILE_EXTENSION_BIN)?;
        info!("Loading {file_name}...");
        let program_mir = match read_program(&mir_file_path) {
            Ok(program_mir) => program_mir,
            Err(error) => {
                error!("Error reading program: {error}");
                continue;
            }
        };

        info!("Compiling {file_name}...");
        let bytecode = match MIR2Bytecode::transform(&program_mir) {
            Ok(bytecode) => bytecode,
            Err(error) => {
                error!("Failed to compile program bytecode {file_name}: {error}");
                continue;
            }
        };
        write_binary_file(&bytecode, target_dir_path, &file_name, BYTECODE_FILE_EXTENSION_BIN);
        if json_format {
            write_json_file(&bytecode, target_dir_path, &file_name, BYTECODE_FILE_EXTENSION_JSON);
        }

        let program = match MPCCompiler::compile(program_mir.clone()) {
            Ok(program) => program,
            Err(error) => {
                error!("Failed to compile program {file_name}: {error}");
                continue;
            }
        };
        write_binary_file(&program, target_dir_path, &file_name, PROTOCOLS_BODY_FILE_EXTENSION_BIN);
        if json_format {
            write_json_file(&program, target_dir_path, &file_name, PROTOCOLS_BODY_FILE_EXTENSION_JSON);
        }
    }
    Ok(())
}

mod args;
mod io;

use crate::{
    args::{args, Args},
    io::{get_file_name, get_input_file_paths, write_binary_file, write_json_file},
};
use anyhow::Result;
use log::{error, info};
use mpc_vm::{MIR2Bytecode, BYTECODE_FILE_EXTENSION_BIN, BYTECODE_FILE_EXTENSION_JSON};
use nada_compiler_backend::mir::{ProgramMIR, MIR_FILE_EXTENSION_JSON};
use serde_files_utils::json::read_json;
use std::path::Path;

fn main() -> Result<()> {
    env_logger::init();
    let Args { directory_path, file_name, json_format } = args();

    let mir_file_paths = get_input_file_paths(&directory_path, file_name, MIR_FILE_EXTENSION_JSON)?;
    let target_dir_path = Path::new(&directory_path);

    for mir_file_path in mir_file_paths {
        let file_name = get_file_name(&mir_file_path, &directory_path, MIR_FILE_EXTENSION_JSON)?;
        info!("Loading {file_name}...");
        let mir = match read_json::<_, ProgramMIR>(&mir_file_path) {
            Ok(mir) => mir,
            Err(error) => {
                error!("Error reading program MIR: {error}");
                continue;
            }
        };

        info!("Compiling {file_name}...");
        let bytecode = match MIR2Bytecode::transform(&mir) {
            Ok(bytecode) => bytecode,
            Err(error) => {
                error!("{error}");
                continue;
            }
        };
        write_binary_file(&bytecode, target_dir_path, &file_name, BYTECODE_FILE_EXTENSION_BIN);
        if json_format {
            write_json_file(&bytecode, target_dir_path, &file_name, BYTECODE_FILE_EXTENSION_JSON);
        }
    }
    Ok(())
}

use crate::{error::IntoEyre, nada_project_toml::ProgramConf, paths::get_target_path};
use eyre::{eyre, Result};
use pynadac::{Compiler, CompilerOptions, PersistOptions};
use std::fs::create_dir_all;

pub fn build_program(program_conf: &ProgramConf, mir_json: bool) -> Result<()> {
    let options = CompilerOptions {
        persist: PersistOptions {
            // We always want the bin mir as that's our true output.
            mir_bin: true,
            mir_json,
        },
    };
    let target_dir = get_target_path()?;
    // Let's make sure that the target dir exists. Otherwise the user might see a strange error later.
    create_dir_all(target_dir.clone())?;
    let compiler = Compiler::with_options(target_dir, options);

    let compiler_output =
        compiler.compile_with_name(&program_conf.path.to_string_lossy(), &program_conf.name).into_eyre()?;
    let validation_result = compiler_output.validation_result;
    if !validation_result.is_successful() {
        validation_result.print(&compiler_output.mir).into_eyre()?;
        return Err(eyre!("MIR validation failed"));
    }
    Ok(())
}

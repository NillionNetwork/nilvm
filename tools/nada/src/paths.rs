use eyre::eyre;
use file_find::find_file_with_parents;
use std::path::PathBuf;

use crate::nada_project_toml::ProgramConf;

pub fn get_nada_project_toml_path() -> eyre::Result<PathBuf> {
    find_file_with_parents("nada-project.toml").map_err(|e| eyre!("Error finding nada-project.toml: {}", e))
}

pub fn get_nada_project_root() -> eyre::Result<PathBuf> {
    let nada_project_toml_path = get_nada_project_toml_path()?;
    nada_project_toml_path.parent().map(|p| p.to_path_buf()).ok_or_else(|| eyre!("Error getting parent dir"))
}

pub fn get_tests_path() -> eyre::Result<PathBuf> {
    let project_root = get_nada_project_root()?;
    Ok(project_root.join("tests"))
}

pub fn get_target_path() -> eyre::Result<PathBuf> {
    let project_root = get_nada_project_root()?;
    Ok(project_root.join("target"))
}

/// Returns the path of a program
pub fn get_compiled_program_path(program_conf: &ProgramConf) -> eyre::Result<PathBuf> {
    let target_path = get_target_path()?;
    Ok(target_path.join(format!("{}.nada.bin", program_conf.name)))
}

use anyhow::{anyhow, Result};
use log::{error, info};
use serde::Serialize;
use serde_files_utils::{binary::write_bin, json::write_json};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub fn get_input_file_paths(
    directory_path: &String,
    file_name: Option<String>,
    file_extension: &str,
) -> Result<Vec<PathBuf>> {
    let directory_path = Path::new(directory_path);
    if !directory_path.is_dir() {
        Err(anyhow!("{directory_path:?} is not a directory"))?;
    }
    let mut paths = Vec::new();
    if let Some(file_name) = file_name {
        let file_path = directory_path.join(file_name);
        paths.push(file_path);
    } else {
        let directory_content = fs::read_dir(directory_path).unwrap();
        for file in directory_content {
            let file_path = file?.path();
            if file_path.display().to_string().ends_with(file_extension) {
                paths.push(file_path);
            }
        }
    }
    Ok(paths)
}

pub fn get_file_name(path: &Path, base_path: &str, extension: &str) -> Result<String> {
    Ok(path
        .file_name()
        .ok_or_else(|| anyhow!("file name not found"))?
        .to_str()
        .ok_or_else(|| anyhow!("file name not found"))?
        .replace(base_path, "")
        .replace(extension, ""))
}

pub fn write_binary_file<T: Serialize>(model: &T, base_path: &Path, file_name: &str, extension: &str) {
    let file_path = build_file_path(base_path, file_name, extension);
    let binding = file_path.clone();
    let file_path_str = binding.to_str().unwrap();
    match write_bin(file_path, model) {
        Ok(_) => info!("{file_path_str} has been created successfully"),
        Err(err) => error!("{file_name} cannot be created: {err}"),
    };
}

pub fn write_json_file<T: Serialize>(model: &T, base_path: &Path, file_name: &str, extension: &str) {
    let file_path = build_file_path(base_path, file_name, extension);
    match write_json(file_path, model) {
        Ok(_) => info!("{file_name} has been created successful"),
        Err(err) => error!("{file_name} cannot be created: {err}"),
    };
}

fn build_file_path(base_path: &Path, file_name: &str, extension: &str) -> PathBuf {
    let file_name = format!("{file_name}{extension}");
    base_path.join(file_name)
}

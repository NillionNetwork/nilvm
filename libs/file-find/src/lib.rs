use anyhow::{anyhow, Context, Result};
use path_absolutize::Absolutize;
use std::{
    env::current_dir,
    fs,
    path::{Path, PathBuf},
};

/// find a file with the given name in the current directory or any of its parents
pub fn find_file_with_parents(file_name: &str) -> Result<PathBuf> {
    let mut current_path = current_dir()?;
    while current_path != Path::new("/") {
        let path = current_path.join(file_name);
        if let Ok(metadata) = fs::metadata(&path) {
            if metadata.is_file() {
                return Ok(path);
            }
        }
        current_path = current_path
            .join("..")
            .absolutize()
            .context(format!("Error expanding path {}", current_path.to_string_lossy()))?
            .to_path_buf()
    }

    Err(anyhow!("File {} not found", file_name))
}

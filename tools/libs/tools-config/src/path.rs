use std::{
    env,
    path::{Path, PathBuf},
};

/// Gets the path the directory where configs are stored.
pub fn config_directory() -> Option<PathBuf> {
    let config_root = if let Ok(config_path) = env::var("XDG_CONFIG_HOME") {
        config_path.into()
    } else {
        let home = env::var("HOME").ok()?;
        Path::new(&home).join(".config")
    };
    Some(config_root.join("nillion"))
}

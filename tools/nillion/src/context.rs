use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::BufReader,
    path::PathBuf,
};
use tools_config::path::config_directory;
use tracing::error;

#[derive(Serialize, Deserialize)]
pub struct ContextConfig {
    pub identity: String,
    pub network: String,
}

impl ContextConfig {
    fn config_path() -> Option<PathBuf> {
        Some(config_directory()?.join("cli.yaml"))
    }

    pub fn load() -> Option<Self> {
        let path = Self::config_path()?;
        let file = File::open(path).ok()?;
        let reader = BufReader::new(file);
        match serde_yaml::from_reader(reader) {
            Ok(config) => Some(config),
            Err(e) => {
                error!("Invalid cli config: {e}");
                None
            }
        }
    }

    pub fn store(&self) -> anyhow::Result<()> {
        let serialized = serde_yaml::to_string(&self)?;
        let path = Self::config_path().ok_or_else(|| anyhow!("no config path found"))?;
        fs::write(path, serialized)?;
        Ok(())
    }
}

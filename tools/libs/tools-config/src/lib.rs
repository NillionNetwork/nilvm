use anyhow::{anyhow, bail, Context, Result};
use serde::{de::DeserializeOwned, Serialize};
use std::{
    ffi::OsStr,
    fs::{self, create_dir_all, File},
    io::Write,
    path::PathBuf,
};

pub mod identities;
pub mod networks;
pub mod path;

#[cfg(feature = "client")]
pub mod client;

const INVALID_CONFIG_CHARS: &[char] = &['/', '.'];

/// Tool configuration.
///
/// Defines standard behaviour of configurations:
/// - serialising to file
/// - reading from file
/// - location path
pub trait ToolConfig {
    /// Serialise the network configuration
    fn write_to_file(&self, name: &str) -> Result<()>
    where
        Self: Serialize,
    {
        let serialized = serde_yaml::to_string(&self)?;
        let config_path = Self::config_path(name)?;
        if let Some(parent) = config_path.parent() {
            create_dir_all(parent)?;
        }
        let mut file = File::create(config_path.clone()).context(format!("{:?}", config_path))?;
        file.write_all(serialized.as_bytes())?;
        Ok(())
    }

    /// Reads the identities from the configuration file
    fn read_from_config(name: &str) -> Result<Self>
    where
        Self: Sized + DeserializeOwned,
    {
        let config_path = Self::config_path(name)?;
        if config_path.exists() {
            let file = File::open(config_path)?;
            let result: Self = serde_yaml::from_reader(file)?;

            Ok(result)
        } else {
            Err(anyhow!("configuration '{name}' not found"))
        }
    }

    fn read_all() -> Result<Vec<NamedConfig<Self>>>
    where
        Self: Sized + DeserializeOwned,
    {
        let dir = Self::root_config_path();
        let mut configs = Vec::new();
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let is_file = entry.file_type()?.is_file();
            if is_file && path.extension() == Some(OsStr::new("yaml")) {
                let name = path
                    .file_stem()
                    .expect("no file")
                    .to_str()
                    .ok_or_else(|| anyhow!("invalid file name found: {path:?}"))?;
                let config = Self::read_from_config(name)?;
                configs.push(NamedConfig { name: name.to_string(), config });
            }
        }
        Ok(configs)
    }

    fn remove_config(name: &str) -> anyhow::Result<()> {
        let path = Self::config_path(name)?;
        fs::remove_file(path)?;
        Ok(())
    }

    /// Get the root config path for this configuration.
    fn root_config_path() -> PathBuf;

    /// Get the file path for the identities file
    fn config_path(name: &str) -> anyhow::Result<PathBuf> {
        if is_valid_config_name(name) {
            Ok(Self::root_config_path().join(format!("{name}.yaml")))
        } else {
            bail!("name cannot contain any of {INVALID_CONFIG_CHARS:?}");
        }
    }
}

fn is_valid_config_name(name: &str) -> bool {
    !name.chars().any(|c| INVALID_CONFIG_CHARS.contains(&c))
}

/// A named configuration.
pub struct NamedConfig<C> {
    /// The name of the configuration.
    pub name: String,

    /// The configuration itself.
    pub config: C,
}

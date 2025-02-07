//! This crate implements the I/O operations for yaml files

use crate::string::{read_string, write_string};
use anyhow::Error;
use serde::{de::DeserializeOwned, Serialize};
use std::path::Path;

/// Read data from yaml file
pub fn read_yaml<P: AsRef<Path>, T: DeserializeOwned>(path: P) -> Result<T, Error> {
    let file_content = read_string(path)?;
    Ok(serde_yaml::from_str(&file_content)?)
}

/// Write data into a yaml file
pub fn write_yaml<P: AsRef<Path>, T: Serialize>(path: P, content: &T) -> Result<(), Error> {
    let content: String = serde_yaml::to_string(content)?;
    write_string(path, content)
}

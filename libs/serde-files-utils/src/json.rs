//! This crate implements the I/O operations for json files

use crate::string::{read_string, write_string};
use anyhow::Error;
use serde::{de::DeserializeOwned, Serialize};
use std::path::Path;

/// Read data from json file
pub fn read_json<P: AsRef<Path>, T: DeserializeOwned>(path: P) -> Result<T, Error> {
    let file_content = read_string(path)?;
    Ok(serde_json::from_str(&file_content)?)
}

/// Write data into a json file
pub fn write_json<P: AsRef<Path>, T: Serialize>(path: P, content: &T) -> Result<(), Error> {
    let content: String = serde_json::to_string(content)?;
    write_string(path, content)
}

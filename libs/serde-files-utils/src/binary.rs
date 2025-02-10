//! This crate implements the I/O operations for binary files

use anyhow::{Context, Error};
use serde::{de::DeserializeOwned, Serialize};
use std::{
    fs::File,
    io::{Read, Write},
    path::Path,
};

/// Read data from a binary file
pub fn read_bin<P: AsRef<Path>, T: DeserializeOwned>(path: P) -> Result<T, Error> {
    let mut file = File::open(path)?;
    let mut file_content = vec![];
    file.read_to_end(&mut file_content)?;
    bincode::deserialize(&file_content).context("bincode::deserialize")
}

/// Read data from a binary file
pub fn write_bin<P: AsRef<Path>, T: Serialize>(path: P, content: T) -> Result<(), Error> {
    let file_content = bincode::serialize(&content).context("bincode::serialize")?;
    let mut file = File::create(path)?;
    file.write_all(&file_content)?;
    Ok(())
}

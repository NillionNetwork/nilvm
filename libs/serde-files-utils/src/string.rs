//! This crate implements the I/O operations for text files

use anyhow::Error;
use std::{
    fs::File,
    io::{Read, Write},
    path::Path,
};

/// Read data from a text file
pub fn read_string<P: AsRef<Path>>(path: P) -> Result<String, Error> {
    let mut code_file = File::open(path)?;
    let mut code = String::new();
    code_file.read_to_string(&mut code)?;
    Ok(code)
}

/// Write data into a text file
pub fn write_string<P: AsRef<Path>>(path: P, content: String) -> Result<(), Error> {
    let mut text_file = File::create(path)?;
    text_file.write_all(content.as_bytes())?;
    Ok(())
}

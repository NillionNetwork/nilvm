use anyhow::{bail, Context};
use serde::de::DeserializeOwned;
use std::{fs::File, io::BufReader, path::Path};

pub mod args;
pub mod config;
pub mod context;
pub mod runner;
pub mod serialize;
pub(crate) mod wrappers;

pub(crate) fn parse_input_file<T>(path: &Path) -> anyhow::Result<T>
where
    T: DeserializeOwned,
{
    let file = File::open(path)?;
    let reader = BufReader::new(file);

    let extension = path.extension().and_then(|ext| ext.to_str()).map(|ext| ext.to_lowercase());

    match extension.as_deref() {
        Some("yaml") | Some("yml") => serde_yaml::from_reader(reader).context("failed to parse YAML file"),
        Some("json") => serde_json::from_reader(reader).context("failed to parse JSON file"),
        _ => bail!("invalid file extension: supported extensions are 'yaml', 'yml', or 'json'"),
    }
}

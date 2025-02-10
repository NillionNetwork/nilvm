//! File representation for secrets/public variables.

use crate::{named::Named, parse::Parse};
use nada_value::{clear::Clear, NadaType, NadaValue};
use serde::Deserialize;
use std::{collections::HashMap, fs::File, io::BufReader, iter};

/// A set of inputs defined in a file.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Inputs {
    /// Integer.
    #[serde(default)]
    pub integers: HashMap<String, String>,

    /// Unsigned integer.
    #[serde(default)]
    pub unsigned_integers: HashMap<String, String>,

    /// Blob secrets.
    #[serde(default)]
    pub blobs: HashMap<String, String>,

    /// Integer secrets.
    #[serde(default)]
    pub secret_integers: HashMap<String, String>,

    /// Unsigned integer secrets.
    #[serde(default)]
    pub secret_unsigned_integers: HashMap<String, String>,
}

impl Inputs {
    /// Load the inputs from a path.
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let file = File::open(path)?;
        let inputs: Inputs = serde_yaml::from_reader(BufReader::new(file))?;
        Ok(inputs)
    }

    /// Parse the defined values.
    pub fn parse_values(&self) -> anyhow::Result<impl Iterator<Item = Named<NadaValue<Clear>>>> {
        let values = iter::empty()
            .chain(NadaType::Integer.parse_named_all(self.integers.iter())?)
            .chain(NadaType::UnsignedInteger.parse_named_all(self.unsigned_integers.iter())?)
            .chain(NadaType::SecretBlob.parse_named_all(self.blobs.iter())?)
            .chain(NadaType::SecretInteger.parse_named_all(self.secret_integers.iter())?)
            .chain(NadaType::SecretUnsignedInteger.parse_named_all(self.secret_unsigned_integers.iter())?);

        Ok(values)
    }
}

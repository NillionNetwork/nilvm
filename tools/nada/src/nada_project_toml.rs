use crate::paths;
use eyre::{eyre, Context, Result};
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    str::FromStr,
};

/// Convenient enumeration with all the Nillion supported prime sizes
#[derive(Serialize_repr, Deserialize_repr, PartialEq, Debug, Clone, Default)]
#[repr(u32)]
pub enum PrimeSize {
    Small64bit = 64,
    #[default]
    Medium128bit = 128,
    Large256bit = 256,
}

impl PrimeSize {
    pub fn value(&self) -> u32 {
        use PrimeSize::*;
        match self {
            Small64bit => 64,
            Medium128bit => 128,
            Large256bit => 256,
        }
    }
}

impl FromStr for PrimeSize {
    type Err = eyre::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "64" => Ok(PrimeSize::Small64bit),
            "128" => Ok(PrimeSize::Medium128bit),
            "256" => Ok(PrimeSize::Large256bit),
            _ => Err(eyre!("Invalid value for prime size, valid values are: 64,128,256.")),
        }
    }
}

#[derive(Deserialize, Serialize, Debug, PartialEq)]
pub struct ProgramToml {
    pub name: Option<String>,
    pub path: String,
    pub prime_size: PrimeSize,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct NadaProjectToml {
    pub name: String,
    pub version: String,
    pub authors: Vec<String>,
    pub programs: Vec<ProgramToml>,
    #[serde(skip_serializing_if = "HashMap::is_empty", default = "HashMap::new")]
    pub test_framework: HashMap<String, TestFramework>,
    #[serde(skip_serializing_if = "HashMap::is_empty", default = "HashMap::new")]
    pub networks: HashMap<String, NetworkConf>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct TestFramework {
    pub command: String,
}

#[derive(Clone, Hash, Eq, PartialEq, Debug)]
pub struct ProgramConf {
    pub name: String,
    pub path: PathBuf,
    pub prime_size: u32,
}

/// The network configuration as defined in the project TOML
#[derive(Deserialize, Serialize, Debug, PartialEq)]
pub struct NetworkConf {
    /// Identities file
    pub identity: String,
}

impl NadaProjectToml {
    pub fn get_programs(&self) -> Result<HashMap<String, ProgramConf>> {
        self.programs
            .iter()
            .map(|p| {
                let path = Path::new(&p.path);
                let file_name =
                    path.file_stem().ok_or_else(|| eyre!("Error getting file stem"))?.to_string_lossy().to_string();
                let program_name = p.name.clone().unwrap_or(file_name);

                let program_conf = ProgramConf {
                    name: program_name.clone(),
                    path: path.to_path_buf(),
                    prime_size: p.prime_size.value(),
                };

                Ok((program_name, program_conf))
            })
            .collect()
    }
    pub(crate) fn find_self() -> Result<NadaProjectToml> {
        let nada_project_toml_path = paths::get_nada_project_toml_path()?;
        let nada_project_toml_str =
            std::fs::read_to_string(nada_project_toml_path).context("Reading nada-project.toml")?;
        let result = toml::from_str(&nada_project_toml_str).context("Deserializing nada-project.toml")?;
        Ok(result)
    }
}

#[cfg(test)]
mod test {

    use eyre::Error;

    use crate::nada_project_toml::{NetworkConf, PrimeSize, ProgramToml};

    use super::NadaProjectToml;

    #[test]
    fn parse_network_config() -> Result<(), Error> {
        let project_toml_string = r#"
name = "dot-product"
version = "0.1.0"
authors = [""]

[networks.local]
identity = "john"

[networks.testnet]
identity = "testnet-john"

[[programs]]
path = "src/main.py"
prime_size = 128
        "#;

        let project_toml: NadaProjectToml = toml::from_str(&project_toml_string)?;
        let local_network = NetworkConf { identity: "john".to_string() };
        let testnet_network = NetworkConf { identity: "testnet-john".to_string() };
        assert_eq!(Some(&local_network), project_toml.networks.get("local"));
        assert_eq!(Some(&testnet_network), project_toml.networks.get("testnet"));
        let program = ProgramToml { name: None, path: "src/main.py".to_string(), prime_size: PrimeSize::Medium128bit };
        assert_eq!(Some(program), project_toml.programs.into_iter().next());
        Ok(())
    }

    #[test]
    fn test_prime_size() {
        assert_eq!(64, PrimeSize::Small64bit as u32);
        assert_eq!(128, PrimeSize::Medium128bit as u32);
        assert_eq!(256, PrimeSize::Large256bit as u32);
    }
}

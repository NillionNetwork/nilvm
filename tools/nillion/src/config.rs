use crate::args::CommandOutputFormat;
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Clone, Debug, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub output: OutputConfig,
}

impl Config {
    pub fn new(path: PathBuf) -> Result<Self, config::ConfigError> {
        let source = config::File::from(path).format(config::FileFormat::Yaml);
        let config = config::Config::builder()
            .add_source(source)
            .add_source(config::Environment::default().separator("__"))
            .build()?;
        config.try_deserialize()
    }
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct OutputConfig {
    #[serde(default)]
    pub format: Option<CommandOutputFormat>,
}

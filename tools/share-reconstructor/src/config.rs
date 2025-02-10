use math_lib::modular::EncodedModulo;
use num_bigint::BigInt;
use serde::{de::DeserializeOwned, Deserialize};
use serde_with::{serde_as, DisplayFromStr};
use uuid::Uuid;

/// The main config type.
#[serde_as]
#[derive(Deserialize)]
pub struct Config {
    /// The prime number to be used. This is `P`.
    pub prime: EncodedModulo,

    /// The shares.
    pub shares: SharesConfig,
}

impl Config {
    /// Loads the config from a file path.
    pub fn load(path: &str) -> Result<Self, config::ConfigError> {
        let mut builder = config::Config::builder();
        builder = builder.add_source(config::File::with_name(path));
        builder.build()?.try_deserialize()
    }
}

/// The shares configuration.
#[derive(Deserialize)]
#[serde(tag = "type")]
pub enum SharesConfig {
    /// We want to reconstruct a share from a prime field.
    PrimeField { shares: Vec<PartyConfig<PrimeFieldConfig>> },

    /// We want to reconstruct a share from a semi-field (2q).
    SemiField { shares: Vec<PartyConfig<SemiFieldConfig>> },
}

/// The configuration for a particular party.
#[serde_as]
#[derive(Deserialize)]
#[serde(bound(deserialize = "T: DeserializeOwned"))]
pub struct PartyConfig<T> {
    /// The party id.
    pub party_id: Uuid,

    /// The share itself.
    #[serde(flatten)]
    pub share: T,
}

/// The configuration for semi-field shares.
#[serde_as]
#[derive(Deserialize)]
pub struct SemiFieldConfig {
    /// The prime element in this semi field.
    #[serde_as(as = "DisplayFromStr")]
    pub prime_element: BigInt,

    /// The binary extension field element in this semi field.
    pub binary_ext_element: u8,
}

/// The configuration for prime field shares.
#[serde_as]
#[derive(Deserialize)]
pub struct PrimeFieldConfig {
    /// The share itself.
    #[serde_as(as = "DisplayFromStr")]
    pub element: BigInt,
}

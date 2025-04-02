//! Utilities for handling network configurations

use crate::{path::config_directory, ToolConfig};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// The network configuration
#[derive(Serialize, Deserialize, Debug, Default)]
pub struct NetworkConfig {
    /// The endpoint for the bootnode to connect to.
    pub bootnode: String,

    /// Payments configuration
    pub payments: Option<PaymentsConfig>,

    /// The nilauth configuration.
    pub nilauth: Option<NilauthConfig>,
}

#[derive(Default, Serialize, Deserialize, Debug, PartialEq)]
pub struct PaymentsConfig {
    /// The chain id used in nilchain.
    pub nilchain_chain_id: Option<String>,

    /// The nilchain RPC endpoint.
    pub nilchain_rpc_endpoint: String,

    /// The nilchain gRPC endpoint.
    #[serde(default)]
    pub nilchain_grpc_endpoint: Option<String>,

    /// The nilchain payments private key.
    pub nilchain_private_key: String,

    /// The gas price to use, in unil units.
    #[serde(default)]
    pub gas_price: Option<f64>,
}

impl ToolConfig for NetworkConfig {
    fn root_config_path() -> PathBuf {
        config_directory().map(|dir| dir.join("networks")).unwrap_or_else(|| PathBuf::from("./"))
    }
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct NilauthConfig {
    /// The nilauth endpoint to use.
    pub endpoint: String,
}

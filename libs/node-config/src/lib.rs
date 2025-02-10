//! The configuration for a node.

use config::ConfigError;
use execution_engine_vm::vm::config::ExecutionVmConfig;
use program_auditor::ProgramAuditorConfig;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use std::{collections::HashMap, net::SocketAddr, num::NonZeroU32, path::PathBuf, time::Duration};

/// The top level configuration.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    /// The runtime configuration.
    pub runtime: RuntimeConfig,

    /// The storage configuration.
    pub storage: StorageConfig,

    /// The node's identity configuration.
    pub identity: IdentityConfig,

    /// The metrics configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metrics: Option<MetricsConfig>,

    /// The tracing configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tracing: Option<TracingConfig>,

    /// The network configuration.
    pub network: NetworkConfig,

    /// The cluster this node is a part of.
    pub cluster: Cluster,

    /// Program auditor configuration
    pub program_auditor: ProgramAuditorConfig,

    /// The payments configuration.
    pub payments: PaymentsConfig,

    /// Execution engine vm configuration.
    #[serde(default)]
    pub execution_engine: ExecutionVmConfig,
}

impl Config {
    /// Load the configuration from a path.
    ///
    /// Any of the configuration properties can also be overridden by using environment variables.
    ///
    /// For example, the `runtime.grpc.bind_endpoint` property can be set by using
    /// `RUNTIME__GRPC__BIND_ENDPOINT=0.0.0.0:1337`. Note the double underscores to delimit segments
    /// and single underscores to refer to fields.
    pub fn new(path: PathBuf) -> Result<Self, ConfigError> {
        let source = config::File::from(path).format(config::FileFormat::Yaml);
        let config = config::Config::builder()
            .add_source(source)
            .add_source(config::Environment::default().separator("__"))
            .build()?;
        config.try_deserialize()
    }
}

/// The metrics configuration.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MetricsConfig {
    /// The endpoint in which the prometheus metrics are exposed.
    pub listen_address: SocketAddr,

    /// The interval at which the process metrics collector runs.
    #[serde(with = "humantime_serde", default = "default_process_collector_interval")]
    pub process_collector_interval: Duration,

    /// The static labels to be used in every exposed metric.
    #[serde(default)]
    pub static_labels: HashMap<String, String>,
}

/// Configuration for the runtime.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RuntimeConfig {
    /// The maximum number of concurrent actions allowed.
    #[serde(default = "default_max_concurrent_actions")]
    pub max_concurrent_actions: usize,

    /// The gRPC config.
    pub grpc: GrpcConfig,
}

/// The gRPC config.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GrpcConfig {
    /// The endpoint to bind to.
    pub bind_endpoint: SocketAddr,

    /// The optional TLS config.
    pub tls: Option<GrpcTlsConfig>,

    /// The rate limiting configuration.
    #[serde(default)]
    pub rate_limit: Option<RateLimitConfig>,
}

/// The gRPC TLS config.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GrpcTlsConfig {
    /// Path to the certificate file in PEM format.
    pub cert: PathBuf,

    /// Path to the key.
    pub key: PathBuf,

    /// Path to the CA's certificate.
    pub ca_cert: Option<PathBuf>,
}

/// The rate limit configuration.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// The bucketting strategy for rate limiting.
    pub bucket: RateLimitBucket,

    /// The max number of requests per bucket.
    pub max_per_bucket: NonZeroU32,
}

/// The rate limit configuration.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum RateLimitBucket {
    /// A per-second rate limit bucket.
    Second,

    /// A per-minute rate limit bucket.
    Minute,

    /// A per-hour rate limit bucket.
    Hour,
}

/// Configuration for the storage.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Object storage configuration.
    pub object_storage: ObjectStorageConfig,

    /// The URL to the SQLite database.
    pub db_url: String,
}

/// Configuration for the object storage.
#[derive(PartialEq, Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObjectStorageConfig {
    /// Local in-memory S3 backend.
    #[default]
    InMemory,

    /// Local filesystem S3 backend.
    Filesystem { path: PathBuf },

    /// AWS S3 backend.
    AwsS3 {
        /// AWS bucket name.
        bucket_name: String,
        /// AWS Region.
        region: Option<String>,
        /// Endpoint URL. This primarily exists to set a static endpoint for tools like `LocalStack`.
        endpoint_url: Option<String>,
        /// Allow use HTTP instead of HTTPS.
        allow_http: Option<bool>,
    },
}

/// Configuration for the private key.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[serde(untagged)]
pub enum PrivateKeyConfig {
    /// Seed used for generating node's private key.
    Seed {
        /// The seed value
        seed: String,

        /// The kind of key used.
        #[serde(default)]
        kind: KeyKind,
    },

    /// A private key's raw bytes.
    Raw {
        /// The key.
        #[serde(deserialize_with = "hex::serde::deserialize")]
        key: Vec<u8>,

        /// The kind of key used.
        #[serde(default)]
        kind: KeyKind,
    },

    File {
        /// Path to the file containing the private key in hex format.
        path: String,

        /// The kind of key used.
        #[serde(default)]
        kind: KeyKind,
    },
}

/// Configuration for the node's identity.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IdentityConfig {
    /// Private key configuration options
    pub private_key: PrivateKeyConfig,
}

fn default_process_collector_interval() -> Duration {
    Duration::from_secs(30)
}

/// Configuration for tracing.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TracingConfig {
    /// The path where to store the JSON traces.
    pub json_path: Option<PathBuf>,

    /// Whether to print output to standard output.
    #[serde(default)]
    pub stdout: bool,
}

/// The payments configuration.
#[derive(Clone, Debug, Deserialize, Serialize, Default)]
pub struct PaymentsConfig {
    /// The payments RPC endpoint.
    pub rpc_endpoint: String,

    /// The quote TTL.
    #[serde(default = "default_quote_ttl")]
    pub quote_ttl: Duration,

    /// The receipt TTL.
    #[serde(default = "default_receipt_ttl")]
    pub receipt_ttl: Duration,

    /// The minimum add funds payment in credits.
    #[serde(default = "default_minimum_add_funds_payment")]
    pub minimum_add_funds_payment: u64,

    /// The expiration time for balances, in days.
    #[serde(default = "default_account_balance_expiration_days")]
    pub account_balance_expiration_days: u16,

    /// A list of pre-funded accounts.
    ///
    /// This should only be used in test environments, for example one for running load tests where
    /// we want to avoid having to manually fund keys.
    #[serde(default)]
    pub prefunded_accounts: Vec<PrefundedAccount>,

    /// Dollar token conversion configuration.
    #[serde(default)]
    pub dollar_token_conversion: Option<TokenDollarConversionConfig>,

    /// Dollar token fixed conversion for cases where coingecko is not used.
    #[serde(default = "default_dollar_token_conversion_fixed")]
    pub dollar_token_conversion_fixed: f64,

    /// The pricing configuration.
    #[serde(default)]
    pub pricing: PricingConfig,
}

/// A pre-funded account.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PrefundedAccount {
    /// The user account to be funded.
    pub account: String,

    /// The amount to fund the account with.
    pub amount: u64,
}

/// The pricing configuration.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct PricingConfig {
    /// Price of retrieve permissions operation
    pub retrieve_permissions_price: u64,
    /// Price of pool status operation
    pub pool_status_price: u64,
    /// Price of overwrite permissions operation
    pub overwrite_permissions_price: u64,
    /// Price of update permissions operation
    pub update_permissions_price: u64,
    /// Price of retrieve values operation
    pub retrieve_values_price: u64,
    /// Price of store program operation
    pub store_program_price: u64,
    /// Price of store values operation
    pub store_values_price: u64,
    /// Price of invoke compute operation
    pub invoke_compute_price: u64,
}

/// A cluster's definition.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Cluster {
    /// The members of this cluster.
    pub members: Vec<ClusterMember>,

    /// The leader for this cluster.
    pub leader: ClusterMember,

    /// The prime number used in this cluster.
    pub prime: Prime,

    /// The polynomial degree used in this cluster.
    pub polynomial_degree: u32,

    /// The security parameter kappa used in this cluster.
    pub kappa: u32,
}

/// A cluster member.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ClusterMember {
    /// The public keys for this member.
    pub public_keys: PublicKeys,

    // The gRPC endpoint this member can be reached at.
    pub grpc_endpoint: String,
}

/// The public keys for a cluster member.
#[serde_as]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PublicKeys {
    /// The authentication public key.
    #[serde(deserialize_with = "hex::serde::deserialize")]
    #[serde(serialize_with = "hex::serde::serialize")]
    pub authentication: Vec<u8>,

    /// The public keys kind.
    #[serde(default)]
    pub kind: KeyKind,
}

/// A key kind.
#[derive(Clone, Debug, Deserialize, Serialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum KeyKind {
    /// An ed25519 key.
    #[default]
    Ed25519,

    /// A secp256k1 key.
    Secp256k1,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Prime {
    // A safe 64 bit prime number.
    Safe64Bits,

    // A safe 128 bit prime number.
    Safe128Bits,

    // A safe 256 bit prime number.
    Safe256Bits,
}

/// The configuration for a pre-processing generation protocol.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Default)]
pub struct PreprocessingProtocolConfig {
    /// The number of elements to be generated on every run.
    pub batch_size: u64,

    /// The threshold at which, once we're below it, we should start preprocessing again.
    pub generation_threshold: u64,

    /// The amount the target offset is moved every time we generate preprocessing elements.
    pub target_offset_jump: u64,
}

/// The pre-processing generation protocols configurations.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Default)]
pub struct PreprocessingConfig {
    /// The PREP-COMPARE generation protocol configuration.
    pub compare: PreprocessingProtocolConfig,

    /// The PREP-DIV-INT-SECRET generation protocol configuration.
    pub division_integer_secret: PreprocessingProtocolConfig,

    /// The PREP-MODULO generation protocol configuration.
    pub modulo: PreprocessingProtocolConfig,

    /// The PREP-PUBLIC-OUTPUT-EQUALITY generation protocol configuration.
    pub public_output_equality: PreprocessingProtocolConfig,

    /// The PREP-TRUNCPR generation protocol configuration
    pub truncpr: PreprocessingProtocolConfig,

    /// The PREP-TRUNC generation protocol configuration
    pub trunc: PreprocessingProtocolConfig,

    /// The PREP-PRIVATE-EQUALITY generation protocol configuration
    pub equals_integer_secret: PreprocessingProtocolConfig,

    /// The RandomInteger generation protocol configuration.
    pub random_integer: PreprocessingProtocolConfig,

    /// The RandomBit generation protocol configuration.
    pub random_boolean: PreprocessingProtocolConfig,
}

impl PreprocessingConfig {
    /// Create a new instance using the same config for all protocols.
    pub fn new(config: PreprocessingProtocolConfig) -> Self {
        Self {
            compare: config.clone(),
            division_integer_secret: config.clone(),
            modulo: config.clone(),
            public_output_equality: config.clone(),
            truncpr: config.clone(),
            trunc: config.clone(),
            equals_integer_secret: config.clone(),
            random_integer: config.clone(),
            random_boolean: config.clone(),
        }
    }
}

/// The configuration for an auxiliary material protocol.
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct AuxiliaryMaterialProtocolConfig {
    /// Whether the protocol is enabled.
    #[serde(default)]
    pub enabled: bool,

    /// The version to be generated.
    #[serde(default)]
    pub version: u32,
}

/// The configuration for auxiliary material generation.
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct AuxiliaryMaterialConfig {
    /// Configuration for the cggmp21 ecdsa auxiliary info material protocol.
    pub cggmp21_aux_info: AuxiliaryMaterialProtocolConfig,
}

/// The network configuration.
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct NetworkConfig {
    /// The preprocessing configuration.
    ///
    /// This should only be set on the coordinator/leader node.
    #[serde(default)]
    pub preprocessing: Option<PreprocessingConfig>,

    /// The auxiliary material configuration.
    ///
    /// This should only be set on the coordinator/leader node.
    #[serde(default)]
    pub auxiliary_material: Option<AuxiliaryMaterialConfig>,

    /// The maximum request payload size.
    #[serde(default = "default_max_payload_size")]
    pub max_payload_size: u64,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct TokenDollarConversionConfig {
    /// The API key for the CoinGecko API.
    pub coingecko_api_key: String,
    /// Coin Id
    pub coin_id: String,
}

/// The default maximum payload size.
pub fn default_max_payload_size() -> u64 {
    6 * 1024 * 1024
}

/// The default TTL for quotes.
pub fn default_quote_ttl() -> Duration {
    Duration::from_secs(60 * 60 * 24)
}

/// The default TTL for receipts.
pub fn default_receipt_ttl() -> Duration {
    Duration::from_secs(60 * 60 * 24)
}

fn default_max_concurrent_actions() -> usize {
    usize::MAX
}

fn default_minimum_add_funds_payment() -> u64 {
    // $ 10
    1_000
}

fn default_account_balance_expiration_days() -> u16 {
    30
}

fn default_dollar_token_conversion_fixed() -> f64 {
    // 1$
    1.0
}

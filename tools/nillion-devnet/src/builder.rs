use anyhow::Context;
use mpc_vm::{requirements::MPCProgramRequirements, vm::ExecutionVmConfig};
use node::{
    builder::{NodeBuilder, NodeHandle, PreprocessingMode},
    config::{
        default_max_payload_size, default_quote_ttl, default_receipt_ttl, Cluster, GrpcConfig, GrpcTlsConfig,
        NetworkConfig, ObjectStorageConfig, PaymentsConfig, PreprocessingConfig, PreprocessingProtocolConfig,
        PricingConfig, StorageConfig,
    },
};
use node_config::{
    AuxiliaryMaterialConfig, AuxiliaryMaterialProtocolConfig, IdentityConfig, KeyKind, PrivateKeyConfig, RuntimeConfig,
};
use program_auditor::ProgramAuditorConfig;
use std::{
    collections::HashMap,
    fs::create_dir_all,
    net::{IpAddr, SocketAddr},
    path::PathBuf,
};
use user_keypair::SigningKey;

const DB_FILENAME: &str = "db.sqlite";

// Note: these are hardcoded as the end user shouldn't care about this when testing.
const DEFAULT_PREPROCESSING_CONFIG: PreprocessingConfig = PreprocessingConfig {
    compare: PreprocessingProtocolConfig { batch_size: 128, generation_threshold: 1_000, target_offset_jump: 1_000 },
    division_integer_secret: PreprocessingProtocolConfig {
        batch_size: 32,
        generation_threshold: 250,
        target_offset_jump: 25,
    },
    modulo: PreprocessingProtocolConfig { batch_size: 32, generation_threshold: 250, target_offset_jump: 25 },
    public_output_equality: PreprocessingProtocolConfig {
        batch_size: 32,
        generation_threshold: 1_000,
        target_offset_jump: 100,
    },
    equals_integer_secret: PreprocessingProtocolConfig {
        batch_size: 32,
        generation_threshold: 1_000,
        target_offset_jump: 100,
    },
    truncpr: PreprocessingProtocolConfig { batch_size: 32, generation_threshold: 1_000, target_offset_jump: 100 },
    trunc: PreprocessingProtocolConfig { batch_size: 32, generation_threshold: 1_000, target_offset_jump: 100 },
    random_integer: PreprocessingProtocolConfig {
        batch_size: 1024,
        generation_threshold: 1_000_000,
        target_offset_jump: 100_000,
    },
    random_boolean: PreprocessingProtocolConfig {
        batch_size: 1024,
        generation_threshold: 1_000_000,
        target_offset_jump: 100_000,
    },
};

// Note: The program auditor configuration is harcoded with the same values as production.
fn default_program_auditor_config(program_auditor_disabled: bool) -> ProgramAuditorConfig {
    ProgramAuditorConfig {
        max_memory_size: 50000,
        max_instructions: 50000,
        max_instructions_per_type: HashMap::new(),
        max_preprocessing: MPCProgramRequirements::default()
            .with_compare_elements(1000)
            .with_division_integer_secret_elements(1000)
            .with_equals_integer_secret_elements(1000)
            .with_modulo_elements(1000)
            .with_public_output_equality_elements(1000)
            .with_trunc_elements(1000)
            .with_truncpr_elements(1000),
        disable: program_auditor_disabled,
    }
}

macro_rules! try_get {
    ($option:ident) => {
        $option.ok_or_else(|| anyhow::anyhow!("option {} not set", stringify!($option)))
    };
}

/// A builder for Nillion nodes.
#[derive(Default)]
pub struct DevnetNodeBuilder {
    state_directory: Option<PathBuf>,
    signing_key: Option<SigningKey>,
    bind_address: Option<IpAddr>,
    cluster: Option<Cluster>,
    payments_rpc_endpoint: Option<String>,
    grpc_port: Option<u16>,
    tls_parameters: Option<GrpcTlsConfig>,
    program_auditor_disabled: bool,
}

impl DevnetNodeBuilder {
    /// Set the directory where the state (e.g. storage) will be persisted.
    pub fn state_directory(mut self, path: PathBuf) -> Self {
        self.state_directory = Some(path);
        self
    }

    /// The address to bind to.
    pub fn bind_address(mut self, address: IpAddr) -> Self {
        self.bind_address = Some(address);
        self
    }

    /// The cluster definition.
    pub fn cluster(mut self, cluster: Cluster) -> Self {
        self.cluster = Some(cluster);
        self
    }

    /// The signing key to use.
    pub fn signing_key(mut self, key: SigningKey) -> Self {
        self.signing_key = Some(key);
        self
    }

    /// The port to use for the gRPC server.
    pub fn grpc_port(mut self, port: u16) -> Self {
        self.grpc_port = Some(port);
        self
    }

    /// Set the TLS parameters to use for the gRPC server.
    pub fn tls_parameters(mut self, cert: PathBuf, key: PathBuf, ca_cert: PathBuf) -> Self {
        self.tls_parameters = Some(GrpcTlsConfig { cert, key, ca_cert: Some(ca_cert) });
        self
    }

    /// The payments RPC endpoint.
    pub fn payments_rpc_endpoint(mut self, endpoint: String) -> Self {
        self.payments_rpc_endpoint = Some(endpoint);
        self
    }

    pub fn program_auditor_disabled(mut self, program_auditor_disabled: bool) -> Self {
        self.program_auditor_disabled = program_auditor_disabled;
        self
    }

    /// Build the node config.
    pub fn build(self) -> anyhow::Result<NodeHandle> {
        let Self {
            state_directory,
            bind_address,
            cluster,
            payments_rpc_endpoint,
            signing_key,
            grpc_port,
            tls_parameters,
            program_auditor_disabled,
        } = self;

        let state_directory = try_get!(state_directory)?;
        let bind_address = try_get!(bind_address)?;
        let payments_rpc_endpoint = try_get!(payments_rpc_endpoint)?;
        let signing_key = try_get!(signing_key)?;
        let grpc_port = try_get!(grpc_port)?;
        let cluster = try_get!(cluster)?;
        let repository_path = state_directory.join("store");
        let db_path = state_directory.join(DB_FILENAME);
        create_dir_all(&state_directory).context("creating state directory")?;

        let grpc = GrpcConfig {
            bind_endpoint: SocketAddr::new(bind_address, grpc_port),
            tls: tls_parameters,
            rate_limit: None,
        };
        let raw_key = signing_key.as_bytes();
        let key_kind = match signing_key {
            SigningKey::Ed25519(_) => KeyKind::Ed25519,
            SigningKey::Secp256k1(_) => KeyKind::Secp256k1,
        };
        let db_url = format!("sqlite://{}", db_path.display());

        let config = node_config::Config {
            identity: IdentityConfig { private_key: PrivateKeyConfig::Raw { key: raw_key, kind: key_kind } },
            network: NetworkConfig {
                preprocessing: Some(DEFAULT_PREPROCESSING_CONFIG.clone()),
                auxiliary_material: Some(AuxiliaryMaterialConfig {
                    cggmp21_aux_info: AuxiliaryMaterialProtocolConfig { enabled: true, version: 0 },
                }),
                max_payload_size: default_max_payload_size(),
            },
            cluster,
            storage: StorageConfig {
                object_storage: ObjectStorageConfig::Filesystem { path: repository_path },
                db_url,
            },
            runtime: RuntimeConfig { max_concurrent_actions: 100, grpc },
            payments: PaymentsConfig {
                rpc_endpoint: payments_rpc_endpoint,
                pricing: PricingConfig {
                    retrieve_permissions_price: 1,
                    pool_status_price: 1,
                    overwrite_permissions_price: 1,
                    update_permissions_price: 1,
                    retrieve_values_price: 1,
                    store_program_price: 1,
                    store_values_price: 1,
                    invoke_compute_price: 1,
                },
                quote_ttl: default_quote_ttl(),
                receipt_ttl: default_receipt_ttl(),
                minimum_add_funds_payment: 1,
                account_balance_expiration_days: 365,
                prefunded_accounts: vec![],
            },
            program_auditor: default_program_auditor_config(program_auditor_disabled),
            execution_engine: ExecutionVmConfig::default(),
            metrics: None,
            tracing: None,
        };
        NodeBuilder::new(config).preprocessing_mode(PreprocessingMode::Fake).launch()
    }
}

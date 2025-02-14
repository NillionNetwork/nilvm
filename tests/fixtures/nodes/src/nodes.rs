use crate::{cleanup, programs::upload_programs};
use ::tracing::{info, warn};
use anyhow::{anyhow, bail, Context, Error};
use basic_types::PartyId;
use ctor::ctor;
use futures::future;
use grpc_channel::{token::TokenAuthenticator, AuthenticatedGrpcChannel, GrpcChannelConfig};
use nillion_chain_client::{
    client::NillionChainClient,
    key::{NillionChainAddress, NillionChainPrivateKey},
};
use nillion_chain_node::{
    node::{GenesisAccount, NillionChainNode, NillionChainNodeBuilder},
    transactions::TokenAmount,
};
use nillion_client::{
    builder::VmClientBuilder, grpc::MembershipClient, payments::NillionChainClientPayer, vm::VmClient,
    Ed25519SigningKey, Secp256k1SigningKey, SigningKey, UserId,
};
use node_config::{Config, KeyKind, PrefundedAccount, PrivateKeyConfig};
use once_cell::sync::Lazy;
use rstest::fixture;
use serde::{Deserialize, Serialize};
use serde_files_utils::yaml::read_yaml;
use std::{
    collections::HashMap,
    env::{self, current_dir},
    fmt,
    fs::{self, File},
    net::TcpStream,
    path::{Path, PathBuf},
    process::{Child, Command},
    sync::atomic::{AtomicU64, Ordering},
    thread::{self, sleep},
    time::{Duration, Instant},
};
use tempfile::TempDir;
use tokio::runtime::Runtime;
use tracing_fixture::{tracing, Tracing};
use xshell::{cmd, Shell};

const PAYER_FUND_CHUNK: usize = 20;
const SIGNING_KEY_PREFIX: &str = "signinig-key-";
const TOTAL_PREFUNDED_KEYS: u64 = 1024;
const PREFUND_AMOUNT: TokenAmount = TokenAmount::Nil(100_000);

static PAYMENTS_RUNTIME: Lazy<Runtime> = Lazy::new(|| Runtime::new().expect("failed to create runtime"));

extern crate libc;

#[ctor]
fn init() {
    unsafe { libc::atexit(cleanup_at_exit) };
}

extern "C" fn cleanup_at_exit() {
    if matches!(TestMode::from_env(), TestMode::Process) {
        cleanup::kill_child_processes();
    }
}

/// The set of pre-uploaded test programs.
pub struct UploadedPrograms(pub(crate) HashMap<String, String>);

impl UploadedPrograms {
    /// Get the identifier for a program uploaded under this namespace.
    pub fn program_id(&self, program_name: &str) -> String {
        self.0.get(program_name).cloned().expect("program not found")
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct RemoteNodesConfig {
    bootnode_endpoint: String,
    payments_rpc_endpoint: String,
    payments_stash_key: String,
}

enum NodeContext {
    Process {
        #[allow(dead_code)]
        tempdir: tempfile::TempDir,
        bootnode_endpoint: String,
    },
    Remote(RemoteNodesConfig),
}

impl NodeContext {
    fn new_process() -> anyhow::Result<Self> {
        let tempdir = tempfile::TempDir::new()?;
        let logs_dir = current_dir()?.join("logs").to_string_lossy().to_string();

        // release or debug
        let default_build_type = if cfg!(debug_assertions) { "debug" } else { "release" };
        let node_build_profile = env::var("NODE_BUILD_PROFILE").unwrap_or_else(|_| default_build_type.to_string());

        // build node waiting before spawning
        info!("Running 'just build-local-{node_build_profile}-node'");
        let sh = Shell::new()?;
        cmd!(sh, "just build-local-{node_build_profile}-node")
            .run()
            .expect("Error running 'just build-local-{node_build_profile}-node");

        let mut bootnode_endpoint = None;
        let configs_path = networks_path();
        for entry in fs::read_dir(configs_path)? {
            let entry = entry?;
            let config_path = entry.path();
            let mut config = Config::new(config_path.clone())?;
            let public_key = match &config.identity.private_key {
                PrivateKeyConfig::Seed { seed, kind: curve } => match curve {
                    KeyKind::Ed25519 => Ed25519SigningKey::from_seed(seed).public_key().as_bytes().to_vec(),
                    KeyKind::Secp256k1 => Secp256k1SigningKey::try_from_seed(seed)?.public_key().as_bytes().to_vec(),
                },
                PrivateKeyConfig::Raw { key, kind: curve } => match curve {
                    KeyKind::Ed25519 => Ed25519SigningKey::try_from(key.as_ref())?.public_key().as_bytes().to_vec(),
                    KeyKind::Secp256k1 => Secp256k1SigningKey::try_from(key.as_ref())?.public_key().as_bytes().to_vec(),
                },
                PrivateKeyConfig::File { path, kind } => {
                    let key = fs::read_to_string(path).context("reading private key file")?;
                    let key = hex::decode(key.trim()).context("decoding private key")?;
                    match kind {
                        KeyKind::Ed25519 => Ed25519SigningKey::try_from(key.as_ref())?.public_key().as_bytes().to_vec(),
                        KeyKind::Secp256k1 => {
                            Secp256k1SigningKey::try_from(key.as_ref())?.public_key().as_bytes().to_vec()
                        }
                    }
                }
            };
            let party_id = PartyId::from(UserId::from_bytes(&public_key).as_ref());
            let port = config.runtime.grpc.bind_endpoint.port();

            info!("Spawning node using config: {config_path:?}");
            info!("Running 'just run-local-{node_build_profile}-node'");
            let stdout_path = Path::new(&logs_dir).join("stdout");
            let stderr_path = Path::new(&logs_dir).join("stderr");
            fs::create_dir_all(stdout_path.clone())?;
            fs::create_dir_all(stderr_path.clone())?;
            let stdout_path = stdout_path.join(format!("node-{port}.stdout.log"));
            let stderr_path = stderr_path.join(format!("node-{port}.stderr.log"));
            let party_config_path = tempdir.path().join(party_id.to_string());
            // Use the filesystem for sqlite so we get a more realistic environment than using it
            // in memory.
            let db_path = party_config_path.join("db.sqlite");
            fs::create_dir_all(db_path.parent().unwrap())?;
            info!("Storing {party_id} database in {}", db_path.display());

            config.storage.db_url = db_path.to_string_lossy().into();
            for index in 0..TOTAL_PREFUNDED_KEYS {
                let account = PrefundedAccount {
                    account: UserId::from_bytes(create_signing_key(index).public_key().as_bytes()).to_string(),
                    amount: PREFUND_AMOUNT.to_unil(),
                };
                config.payments.prefunded_accounts.push(account);
            }
            let config_path = party_config_path.join("config.yaml");
            fs::write(&config_path, serde_yaml::to_string(&config)?)?;

            let mut command = Command::new("just");
            command
                .arg(format!("run-local-{node_build_profile}-node"))
                .env("CONFIG_PATH", config_path)
                .stdout(File::create(stdout_path.clone()).unwrap_or_else(|err| {
                    panic!("Cannot open stdout log for node at path {:?}: {err}", stdout_path.display())
                }))
                .stderr(File::create(stderr_path.clone()).unwrap_or_else(|err| {
                    panic!("Cannot open stderr log for node at path {:?}: {err}", stderr_path.display())
                }));

            let child = command.spawn()?;
            cleanup::register_parent_process(child);

            if bootnode_endpoint.is_none() {
                bootnode_endpoint = Some(
                    config
                        .cluster
                        .members
                        .iter()
                        .find(|m| m.public_keys.authentication == public_key)
                        .expect("node not part of cluster")
                        .grpc_endpoint
                        .clone(),
                );
            }
        }
        let bootnode_endpoint = bootnode_endpoint.ok_or_else(|| anyhow!("no bootnode endpoint"))?;
        Ok(Self::Process { tempdir, bootnode_endpoint })
    }

    fn new_remote(config_path: PathBuf) -> anyhow::Result<Self> {
        let config: RemoteNodesConfig = read_yaml(&config_path)?;
        Ok(Self::Remote(config))
    }

    fn bootnode_endpoint(&self) -> String {
        match self {
            Self::Process { bootnode_endpoint, .. } => bootnode_endpoint.clone(),
            Self::Remote(config) => config.bootnode_endpoint.clone(),
        }
    }
}

pub struct Nodes {
    context: NodeContext,
    pub uploaded_programs: UploadedPrograms,
    nillion_chain_node: Box<dyn NillionChainNode>,
    stash_client: tokio::sync::Mutex<NillionChainClient>,
    next_payment_key_id: AtomicU64,
    next_signing_key_id: AtomicU64,
    funded_payers: tokio::sync::Mutex<Vec<NillionChainClientPayer>>,
    bootnode_party_id: Option<PartyId>,
}

impl Nodes {
    pub fn bootnode_channel(&self, key: SigningKey) -> AuthenticatedGrpcChannel {
        let party_id = self.bootnode_party_id.clone().expect("no bootnode party id");
        let config = self.bootnode_channel_config();
        let authenticator = TokenAuthenticator::new(key, party_id.as_ref().to_vec().into(), Duration::from_secs(60));
        config.authentication(authenticator).build().expect("failed to build channel")
    }

    pub async fn build_client(&self) -> VmClient {
        let payer = self.allocate_payer().await;
        self.build_custom_client(move |builder| builder.nilchain_payer(payer)).await
    }

    pub fn node_channel_config(&self, endpoint: String) -> GrpcChannelConfig {
        let mut config = GrpcChannelConfig::new(endpoint.clone());
        if self.needs_ca_cert(&endpoint) {
            config = config.ca_certificate(include_bytes!("../../../resources/tls/ca.pem")).domain("nillion.local");
        }
        config
    }

    pub fn nillion_chain_rpc_endpoint(&self) -> String {
        self.nillion_chain_node.rpc_endpoint()
    }

    pub fn nillion_chain_grpc_endpoint(&self) -> String {
        self.nillion_chain_node.grpc_endpoint().clone()
    }

    /// Fund, if needed, the list of addresses so they al have at least `amount` tokens.
    pub async fn top_up_balances(&self, addresses: Vec<NillionChainAddress>, amount: TokenAmount) {
        let _guard = PAYMENTS_RUNTIME.enter();
        let mut stash_client = self.stash_client.lock().await;
        info!("Topping up address {} addresses up to amount of {amount}", addresses.len());
        let target = TokenAmount::Unil((amount.to_unil() as f64 * 1.1) as u64);
        stash_client.top_up_balances(addresses, amount, target).await.expect("failed to fund key");
        info!("Addresses funded");
    }

    fn bootnode_channel_config(&self) -> GrpcChannelConfig {
        let endpoint = self.context.bootnode_endpoint();
        self.node_channel_config(endpoint)
    }

    fn needs_ca_cert(&self, endpoint: &str) -> bool {
        let is_tls = endpoint.starts_with("https://");
        is_tls && matches!(self.context, NodeContext::Process { .. })
    }

    pub async fn build_custom_client<F>(&self, callback: F) -> VmClient
    where
        F: FnOnce(VmClientBuilder) -> VmClientBuilder,
    {
        let key_id = self.next_signing_key_id.fetch_add(1, Ordering::Relaxed);
        let key = create_signing_key(key_id);
        let endpoint = self.context.bootnode_endpoint();
        let mut builder = VmClientBuilder::default().bootnode_url(endpoint.clone()).signing_key(key);
        if self.needs_ca_cert(&endpoint) {
            builder =
                builder.ca_cert(include_bytes!("../../../resources/tls/ca.pem")).certificate_domain("nillion.local");
        }
        let builder = callback(builder);
        builder.build().await.expect("failed to build client")
    }

    async fn wait_network_ready(&mut self) -> Result<(), Error> {
        let start_time = Instant::now();
        let timeout = Duration::from_secs(300);
        let membership_client =
            MembershipClient::new(self.bootnode_channel_config().build().expect("failed to create config"));
        let cluster = loop {
            match membership_client.cluster().await {
                Ok(cluster) => {
                    break cluster;
                }
                Err(e) => {
                    warn!("Bootnode is not up yet, retrying: {e}")
                }
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
            if start_time.elapsed() > timeout {
                bail!("Timed out waiting for bootnode to have gRPC endpoint up",);
            }
        };
        let bootnode_endpoint = self.context.bootnode_endpoint();
        let bootnode = cluster
            .members
            .iter()
            .find(|m| m.grpc_endpoint == bootnode_endpoint)
            .expect("bootnode not found in cluster");
        self.bootnode_party_id = Some(PartyId::from(Vec::from(bootnode.identity.clone())));
        for node in &cluster.members {
            let endpoint =
                node.grpc_endpoint.trim_start_matches("http://").trim_start_matches("https://").trim_end_matches("/");
            loop {
                if TcpStream::connect(endpoint).is_ok() {
                    break;
                }
                info!("Connection to node {endpoint} failed, retrying");
                sleep(Duration::from_millis(500));
                if start_time.elapsed() > timeout {
                    bail!("Timed out waiting for node {endpoint} to have gRPC endpoint up",);
                }
            }
        }
        Ok(())
    }

    pub async fn allocate_payer(&self) -> NillionChainClientPayer {
        // enter the payments runtime so all payers share the same reqwest client pool
        let _guard = PAYMENTS_RUNTIME.enter();
        let mut funded_payers = self.funded_payers.lock().await;

        if funded_payers.is_empty() {
            // Otherwise fund a chunk of payers at once if we don't have any
            let mut addresses = Vec::new();
            let mut keys = Vec::new();
            for _ in 0..PAYER_FUND_CHUNK {
                let payment_key_id = self.next_payment_key_id.fetch_add(1, Ordering::AcqRel);
                let payments_seed = format!("payment-seed-{payment_key_id}");
                let key = NillionChainPrivateKey::from_seed(&payments_seed).expect("private key creation failed");
                let address = key.address.clone();
                keys.push(key);
                addresses.push(address);
            }
            self.top_up_balances(addresses, TokenAmount::Nil(1)).await;

            // Create all the clients in bulk as this performs a lookup on the chain.
            let mut futs = Vec::new();
            for key in keys {
                let payments_rpc_endpoint = self.nillion_chain_rpc_endpoint();
                futs.push(NillionChainClient::new(payments_rpc_endpoint, key));
            }
            for result in future::join_all(futs).await {
                let client = result.expect("failed to look up client");
                let payer = NillionChainClientPayer::new(client);
                funded_payers.push(payer);
            }
        }
        funded_payers.pop().expect("should not be empty")
    }
}

#[derive(Clone)]
pub enum TestMode {
    RemoteManaged { config_path: String },
    Process,
}

impl fmt::Display for TestMode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            TestMode::RemoteManaged { config_path } => write!(f, "RemoteManaged with config path: {config_path}"),
            TestMode::Process => write!(f, "Process"),
        }
    }
}

impl TestMode {
    fn from_env() -> Self {
        if let Ok(config_path) = env::var("REMOTE_NODES") { Self::RemoteManaged { config_path } } else { Self::Process }
    }
}

pub struct RemoteNillionChainNode {
    pub rpc_endpoint: String,
    pub stash_key: String,
}

impl NillionChainNode for RemoteNillionChainNode {
    fn home(&self) -> PathBuf {
        unimplemented!()
    }

    fn rpc_endpoint(&self) -> String {
        self.rpc_endpoint.clone()
    }

    fn grpc_endpoint(&self) -> String {
        unimplemented!()
    }

    fn rest_api_endpoint(&self) -> String {
        unimplemented!()
    }

    fn get_genesis_account_private_key(&self, _name: &str) -> Result<NillionChainPrivateKey, Error> {
        NillionChainPrivateKey::from_hex(&self.stash_key)
    }

    fn take_child_process(&mut self) -> Option<Child> {
        None
    }

    fn chain_id(&self) -> &str {
        unimplemented!()
    }
}

#[fixture]
#[once]
pub fn nodes(_tracing: &Tracing) -> Nodes {
    let mode = TestMode::from_env();
    info!("Node mode: {mode}");

    let context = match mode {
        TestMode::RemoteManaged { config_path } => {
            NodeContext::new_remote(config_path.into()).expect("invalid remote node config")
        }
        TestMode::Process => NodeContext::new_process().expect("failed to start nodes"),
    };

    let mut nillion_chain_node: Box<dyn NillionChainNode> = match &context {
        NodeContext::Remote(config) => Box::new(RemoteNillionChainNode {
            rpc_endpoint: config.payments_rpc_endpoint.clone(),
            stash_key: config.payments_stash_key.clone(),
        }),
        NodeContext::Process { .. } => {
            let home = TempDir::new().expect("could not create temp dir").into_path();
            info!("nillion-chain home: {}", home.display());
            NillionChainNodeBuilder::new(home)
                .genesis_accounts(vec![GenesisAccount {
                    name: "stash".to_string(),
                    amount: TokenAmount::Nil(1_000_000_000),
                }])
                .log("logs/stdout/nillion-chain.log")
                .build()
                .expect("could not create nillion chain node")
        }
    };

    let stash_key = nillion_chain_node.get_genesis_account_private_key("stash").expect("failed to get stash key");
    let stash_client = thread::scope(|s| {
        s.spawn(|| {
            // `HttpClient` uses reqwest/hyper which spin up a background task to do connection
            // pooling. Because we use this client in tests and each test has its own tokio runtime,
            // the connection pooling task ends up in the first test's runtime so eventually some other
            // random test fails once that first test ends. This enters a runtime that lives forever so
            // that the pooling task starts there. This is probably not the best place to put this in
            // but it's certainly the easiest.
            PAYMENTS_RUNTIME.block_on(async {
                NillionChainClient::new(nillion_chain_node.rpc_endpoint(), stash_key)
                    .await
                    .expect("failed to create stash client")
            })
        })
        .join()
        .expect("waiting for network ready thread failed")
    });

    // Keep the children handle ourselves to clean it up at the end. The `Nodes` fixture is owned
    // by rstest and therefore never dropped so without this the nillion-chain process won't be
    // killed.
    if let Some(child_process) = nillion_chain_node.take_child_process() {
        cleanup::register_child_process(child_process);
    }

    let mut nodes = Nodes {
        context,
        uploaded_programs: UploadedPrograms(Default::default()),
        nillion_chain_node,
        stash_client: stash_client.into(),
        next_payment_key_id: Default::default(),
        next_signing_key_id: Default::default(),
        funded_payers: Default::default(),
        bootnode_party_id: None,
    };

    // This is because this is a non async rstest fixture but it gets run within an async context
    // because all tests are async. So to get around that we spin up a runtime and run a future
    // inside it.
    nodes.uploaded_programs = thread::scope(|s| {
        let namespace = s
            .spawn(|| {
                PAYMENTS_RUNTIME
                    .block_on(async {
                        nodes.wait_network_ready().await.expect("network did not become ready in time");
                        upload_programs(&nodes).await
                    })
                    .expect("uploading programs failed")
            })
            .join()
            .expect("program upload thread failed");
        namespace
    });
    nodes
}

fn networks_path() -> PathBuf {
    // ROOT_PATH is a workaround for an open rust-analyzer bug (https://github.com/rust-lang/rust-analyzer/issues/13208)
    // where the cwd when running and debugging a test is not the same, resulting in failed resource accesses.
    let resources = match env::var("ROOT_PATH") {
        Ok(root_path) => load_env("RESOURCES_PATH", PathBuf::from(root_path).join("tests/resources")),
        Err(_) => {
            let cwd = current_dir().expect("failed to get cwd");
            load_env("RESOURCES_PATH", cwd.join("../resources"))
        }
    };
    load_env("NETWORK_PATH", resources.join("network/default"))
}

fn load_env<S>(key: &str, default_value: S) -> PathBuf
where
    S: Into<PathBuf>,
{
    env::var(key).map(PathBuf::from).unwrap_or_else(|_| default_value.into())
}

fn create_signing_key(index: u64) -> SigningKey {
    let input = format!("{SIGNING_KEY_PREFIX}{index}");
    let key = Secp256k1SigningKey::try_from_seed(&input).expect("failed to create signing key");
    key.into()
}

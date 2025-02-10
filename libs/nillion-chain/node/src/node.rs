use anyhow::{anyhow, Context, Result};
use nillion_chain_client::{key::NillionChainPrivateKey, transactions::TokenAmount};
use std::{
    env,
    fs::{self, File, OpenOptions},
    io::{self, BufRead, BufReader, Read, Write},
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
};
use toml::Value;
use tracing::{debug, info};

const DEFAULT_NODE_BINARY_NAME: &str = "nilchaind";
const DEFAULT_NODE_BINARY_FOLDER: &str = "bin";
const DEFAULT_RPC_HOST: &str = "localhost";
const DEFAULT_RPC_PORT: u16 = 26648;
const DEFAULT_GRPC_PORT: u16 = 26649;
const DEFAULT_REST_API_PORT: u16 = 26650;
const DEFAULT_CHAIN_ID: &str = "nillion-chain-devnet";
const DEFAULT_MONIKER: &str = "nilchaind";
const DEFAULT_KEYRING_BACKEND: &str = "test";

/// A genesis account to be added to the chain
pub struct GenesisAccount {
    pub name: String,
    pub amount: TokenAmount,
}

/// The mode to use when initialising the chain
pub enum NillionChainInitMode {
    /// Fail if the chain is already initialised
    FailIfExists,

    /// Try to initialise the chain and ignore if it is already initialised
    IgnoreIfExists,

    /// Do not initialise the chain
    Disable,
}

/// A running instance of a nillion-chain node
pub struct DefaultNillionChainNode {
    instance: Option<Child>,
    home: PathBuf,
    node: PathBuf,
    rpc_endpoint: String,
    grpc_endpoint: String,
    rest_api_endpoint: String,
    chain_id: String,
}

pub trait NillionChainNode: Send + Sync {
    fn home(&self) -> PathBuf;
    fn rpc_endpoint(&self) -> String;
    fn grpc_endpoint(&self) -> String;
    fn rest_api_endpoint(&self) -> String;
    fn get_genesis_account_private_key(&self, account_name: &str) -> Result<NillionChainPrivateKey>;
    fn take_child_process(&mut self) -> Option<Child>;
    fn chain_id(&self) -> &str;
}

impl NillionChainNode for DefaultNillionChainNode {
    fn home(&self) -> PathBuf {
        self.home.clone()
    }

    fn rpc_endpoint(&self) -> String {
        self.rpc_endpoint.clone()
    }

    fn grpc_endpoint(&self) -> String {
        self.grpc_endpoint.clone()
    }

    fn rest_api_endpoint(&self) -> String {
        self.rest_api_endpoint.clone()
    }

    /// Gets the private key from nillion-chain home dir based on account name
    fn get_genesis_account_private_key(&self, account_name: &str) -> Result<NillionChainPrivateKey> {
        // Execute the nilchaind subcommand to reveal the private key
        let mut cmd = Command::new(self.node.clone())
            .args(vec![
                "keys",
                "export",
                account_name,
                "--keyring-backend",
                DEFAULT_KEYRING_BACKEND,
                "--home",
                self.home.to_str().context("home path is not valid")?,
                "--unsafe",
                "--unarmored-hex",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        // Write "y" to stdin to confirm revealing the private key
        while let Some(mut stdin) = cmd.stdin.take() {
            stdin.write_all(b"y\n")?;
        }

        let output = cmd.wait_with_output()?;
        let key = output.stdout;
        let key_hex = String::from_utf8(key.trim_ascii().to_vec())?;

        NillionChainPrivateKey::from_hex(&key_hex)
    }

    /// Takes the underlying child process handle.
    ///
    /// It's a responsibility of the caller to keep the handle alive.
    fn take_child_process(&mut self) -> Option<Child> {
        self.instance.take()
    }

    fn chain_id(&self) -> &str {
        &self.chain_id
    }
}

impl Drop for DefaultNillionChainNode {
    fn drop(&mut self) {
        if let Some(mut instance) = self.instance.take() {
            info!("Killing nillion chain node");
            let _ = instance.kill();
            info!("Waiting for nillion chain node to shutdown");
            let _ = instance.wait();
        }
    }
}

pub struct NillionChainNodeBuilder {
    home: PathBuf,
    node: PathBuf,
    chain_id: String,
    moniker: Option<String>,
    genesis_accounts: Vec<GenesisAccount>,
    init: NillionChainInitMode,
    log: Option<PathBuf>,
    bind_address: String,
}

impl NillionChainNodeBuilder {
    /// Creates a new NillionChainNodeBuilder with the given home directory.
    pub fn new<T: Into<PathBuf>>(home: T) -> Self {
        let home: PathBuf = home.into();
        let node = home.join(DEFAULT_NODE_BINARY_FOLDER).join(DEFAULT_NODE_BINARY_NAME);

        NillionChainNodeBuilder {
            home,
            node,
            chain_id: String::from(DEFAULT_CHAIN_ID),
            moniker: Some(String::from(DEFAULT_MONIKER)),
            genesis_accounts: vec![],
            init: NillionChainInitMode::FailIfExists,
            log: None,
            bind_address: DEFAULT_RPC_HOST.to_string(),
        }
    }

    // Extracts the embedded node binary to the given path
    fn extract_embedded_node_binary(path: PathBuf) -> Result<()> {
        // Embed the node binary during compilation (binary is downloaded by the build.rs script)
        let embedded_binary = include_bytes!(concat!(env!("OUT_DIR"), "/node/nilchaind"));

        if let Some(directory_path) = path.parent() {
            fs::create_dir_all(directory_path).context("Failed to create directories for nillion-chain binary")?;
        } else {
            return Err(anyhow::anyhow!("Failed to get the parent directory of the nillion-chain binary path"));
        }

        let mut file = File::create(&path).context("Failed to create node binary file")?;
        file.write_all(embedded_binary).context("Failed to write node binary to a file")?;
        let mut perms = fs::metadata(&path).expect("Failed to get metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).context("Failed to set permissions")?;
        Ok(())
    }

    /// Sets the chain ID
    pub fn chain_id(mut self, chain_id: String) -> Self {
        self.chain_id = chain_id;
        self
    }

    /// Sets the moniker, which is the custom username of the node
    pub fn moniker(mut self, moniker: String) -> Self {
        self.moniker = Some(moniker);
        self
    }

    /// Sets the genesis accounts
    pub fn genesis_accounts(mut self, genesis_accounts: Vec<GenesisAccount>) -> Self {
        self.genesis_accounts = genesis_accounts;
        self
    }

    /// Sets whether to enable logging
    pub fn log<T: Into<PathBuf>>(mut self, path: T) -> Self {
        self.log = Some(path.into());
        self
    }

    /// Set the bind address.
    pub fn bind_address<S: Into<String>>(mut self, address: S) -> Self {
        self.bind_address = address.into();
        self
    }

    /// Sets whether to initialise the node and fail if the chain is already initialised
    pub fn init(mut self, mode: NillionChainInitMode) -> Self {
        self.init = mode;
        self
    }

    // Executes a node binary CLI command with the given arguments
    fn execute(&self, args: Vec<&str>) -> Result<()> {
        let mut child = self.spawn(args)?;
        let status = child.wait()?;
        if status.success() { Ok(()) } else { Err(anyhow!("exited with status: {}", status)) }
    }

    // Spawns a new node process with the given arguments
    fn spawn(&self, args: Vec<&str>) -> Result<Child> {
        let node = self.node.clone();

        let mut cmd = Command::new(node).args(&args).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;

        if let Some(log_path) = &self.log {
            cmd = self.enable_logging(cmd, log_path)?;
        }

        Ok(cmd)
    }

    // Enables logging for the given command
    fn enable_logging(&self, mut cmd: Child, log_path: &Path) -> Result<Child> {
        let log_file = OpenOptions::new().create(true).append(true).open(log_path)?;

        if let Some(stdout) = cmd.stdout.take() {
            let log_file_clone = log_file.try_clone()?;
            std::thread::spawn(move || {
                Self::handle_log_stream(log_file_clone, stdout, "stdout").unwrap();
            });
        }

        if let Some(stderr) = cmd.stderr.take() {
            std::thread::spawn(move || {
                Self::handle_log_stream(log_file, stderr, "stderr").unwrap();
            });
        }

        Ok(cmd)
    }

    // Handles the log stream for the given reader
    fn handle_log_stream<R: std::io::Read + Send + 'static>(
        mut log_file: File,
        reader: R,
        label: &'static str,
    ) -> io::Result<()> {
        let reader = BufReader::new(reader);
        for line in reader.lines() {
            let line = line?;
            let output = format!("{label}: {line}\n");
            log_file.write_all(output.as_bytes())?;
        }
        Ok(())
    }

    fn read_toml_file(path: &str) -> Result<Value> {
        let mut file = File::open(path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        contents.parse().context("invalid TOML format")
    }

    fn write_toml_file(path: &str, value: &Value) -> Result<()> {
        let new_config = toml::to_string(value).context("Failed to serialize TOML")?;
        let mut file = File::create(path)?;
        file.write_all(new_config.as_bytes())?;
        Ok(())
    }

    fn update_comet_config(path: &str) -> Result<()> {
        let mut config = Self::read_toml_file(path)?;

        // TX time improvement settings:
        if let Some(api_section) = config.get_mut("consensus") {
            if let Some(timeout_commit) = api_section.get_mut("timeout_commit") {
                *timeout_commit = Value::String("200ms".to_string());
            }
            if let Some(timeout_propose) = api_section.get_mut("timeout_propose") {
                *timeout_propose = Value::String("200ms".to_string());
            }
            if let Some(timeout_precommit) = api_section.get_mut("timeout_precommit") {
                *timeout_precommit = Value::String("200ms".to_string());
            }
            if let Some(timeout_prevote) = api_section.get_mut("timeout_prevote") {
                *timeout_prevote = Value::String("200ms".to_string());
            }
            if let Some(skip_timeout_commit) = api_section.get_mut("skip_timeout_commit") {
                *skip_timeout_commit = Value::Boolean(true);
            }
        }

        Self::write_toml_file(path, &config)
    }

    fn update_app_config(&self, path: &str) -> Result<()> {
        let mut config = Self::read_toml_file(path)?;

        // Enable REST API and its shenanigans
        if let Some(api_section) = config.get_mut("api") {
            if let Some(enable) = api_section.get_mut("enable") {
                *enable = Value::Boolean(true);
            }
            if let Some(swagger) = api_section.get_mut("swagger") {
                *swagger = Value::Boolean(true);
            }
            if let Some(address) = api_section.get_mut("address") {
                *address = Value::String(format!("tcp://{}:{DEFAULT_REST_API_PORT}", self.bind_address));
            }
            if let Some(enable_unsafe_cors) = api_section.get_mut("enabled-unsafe-cors") {
                *enable_unsafe_cors = Value::Boolean(true);
            }
        }

        // Memory optimisation settings:
        // Enable custom pruning
        if let Some(pruning) = config.get_mut("pruning") {
            *pruning = Value::String("custom".to_string());
        }
        // Specifies the number of recent states to keep. A lower number reduces storage and memory usage
        if let Some(pruning_keep_recent) = config.get_mut("pruning-keep-recent") {
            *pruning_keep_recent = Value::Integer(1000); // default: 362880
        }
        // Defines how often the pruning operation is performed (in number of blocks)
        if let Some(pruning_interval) = config.get_mut("pruning-interval") {
            *pruning_interval = Value::Integer(100); // default: 10
        }
        // This controls the size of the IAVL tree cache. Reducing this can decrease RAM usage but may increase I/O operations.
        if let Some(iavl_cache) = config.get_mut("iavl-cache-size") {
            *iavl_cache = Value::Integer(1000); // default: 781250
        }

        Self::write_toml_file(path, &config)
    }

    // Bootstraps the node
    fn bootstrap(&self) -> Result<()> {
        let home_path = self.home.to_str().context("home path is not valid")?;
        let chain_id = self.chain_id.clone();
        let moniker = self.moniker.clone().context("moniker is required")?;
        let validator_amount = TokenAmount::Nil(1);

        debug!("Initialising nillion-chain node using home {home_path}");
        self.execute(vec![
            "init",
            &*moniker,
            "--chain-id",
            &*chain_id,
            "--default-denom",
            TokenAmount::lowest_denomination(),
            "--home",
            home_path,
        ])
        .context("failed to init chain")?;

        debug!("Configuring chain ID and keyring backend");
        self.execute(vec!["config", "set", "client", "chain-id", &*chain_id, "--home", home_path])
            .context("failed to configure chain id")?;
        self.execute(vec!["config", "set", "client", "keyring-backend", DEFAULT_KEYRING_BACKEND, "--home", home_path])
            .context("failed to configure keyring backend")?;

        if !self.genesis_accounts.is_empty() {
            for (index, acc) in self.genesis_accounts.iter().enumerate() {
                if acc.amount < validator_amount {
                    return Err(anyhow!("amount must be greater than {validator_amount}"));
                }

                debug!("Adding key for account: {:?}", acc.name);
                self.execute(vec![
                    "keys",
                    "add",
                    &*acc.name,
                    "--keyring-backend",
                    DEFAULT_KEYRING_BACKEND,
                    "--home",
                    home_path,
                ])?;

                debug!("Adding genesis account for: {:?}", acc.name);
                self.execute(vec![
                    "genesis",
                    "add-genesis-account",
                    &*acc.name,
                    format!("{}{}", acc.amount.to_unil(), TokenAmount::lowest_denomination()).as_str(),
                    "--keyring-backend",
                    DEFAULT_KEYRING_BACKEND,
                    "--home",
                    home_path,
                ])?;

                // Only first account is validator
                if index == 0 {
                    debug!("Declaring validator by adding gentx for: {:?}", acc.name);
                    self.execute(vec![
                        "genesis",
                        "gentx",
                        &*acc.name,
                        format!("{}{}", validator_amount.to_unil(), TokenAmount::lowest_denomination()).as_str(),
                        "--chain-id",
                        &*chain_id,
                        "--keyring-backend",
                        DEFAULT_KEYRING_BACKEND,
                        "--home",
                        home_path,
                    ])?;
                }
            }

            debug!("Adding all gentxs to genesis");
            self.execute(vec!["genesis", "collect-gentxs", "--home", home_path])?;
        }

        Self::update_comet_config(&format!("{home_path}/config/config.toml"))
            .context("failed to update cometbft config")?;

        self.update_app_config(&format!("{home_path}/config/app.toml")).context("failed to update app config")?;

        Ok(())
    }

    /// Builds the nillion-chain node and starts it.
    /// Returns a NillionChainNodeInstance.
    pub fn build(self) -> Result<Box<dyn NillionChainNode>> {
        info!("Running nillion-chain node in: {:?}", self.home);
        Self::extract_embedded_node_binary(self.node.clone())?;

        match self.init {
            NillionChainInitMode::FailIfExists => {
                if self.home.join("config/genesis.json").exists() {
                    return Err(anyhow::anyhow!("chain is already initialised"));
                } else {
                    self.bootstrap()?;
                }
            }
            NillionChainInitMode::IgnoreIfExists => {
                if !self.home.join("config/genesis.json").exists() {
                    self.bootstrap()?;
                } else {
                    debug!("Ignoring chain initialisation because it is already initialised");
                }
            }
            NillionChainInitMode::Disable => {
                debug!("Chain initialisation is disabled");
            }
        }

        debug!("Starting nillion-chain node",);
        let bind_address = &self.bind_address;
        let rpc_endpont = format!("{bind_address}:{DEFAULT_RPC_PORT}");
        let grpc_endpoint = format!("{bind_address}:{DEFAULT_GRPC_PORT}");
        let instance = self.spawn(vec![
            "start",
            "--rpc.laddr",
            &format!("tcp://{rpc_endpont}"),
            "--grpc.address",
            &grpc_endpoint,
            "--home",
            self.home.to_str().context("home path is not valid")?,
        ])?;

        Ok(Box::new(DefaultNillionChainNode {
            instance: Some(instance),
            home: self.home,
            node: self.node,
            rpc_endpoint: format!("http://{rpc_endpont}"),
            grpc_endpoint: format!("http://{grpc_endpoint}"),
            rest_api_endpoint: format!("http://{bind_address}:{DEFAULT_REST_API_PORT}"),
            chain_id: self.chain_id,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nillion_chain_client::{
        client::NillionChainClient,
        tx::{DefaultPaymentTransactionRetriever, PaymentTransactionRetriever},
    };
    use tempfile::TempDir;
    use tokio::{
        fs::File,
        io::{AsyncBufReadExt, BufReader},
        time::{self, Duration},
    };
    use tracing_test::traced_test;

    #[tokio::test]
    #[traced_test]
    async fn test_start_node() {
        let home = TempDir::new().expect("could not create temp dir").into_path();

        let stash_accounts_num = 3;
        let mut stash_accounts = vec![];
        for i in 0..stash_accounts_num {
            stash_accounts.push(GenesisAccount { name: format!("stash-{}", i), amount: TokenAmount::Nil(1_000_000) });
        }

        let nillion_chain_node = NillionChainNodeBuilder::new(home.clone())
            .genesis_accounts(stash_accounts)
            .log(home.join("nilchaind.log"))
            .build()
            .expect("could not build node");

        let log_file_path = home.join("nilchaind.log");
        let timeout_duration = Duration::from_secs(10);
        let timeout_result = time::timeout(timeout_duration, async {
            loop {
                if let Ok(log_file) = File::open(&log_file_path).await {
                    let mut reader = BufReader::new(log_file).lines();

                    while let Some(line) = reader.next_line().await.context("Failed to read line from log file")? {
                        if line.contains("Starting RPC HTTP server") {
                            return Ok::<(), anyhow::Error>(());
                        }
                    }
                } else {
                    time::sleep(Duration::from_millis(100)).await;
                }
            }
        })
        .await;

        match timeout_result {
            Ok(Ok(())) => {}
            Ok(Err(e)) => panic!("Test failed: {}", e),
            Err(_) => panic!("Test failed: timed out after 10 seconds"),
        }

        // Create tx retriever
        let tx_retriever = DefaultPaymentTransactionRetriever::new(&nillion_chain_node.rpc_endpoint())
            .expect("could not create tx retriever");

        for i in 0..stash_accounts_num {
            // Get stash account private key
            let pk = nillion_chain_node
                .get_genesis_account_private_key(&format!("stash-{}", i))
                .expect("could not get genesis account private key");

            // Create nillion chain client
            let mut client = NillionChainClient::new(nillion_chain_node.rpc_endpoint(), pk)
                .await
                .expect("could not create nillion chain client");

            // Resource info
            let resource = "nonce:test";

            // Let's pay for resource
            let tx_hash = client
                .pay_for_resource(TokenAmount::Nil(100), resource.as_bytes().to_vec())
                .await
                .expect("could not pay");

            // Check if transaction is valid
            let tx = tx_retriever.get(tx_hash.as_str()).await.expect("could not validate tx");

            assert_eq!(tx.amount, TokenAmount::Nil(100));
        }
    }
}

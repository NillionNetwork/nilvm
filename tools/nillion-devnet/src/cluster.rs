//! The cluster orchestrator.

use crate::{
    args::Cli,
    builder::DevnetNodeBuilder,
    identity::{NodeIdentities, NodeIdentity},
    proxy::NilchainProxy,
};
use anyhow::{anyhow, bail, Context, Result};
use basic_types::party::PartyId;
use clap::{error::ErrorKind, CommandFactory};
use futures::future::join_all;
use math_lib::modular::EncodedModulo;
use nillion_chain_client::{client::NillionChainClient, key::NillionChainPrivateKey};
use nillion_chain_node::{
    node::{GenesisAccount, NillionChainInitMode, NillionChainNode, NillionChainNodeBuilder},
    transactions::TokenAmount,
};
use node::{
    builder::{NodeBuilder, NodeHandle},
    config::{Cluster, ClusterMember, KeyKind, MetricsConfig, Prime, PublicKeys},
};
use rand::{rngs::SmallRng, seq::SliceRandom, Rng, SeedableRng};
use std::{
    collections::HashMap,
    fs::{self, create_dir_all},
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::{Path, PathBuf},
    time::Duration,
};
use tempfile::{tempdir, TempDir};
use tokio::{
    select,
    signal::unix::{signal, SignalKind},
};
use tools_config::{networks::PaymentsConfig, path::config_directory, ToolConfig};
use uuid::Uuid;

const MIN_LISTEN_PORT: u16 = 30000;
const MAX_LISTEN_PORT: u16 = 65535;
const NILCHAIN_RPC_ENDPOINT: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 48102);
const NILCHAIN_CHAIN_ID: &str = "nillion-chain-devnet";

/// Sets up and launches a cluster.
pub struct ClusterOrchestrator {
    cluster_id: Uuid,
    identities: NodeIdentities,
    leader: PartyId,
    state_directory: StateDirectory,
    bind_address: IpAddr,
    prime: EncodedModulo,
    prime_bits: usize,
    node_ports: HashMap<PartyId, Ports>,
    metrics_endpoint: Option<SocketAddr>,
    grpc: Grpc,
    program_auditor_disabled: bool,
}

impl ClusterOrchestrator {
    /// Construct an orchestrator from the CLI arguments.
    pub fn new(cli: Cli) -> Result<Self> {
        let seed = cli.seed;
        let cluster_id = cli.cluster_id.unwrap_or_else(|| Self::uuid_from_seed(seed.as_bytes()));
        let identities = NodeIdentities::new(&seed, cli.node_count)?;
        // pull all nodes from the same shuffling of ports to avoid collisions
        let mut ports = Self::build_port_shuffling(seed.as_bytes());
        let node_ports = Self::build_node_ports(&identities, ports.as_mut())?;
        let state_directory = match cli.state_directory {
            Some(path) => StateDirectory::Static(path),
            None => StateDirectory::temporary()?,
        };
        let bind_address = cli.bind_address;
        let prime_bits = cli.prime_bits;
        let leader = {
            let mut parties = Vec::from_iter(identities.0.iter().map(|i| i.party_id.clone()));
            parties.sort();
            parties.into_iter().next().expect("no parties").clone()
        };

        let prime = match EncodedModulo::try_safe_prime_from_bits(prime_bits as u32) {
            Ok(prime) => prime,
            Err(e) => {
                let mut cmd = Cli::command();
                cmd.error(ErrorKind::ValueValidation, format!("Error in prime_bits : {}", e)).exit()
            }
        };
        let metrics_endpoint = match cli.enable_metrics {
            true => {
                let port = ports.pop().ok_or_else(|| anyhow!("no ports left"))?;
                Some(SocketAddr::new(cli.metrics_bind_address, port))
            }
            false => None,
        };
        let grpc = match (cli.tls_certificate, cli.tls_key, cli.tls_ca_certificate) {
            (None, None, None) => Grpc::EnabledInsecure,
            (Some(cert), Some(key), Some(ca_cert)) => Grpc::EnabledTls { cert, key, ca_cert },
            (..) => bail!("tls key and cert paths must be provided"),
        };

        Ok(Self {
            cluster_id,
            identities,
            leader,
            state_directory,
            bind_address,
            prime,
            prime_bits,
            node_ports,
            metrics_endpoint,
            grpc,
            program_auditor_disabled: cli.disable_program_auditor,
        })
    }

    fn build_port_shuffling(seed: &[u8]) -> Vec<u16> {
        // Choose a random shuffling of ports in between the pre-assigned range, this
        // attempts to a) bind everything to a different port and b) use high ports to
        // lower the chances of choosing an already bound one.
        let mut rng = Self::make_rng(seed);
        let mut ports: Vec<_> = (MIN_LISTEN_PORT..=MAX_LISTEN_PORT).collect();
        ports.shuffle(&mut rng);
        ports
    }

    async fn launch(&self) -> Result<Handles> {
        println!("ℹ️ cluster id is {}", self.cluster_id);
        println!("ℹ️ using {} bit prime", self.prime_bits);
        println!(
            "ℹ️ storing state in {} ({} available)",
            self.state_directory.path().display(),
            self.available_state_space()
        );
        // This needs to be done before spinning up the nodes, as otherwise metrics will fail
        // to register in the prometheus registry.
        if let Some(endpoint) = self.metrics_endpoint {
            Self::launch_metrics_export(endpoint).await?;
        }

        let nillion_chain_node = self.launch_nillion_chain_node().await?;
        let cluster = self.build_cluster_definition();
        let mut nodes = Vec::new();
        for (index, identity) in self.identities.0.iter().enumerate() {
            let node = self.launch_node(index + 1, identity, &*nillion_chain_node, cluster.clone())?;
            nodes.push(node);
        }
        let bootnode_address = self.build_bootnode_address(&self.identities.0[0].party_id).await;
        let payment_keys = self.fund_payment_keys(nillion_chain_node.as_ref()).await?;

        self.dump_network_config(bootnode_address.clone(), nillion_chain_node.as_ref(), &payment_keys)?;
        self.dump_env_config(bootnode_address.clone(), nillion_chain_node.as_ref(), &payment_keys)?;
        Ok(Handles { nodes, nillion_chain_node })
    }

    fn build_cluster_definition(&self) -> Cluster {
        let mut members = Vec::new();
        let is_https = matches!(self.grpc, Grpc::EnabledTls { .. });
        let scheme = if is_https { "https" } else { "http" };
        let mut leader_index = 0;
        for (index, identity) in self.identities.0.iter().enumerate() {
            let authentication_public_key = identity.key.public_key().as_bytes().to_vec();
            let port = self.node_ports.get(&identity.party_id).unwrap().grpc;
            let grpc_endpoint = format!("{scheme}://127.0.0.1:{port}");
            if identity.party_id == self.leader {
                leader_index = index;
            }
            members.push(ClusterMember {
                grpc_endpoint,
                public_keys: PublicKeys { authentication: authentication_public_key, kind: KeyKind::Secp256k1 },
            });
        }
        let leader = members[leader_index].clone();
        Cluster {
            members,
            leader,
            prime: match self.prime {
                EncodedModulo::U64SafePrime => Prime::Safe64Bits,
                EncodedModulo::U128SafePrime => Prime::Safe128Bits,
                EncodedModulo::U256SafePrime => Prime::Safe256Bits,
                _ => panic!("invalid prime"),
            },
            polynomial_degree: 1,
            kappa: 0,
        }
    }

    async fn fund_payment_keys(&self, nillion_chain_node: &dyn NillionChainNode) -> Result<Vec<String>> {
        println!("👛 funding nilchain keys");
        let stash_key =
            nillion_chain_node.get_genesis_account_private_key("stash").context("getting nilchain genesis key")?;
        let mut stash_client = NillionChainClient::new(nillion_chain_node.rpc_endpoint(), stash_key)
            .await
            .context("creating nilchain stash client")?;
        let mut payments = Vec::new();
        let mut keys = Vec::new();
        for index in 0..10 {
            let seed = format!("nillion-devnet-key-{index}");
            let key = NillionChainPrivateKey::from_seed(&seed).context("creating nilchain private key")?;
            payments.push((key.address.clone(), TokenAmount::Nil(1_000_000)));
            keys.push(key.as_hex());
        }
        stash_client.pay_many(payments).await.context("funding nilchain keys")?;
        Ok(keys)
    }

    fn available_state_space(&self) -> String {
        let Ok(free_bytes) = fs2::available_space(self.state_directory.path()) else {
            return "<unknown>".into();
        };
        let free_gbs = free_bytes as f64 / f64::from(1024 * 1024 * 1024);
        format!("{free_gbs:.2}Gbs")
    }

    fn dump_network_config(
        &self,
        bootnode_address: String,
        nillion_chain_node: &dyn NillionChainNode,
        private_keys: &[String],
    ) -> Result<()> {
        let private_key = private_keys.first().ok_or_else(|| anyhow!("no private keys")).cloned()?;
        let network_config = tools_config::networks::NetworkConfig {
            bootnode: bootnode_address,
            payments: Some(PaymentsConfig {
                nilchain_chain_id: Some(NILCHAIN_CHAIN_ID.to_string()),
                nilchain_rpc_endpoint: nillion_chain_node.rpc_endpoint(),
                nilchain_grpc_endpoint: Some(nillion_chain_node.grpc_endpoint()),
                nilchain_private_key: private_key,
                gas_price: None,
            }),
        };
        network_config.write_to_file("devnet")?;

        println!(
            "📝 network configuration written to {}",
            tools_config::networks::NetworkConfig::config_path("devnet")?.display()
        );
        Ok(())
    }

    fn dump_env_config(
        &self,
        bootnode_address: String,
        nillion_chain_node: &dyn NillionChainNode,
        private_keys: &[String],
    ) -> Result<()> {
        let cluster_id = self.cluster_id;
        let chain_id = nillion_chain_node.chain_id();
        let nilchain_grpc = nillion_chain_node.grpc_endpoint();
        let nilchain_rest_api = nillion_chain_node.rest_api_endpoint();
        let mut contents = format!(
            r#"# Nillion devnet parameters
NILLION_CLUSTER_ID={cluster_id}
NILLION_NILCHAIN_CHAIN_ID={chain_id}
NILLION_NILCHAIN_JSON_RPC=http://{NILCHAIN_RPC_ENDPOINT}
NILLION_NILCHAIN_REST_API={nilchain_rest_api}
NILLION_NILCHAIN_GRPC={nilchain_grpc}
NILLION_GRPC_ENDPOINT={bootnode_address}
"#
        );
        for (index, key) in private_keys.iter().enumerate() {
            let line = format!("NILLION_NILCHAIN_PRIVATE_KEY_{index}={key}\n");
            contents.push_str(&line);
        }

        let env_file_path = config_directory().unwrap_or_else(|| PathBuf::from(".")).join("nillion-devnet.env");
        fs::write(&env_file_path, contents).context("writing environment file")?;
        println!("🌄 environment file written to {}", env_file_path.display());
        Ok(())
    }

    async fn launch_nillion_chain_node(&self) -> Result<Box<dyn NillionChainNode>> {
        let home = self.state_directory.path().join("nillion-chain");
        let log_path = home.join("nilchaind.log");
        create_dir_all(&home)?;

        let node_builder = NillionChainNodeBuilder::new(home.clone())
            .init(NillionChainInitMode::IgnoreIfExists)
            .genesis_accounts(vec![GenesisAccount {
                name: "stash".to_string(),
                amount: TokenAmount::Nil(1_000_000_000),
            }])
            .chain_id(NILCHAIN_CHAIN_ID.to_string())
            .log(log_path);

        println!("🏃 starting nilchain node in: {}", home.display());
        let node = node_builder.build()?;
        NilchainProxy::run(NILCHAIN_RPC_ENDPOINT, node.rpc_endpoint()).await.context("launching nilchain proxy")?;

        println!("⛓  nilchain JSON RPC available at http://{NILCHAIN_RPC_ENDPOINT}");
        println!("⛓  nilchain REST API available at {}", node.rest_api_endpoint());
        println!("⛓  nilchain gRPC available at {}", node.grpc_endpoint());
        Ok(node)
    }

    pub async fn run(self) -> Result<()> {
        let handles = self.launch().await?;
        Self::run_until_signals(handles.nodes).await?;
        Ok(())
    }

    async fn run_until_signals(nodes: Vec<NodeHandle>) -> Result<()> {
        let mut term_signal = signal(SignalKind::terminate())?;
        let mut interrupt_signal = signal(SignalKind::interrupt())?;
        let mut hangup_signal = signal(SignalKind::hangup())?;
        select! {
            _ = term_signal.recv() => (),
            _ = interrupt_signal.recv() => (),
            _ = hangup_signal.recv() => (),
        };
        println!("⚠️ shutting down...");

        let mut futs = Vec::new();
        for node in nodes {
            futs.push(node.shutdown());
        }
        join_all(futs).await;
        Ok(())
    }

    fn launch_node(
        &self,
        node_index: usize,
        identity: &NodeIdentity,
        nillion_chain_node: &dyn NillionChainNode,
        cluster: Cluster,
    ) -> Result<NodeHandle> {
        let ports = self.node_ports.get(&identity.party_id).expect("ports for party not found");
        let state_path = self.state_directory.path().join(format!("node-{node_index}"));
        let keypair = identity.key.clone();
        println!("🏃 starting node {node_index}");

        let mut node_builder = DevnetNodeBuilder::default()
            .cluster(cluster)
            .bind_address(self.bind_address)
            .state_directory(state_path)
            .signing_key(keypair)
            .payments_rpc_endpoint(nillion_chain_node.rpc_endpoint())
            .program_auditor_disabled(self.program_auditor_disabled)
            .grpc_port(ports.grpc);
        if let Grpc::EnabledTls { cert, key, ca_cert } = &self.grpc {
            node_builder = node_builder.tls_parameters(cert.clone(), key.clone(), ca_cert.clone())
        }
        let node = node_builder.build()?;
        Ok(node)
    }

    async fn build_bootnode_address(&self, party_id: &PartyId) -> String {
        let bootnode_ports = self.node_ports.get(party_id).expect("no leader ports");
        let prefix = match self.grpc {
            Grpc::EnabledInsecure => "http",
            Grpc::EnabledTls { .. } => "https",
        };
        let port = bootnode_ports.grpc;
        let endpoint = format!("{prefix}://127.0.0.1:{port}");
        endpoint
    }

    fn make_rng(seed: &[u8]) -> SmallRng {
        let mut rng_seed: <SmallRng as SeedableRng>::Seed = Default::default();
        for (rng_byte, seed_byte) in rng_seed.iter_mut().zip(seed.iter()) {
            *rng_byte = *seed_byte;
        }
        SmallRng::from_seed(rng_seed)
    }

    fn uuid_from_seed(seed: &[u8]) -> Uuid {
        let mut rng = Self::make_rng(seed);
        let bytes = rng.gen();
        uuid::Builder::from_random_bytes(bytes).into_uuid()
    }

    fn build_node_ports(identities: &NodeIdentities, ports: &mut Vec<u16>) -> Result<HashMap<PartyId, Ports>> {
        let ports_per_node = 1;
        let ports_needed = identities.0.len() * ports_per_node;
        if ports_needed > ports.len() {
            bail!("cannot instantiate this many nodes");
        }
        let ports: Vec<_> = ports.drain(0..ports_needed).collect();
        let both_ports = ports.chunks(ports_per_node);
        let node_ports = identities
            .0
            .iter()
            .zip(both_ports)
            .map(|(identity, ports)| (identity.party_id.clone(), Ports { grpc: ports[0] }))
            .collect();
        Ok(node_ports)
    }

    async fn launch_metrics_export(endpoint: SocketAddr) -> Result<()> {
        let metrics = MetricsConfig {
            listen_address: endpoint,
            process_collector_interval: Duration::from_secs(30),
            static_labels: Default::default(),
        };
        NodeBuilder::initialize_metrics(&metrics).await?;
        println!("📈 nilvm prometheus metrics are available at http://{endpoint}/metrics");
        Ok(())
    }
}

enum Grpc {
    EnabledInsecure,
    EnabledTls { cert: PathBuf, key: PathBuf, ca_cert: PathBuf },
}

#[derive(Debug)]
struct Ports {
    grpc: u16,
}

struct Handles {
    nodes: Vec<NodeHandle>,
    #[allow(dead_code)]
    nillion_chain_node: Box<dyn NillionChainNode>,
}

enum StateDirectory {
    Static(PathBuf),
    Temporary(TempDir),
}

impl StateDirectory {
    fn temporary() -> Result<Self> {
        let dir = tempdir()?;
        Ok(Self::Temporary(dir))
    }

    fn path(&self) -> &Path {
        match self {
            Self::Static(path) => path,
            Self::Temporary(dir) => dir.path(),
        }
    }
}

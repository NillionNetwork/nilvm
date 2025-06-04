use anyhow::{bail, Context, Error};
use chrono::{SecondsFormat, Utc};
use clap::Parser;
use load_tool::{
    clients_pool::{ClientsPoolBuilder, Grpc},
    flow::{Flow, RetrieveValueArgs, StoreProgramArgs, StoreValueArgs},
    inputs::ArgsGenerator,
    runner::{LoadTestRunner, RunnerConfig},
    spec::{Operation, Seeds, SigningKeyMode, TestSpec, WorkerIncrementMode},
};
use log::info;
use nilchain_client::{client::NillionChainClient, key::NillionChainPrivateKey, transactions::TokenAmount};
use nillion_client::{Secp256k1SigningKey, SigningKey};
use serde_files_utils::yaml::read_yaml;
use std::{
    env, fs,
    path::{Path, PathBuf},
    sync::Arc,
};

/// Nillion load tool.
///
/// This tool allows creating load on the network in a user defined way.
#[derive(Parser, Debug)]
#[clap(about, version)]
struct Args {
    /// Load test file
    #[clap(short, long)]
    spec_path: PathBuf,

    /// Output file path.
    #[clap(short, long)]
    output_path: Option<PathBuf>,

    /// The bootnode to use.
    #[clap(short, long)]
    bootnode: String,

    /// The stash private key to use to fund nilchain keys.
    #[clap(long)]
    nilchain_stash_private_key: String,

    /// The nilchain RPC endpoint.
    #[clap(long)]
    nilchain_rpc_endpoint: String,

    /// The root certificate to verify the bootnode's identity.
    #[clap(long)]
    root_certificate: Option<PathBuf>,

    /// Enable verbose client output.
    #[clap(short, long)]
    verbose: bool,
}

struct LoadTestParameters {
    bootnode: String,
    spec_path: PathBuf,
    output_path: PathBuf,
    spec: TestSpec,
    nilchain_stash_private_key: String,
    nilchain_rpc_endpoint: String,
    root_certificate: Option<PathBuf>,
}

async fn execute_load_test(parameters: LoadTestParameters) -> Result<(), Error> {
    let LoadTestParameters {
        bootnode,
        spec_path,
        output_path,
        spec,
        nilchain_stash_private_key,
        nilchain_rpc_endpoint,
        root_certificate,
    } = parameters;
    let TestSpec {
        operation,
        max_flow_duration,
        max_test_duration,
        max_error_rate,
        mode,
        start_policy,
        error_policy,
        seeds,
        required_starting_balance,
        signing_key,
    } = spec;

    let clients_quantity = derive_clients_count(&mode);
    let required_starting_balance = TokenAmount::Unil(required_starting_balance);

    info!("Creating stash client");
    let stash_key =
        NillionChainPrivateKey::from_hex(&nilchain_stash_private_key).context("invalid payments stash key")?;
    let stash_client =
        NillionChainClient::new(nilchain_rpc_endpoint.clone(), stash_key).await.context("creating payments client")?;
    let grpc = match root_certificate {
        Some(cert) => {
            let cert = fs::read(cert).context("reading cert file")?;
            Grpc::EnabledSecure(bootnode, cert)
        }
        None => Grpc::EnabledInsecure(bootnode),
    };
    let signing_key = match signing_key {
        SigningKeyMode::Random => SigningKey::generate_secp256k1(),
        SigningKeyMode::PrivateKey(key) => {
            Secp256k1SigningKey::try_from(key.as_slice()).context("invalid signing key")?.into()
        }
    };
    let mut clients_pool_builder = ClientsPoolBuilder::new(
        clients_quantity,
        stash_client,
        nilchain_rpc_endpoint,
        required_starting_balance,
        grpc,
        signing_key,
    );

    if let Some(seeds) = seeds {
        let seeds = match seeds {
            Seeds::Prefix(prefix) => (0..clients_quantity).map(|i| format!("{}{}", prefix, i + 1)).collect(),
            Seeds::List(seeds) => seeds,
        };
        clients_pool_builder = clients_pool_builder.with_seeds(seeds);
    }

    let mut clients_pool = clients_pool_builder.build().await?;

    let mut clients = clients_pool.next().context("no clients available")?;

    if matches!(mode, WorkerIncrementMode::Steady { .. }) && max_test_duration.is_none() {
        bail!("steady mode requires max test duration");
    }
    info!("Pre-fetching cluster information...");
    let cluster_information = clients.vm.cluster();
    info!("Cluster has {} members", cluster_information.members.len());

    let configuration = RunnerConfig {
        output_path,
        max_test_duration,
        max_flow_duration,
        max_error_rate,
        mode,
        start_policy,
        error_policy,
    };

    let base_path = spec_path.parent().unwrap_or_else(|| Path::new("."));
    let flow = match operation {
        Operation::StoreValues { inputs } => {
            let values = ArgsGenerator::load_secrets_spec(inputs)?;
            let args = StoreValueArgs { values: Arc::new(values) };
            Flow::StoreValues(args)
        }
        Operation::RetrieveValue { input: values } => {
            let values = ArgsGenerator::load_secrets_spec(values)?;
            if values.len() != 1 {
                bail!("retrieve value spec requires a single secret");
            }
            info!("Storing value to be retrieved...");

            let (_, value) = values.iter().next().expect("no values");
            let values_id = clients.vm.store_values().ttl_days(1).add_values(values.clone()).build()?.invoke().await?;
            Flow::RetrieveValue(RetrieveValueArgs { values_id, value: value.clone() })
        }
        Operation::Compute(spec) => {
            let args = ArgsGenerator::build_compute_args(base_path, spec, &mut clients).await?;
            Flow::Compute(args)
        }
        Operation::StoreProgram { program_path: path } => {
            let raw_program = fs::read(path)?;
            Flow::StoreProgram(StoreProgramArgs { raw_program })
        }
    };
    let runner = LoadTestRunner::new(configuration);
    runner.run(clients_pool, flow).await?;
    Ok(())
}

fn derive_clients_count(worker_increment_mode: &WorkerIncrementMode) -> u32 {
    match worker_increment_mode {
        WorkerIncrementMode::Manual { initial_workers, .. } => *initial_workers,
        WorkerIncrementMode::Automatic => 1,
        WorkerIncrementMode::Steady { workers, clients, .. } => clients.unwrap_or(*workers),
    }
}

fn make_output_file_name() -> PathBuf {
    let date = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
    PathBuf::from(format!("{date}.json"))
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let args = Args::parse();
    let level = env::var("RUST_LOG");
    // Disable everything but our output unless `verbose` is set.
    if !args.verbose && level.is_ok() {
        let level = level.unwrap();
        env::set_var("RUST_LOG", format!("none,load_tool={level}"));
    }
    env_logger::init();

    let spec: TestSpec = read_yaml(&args.spec_path).context("error reading test file")?;

    // Use the given one or generate one based on the current time.
    let output_path = args.output_path.unwrap_or_else(make_output_file_name);

    info!("Initializing test");
    let parameters = LoadTestParameters {
        bootnode: args.bootnode,
        nilchain_stash_private_key: args.nilchain_stash_private_key,
        nilchain_rpc_endpoint: args.nilchain_rpc_endpoint,
        root_certificate: args.root_certificate,
        spec_path: args.spec_path,
        output_path,
        spec,
    };
    execute_load_test(parameters).await.map_err(|e| {
        log::error!("Error executing load test: {}", e);
        e
    })
}

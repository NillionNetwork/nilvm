use anyhow::{anyhow, Error, Result};
use clap::{error::ErrorKind, Args, CommandFactory, Parser, Subcommand, ValueEnum};
use clap_utils::shell_completions::ShellCompletionsArgs;
use nada_values_args::NadaValueArgs;
use nillion_client::{grpc::membership::NodeId, Clear, NadaValue, UserId, Uuid};
use serde::Deserialize;
use serde_with::{serde_as, DisplayFromStr};
use std::{collections::HashMap, path::PathBuf, str::FromStr};
use tools_config::{identities::Kind, path::config_directory};

/// Nillion CLI.
#[derive(Parser)]
pub struct Cli {
    /// Identities configuration
    #[clap(short, long)]
    pub identity: Option<String>,

    /// Network configuration name
    #[clap(short, long)]
    pub network: Option<String>,

    /// The command to be run.
    #[command(subcommand)]
    pub command: Command,

    /// Output format
    #[clap(long, global = true, value_enum)]
    pub output_format: Option<CommandOutputFormat>,

    /// The path to the configuration file.
    #[clap(long, global = true, default_value = default_config_path().into_os_string())]
    pub config_path: PathBuf,
}

/// A command to be executed.
#[derive(Subcommand)]
pub enum Command {
    /// Store or update values in the network.
    StoreValues(StoreValuesArgs),

    /// Retrieve values from the network.
    RetrieveValues(RetrieveValuesArgs),

    /// Store a program in the network.
    StoreProgram(StoreProgramArgs),

    /// Perform a computation in the network.
    Compute(ComputeArgs),

    /// Fetch the cluster's information.
    ClusterInformation,

    /// Delete values from the network.
    DeleteValues(DeleteValuesArgs),

    /// Fetch the preprocessing pool status for a cluster.
    PreprocessingPoolStatus(PreprocessingPoolStatusArgs),

    /// Display the node/user ids derived from the provided keys.
    InspectIds,

    /// Generate shell completions
    ShellCompletions(ShellCompletionsArgs),

    /// Retrieve permissions for stored secrets
    RetrievePermissions(RetrievePermissionsArgs),

    /// Overwrite all permissions on a stored secrets
    OverwritePermissions(OverwritePermissionsArgs),

    /// Update certain permissions on a stored secrets
    UpdatePermissions(UpdatePermissionsArgs),

    /// Generate user identities
    ///
    /// This is deprecated in favor of `identities add`.
    IdentityGen(IdentityGenArgs),

    /// Manage network configurations.
    #[clap(subcommand)]
    Networks(NetworksCommand),

    /// Manage identities.
    #[clap(subcommand)]
    Identities(IdentitiesCommand),

    /// Manage the context to use for all upcoming command invocations.
    #[clap(subcommand)]
    Context(ContextCommand),

    /// Display balance and add funds.
    #[clap(subcommand)]
    Balance(BalanceCommand),

    /// Get configuration information.
    #[clap(subcommand)]
    Config(ConfigCommand),
}

/// The output format for the command. Default is YAML.
#[derive(ValueEnum, Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandOutputFormat {
    /// Serialise into YAML
    #[default]
    Yaml,

    /// Serialise into JSON
    Json,
}

/// The arguments for the identity generation command.
#[derive(Args)]
pub struct IdentityGenArgs {
    /// Seed to use when generating the key
    #[arg(short, long)]
    pub seed: Option<String>,

    /// The curve to use.
    #[arg(short, long, default_value_t = Kind::Secp256k1)]
    pub curve: Kind,

    /// identity name
    pub name: String,
}

/// The identities command.
#[derive(Subcommand)]
pub enum IdentitiesCommand {
    /// Add an identity.
    Add(AddIdentityArgs),

    /// Lists all the available identities.
    List,

    /// Edit an identity file.
    Edit(EditIdentityArgs),

    /// Display an identity's information.
    Show(ShowIdentityArgs),

    /// Removes an identity.
    Remove(RemoveIdentityArgs),
}

/// The arguments for the identities add command.
#[derive(Args)]
pub struct AddIdentityArgs {
    /// The name of the identity to be added.
    pub name: String,

    /// Seed to use when generating the key.
    #[arg(short, long)]
    pub seed: Option<String>,
}

/// The arguments for the identity edit command.
#[derive(Args)]
pub struct EditIdentityArgs {
    /// The name of the identity to be edited.
    pub name: String,
}

/// The arguments for the identities show command.
#[derive(Args)]
pub struct ShowIdentityArgs {
    /// The name of the identity to be displayed.
    pub name: String,
}

/// The arguments for the identities remove command.
#[derive(Args)]
pub struct RemoveIdentityArgs {
    /// The name of the identity to be removed.
    pub name: String,
}

/// The network command.
#[derive(Subcommand)]
pub enum NetworksCommand {
    /// Add a network configuration.
    Add(AddNetworkArgs),

    /// Lists all the available networks.
    List,

    /// Edit a network file.
    Edit(EditNetworkArgs),

    /// Display a network's configuration.
    Show(ShowNetworkArgs),

    /// Removes a network.
    Remove(RemoveNetworkArgs),
}

/// The arguments for the network add command.
#[derive(Args)]
pub struct AddNetworkArgs {
    /// The name of the network to be added.
    pub name: String,

    /// The bootnode endpoint. e.g. https://example.com:1234
    pub bootnode: String,

    /// The nilchain RPC endpoint. e.g. http://example.com/
    #[arg(short = 'r', long)]
    pub nilchain_rpc_endpoint: Option<String>,

    /// The nilchain gRPC endpoint. e.g. http://example.com/
    #[arg(short = 'g', long, requires = "nilchain_rpc_endpoint")]
    pub nilchain_grpc_endpoint: Option<String>,

    /// The private key to be used for nilchain payments.
    #[arg(short = 'p', long, env = "NILCHAIN_PRIVATE_KEY", requires = "nilchain_rpc_endpoint")]
    pub nilchain_private_key: Option<String>,

    /// The nilchain chain id.
    #[arg(long, requires = "nilchain_rpc_endpoint")]
    pub nilchain_chain_id: Option<String>,

    /// The gas price to use in nilchain transactions.
    #[arg(long, requires = "nilchain_rpc_endpoint")]
    pub nilchain_gas_price: Option<f64>,
}

/// The arguments for the network edit command.
#[derive(Args)]
pub struct EditNetworkArgs {
    /// The name of the network to be edited.
    pub name: String,
}

/// The arguments for the network show command.
#[derive(Args)]
pub struct ShowNetworkArgs {
    /// The name of the network to be displayed.
    pub name: String,
}

/// The arguments for the network gen command.
#[derive(Args)]
pub struct RemoveNetworkArgs {
    /// The name of the network to be removed.
    pub name: String,
}

/// The context command.
#[derive(Subcommand)]
pub enum ContextCommand {
    /// Use a context in upcoming command invocations.
    Use(UseContextArgs),

    /// Show the current context.
    Show,
}

/// The arguments for the context use command.
#[derive(Args)]
pub struct UseContextArgs {
    /// The identity to be used.
    pub identity: String,

    /// The network to be used.
    pub network: String,
}

/// The arguments for the store value command.
#[derive(Args)]
pub struct StoreValuesArgs {
    /// The values to be stored.
    #[clap(flatten)]
    values: NadaValueArgs,

    /// The time to live for the values in days. If not set, then will default to TTL set by the cluster.
    #[clap(short, long)]
    pub ttl_days: Option<u32>,

    /// The program id that the store is for, if any.
    #[clap(long)]
    pub program_id: Option<String>,

    /// Give execution access to this user on the secret we're uploading.
    #[clap(long)]
    pub authorize_user_execution: Vec<String>,

    /// Sets the update identifier. When this is provided, this will cause the values already stored in the network with the given identifier to be updated.
    #[clap(long)]
    pub update_identifier: Option<Uuid>,

    /// Only get a price quote for the operation.
    #[clap(long)]
    pub quote: bool,
}

impl StoreValuesArgs {
    /// Collect all secrets.
    pub fn values(&self) -> Result<HashMap<String, NadaValue<Clear>>, Error> {
        self.values.parse()
    }
}

/// The arguments for the retrieve values command.
#[derive(Args)]
pub struct RetrieveValuesArgs {
    /// The values id to retrieve.
    pub values_id: Uuid,

    /// Only get a price quote for the operation.
    #[clap(long)]
    pub quote: bool,
}

// The arguments for the store program command.
#[derive(Args)]
pub struct StoreProgramArgs {
    // The path to the program's bytecode.
    pub program_path: PathBuf,

    /// The name of the program.
    pub program_name: String,

    /// Only get a price quote for the operation.
    #[clap(long)]
    pub quote: bool,
}

// The arguments for the compute command.
#[derive(Args)]
pub struct ComputeArgs {
    /// The id of the program to be run.
    pub program_id: String,

    /// The value ids to be used as parameters to the compute operation.
    #[clap(long = "value-id")]
    pub value_ids: Vec<Uuid>,

    /// The input bindings.
    ///
    /// These must follow the format `<party_name>=<user_id>`.
    #[clap(long = "input-binding")]
    pub input_bindings: Vec<UserBinding>,

    /// The output bindings.
    ///
    /// These must follow the format `<party_name>=<user_id>`.
    #[clap(long = "output-binding")]
    pub output_bindings: Vec<UserBinding>,

    /// The compute-time values to use.
    #[clap(flatten)]
    pub values: NadaValueArgs,

    /// Only get a price quote for the operation.
    #[clap(long)]
    pub quote: bool,
}

impl ComputeArgs {
    /// Collect all compute values.
    pub fn values(&self) -> Result<HashMap<String, NadaValue<Clear>>, Error> {
        self.values.parse()
    }
}

/// A binding for a compute operation.
#[derive(Debug, Clone)]
pub struct UserBinding {
    /// The name of the input/output being bound.
    pub name: String,

    /// The user id being bound.
    pub user_id: UserId,
}

impl FromStr for UserBinding {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (name, user_id) =
            s.split_once('=').ok_or_else(|| anyhow!("binding not in '<party>=<user-id>' format: '{s}'"))?;
        let user_id = user_id.parse()?;
        Ok(Self { name: name.into(), user_id })
    }
}

// The delete-values command arguments.
#[derive(Args)]
pub struct DeleteValuesArgs {
    /// The values id to delete.
    pub values_id: Uuid,
}

// The preprocessing pool status command arguments.
#[derive(Args)]
pub struct PreprocessingPoolStatusArgs {
    /// Only get a price quote for the operation.
    #[clap(long)]
    pub quote: bool,
}

#[derive(Args)]
pub struct RetrievePermissionsArgs {
    /// The values id permissions to retrieve.
    pub values_id: Uuid,

    /// Only get a price quote for the operation.
    #[clap(long)]
    pub quote: bool,
}

#[derive(Args)]
pub struct OverwritePermissionsArgs {
    /// The values id permissions to replace.
    #[clap(short, long)]
    pub values_id: Uuid,

    /// Path to a yaml or json file containing new permissions.
    pub permissions_path: String,

    /// Only get a price quote for the operation.
    #[clap(long)]
    pub quote: bool,
}

#[derive(Args)]
pub struct UpdatePermissionsArgs {
    /// The values id permissions to change.
    #[clap(short, long)]
    pub values_id: Uuid,

    /// Path to a yaml or json file containing permissions delta.
    #[clap(long)]
    pub permissions_path: Option<String>,

    /// Grant and revoke actions for different permissions.
    #[clap(flatten)]
    pub permission_actions: PermissionActionArgs,

    /// Only get a price quote for the operation.
    #[clap(long)]
    pub quote: bool,
}

#[serde_as]
#[derive(Clone, Default, Deserialize, Args)]
pub struct PermissionActionArgs {
    /// Users to grant retrieve permissions.
    #[serde(default)]
    #[serde_as(as = "Vec<DisplayFromStr>")]
    #[clap(long, num_args(1..))]
    pub grant_retrieve: Vec<UserId>,

    /// Users to revoke retrieve permissions.
    #[serde(default)]
    #[serde_as(as = "Vec<DisplayFromStr>")]
    #[clap(long, num_args(1..))]
    pub revoke_retrieve: Vec<UserId>,

    /// Users to grant update permissions.
    #[serde(default)]
    #[serde_as(as = "Vec<DisplayFromStr>")]
    #[clap(long, num_args(1..))]
    pub grant_update: Vec<UserId>,

    /// Users to revoke update permissions.
    #[serde(default)]
    #[serde_as(as = "Vec<DisplayFromStr>")]
    #[clap(long, num_args(1..))]
    pub revoke_update: Vec<UserId>,

    /// Users to grant delete permissions.
    #[serde(default)]
    #[serde_as(as = "Vec<DisplayFromStr>")]
    #[clap(long, num_args(1..))]
    pub grant_delete: Vec<UserId>,

    /// Users to revoke delete permissions.
    #[serde(default)]
    #[serde_as(as = "Vec<DisplayFromStr>")]
    #[clap(long, num_args(1..))]
    pub revoke_delete: Vec<UserId>,

    /// The list of users with program IDs for granting compute permissions. Format: `<UserId>=<ProgramId1>,<ProgramId2>`.
    #[serde(default)]
    #[serde_as(as = "Vec<(DisplayFromStr, Vec<DisplayFromStr>)>")]
    #[clap(long, num_args(1..), value_parser(parse_tuple_from_str))]
    pub grant_compute: Vec<(UserId, Vec<String>)>,

    /// The list of users with program IDs for revoking compute permissions. Format: `<UserId>=<ProgramId1>,<ProgramId2>`.
    #[serde(default)]
    #[serde_as(as = "Vec<(DisplayFromStr, Vec<DisplayFromStr>)>")]
    #[clap(long, num_args(1..), value_parser(parse_tuple_from_str))]
    pub revoke_compute: Vec<(UserId, Vec<String>)>,
}

/// The balance command.
#[derive(Subcommand)]
pub enum BalanceCommand {
    /// Show the user's account balance.
    Show,

    /// Add funds.
    AddFunds(AddFundsArgs),
}

/// Add funds arguments.
#[derive(Args)]
pub struct AddFundsArgs {
    /// The recipient of the funds, defaults to ourselves if not set.
    #[clap(long)]
    pub recipient: Option<UserId>,

    /// The amount in unil.
    pub amount: u64,
}

/// The config command.
#[derive(Subcommand)]
pub enum ConfigCommand {
    /// Get the payments configuration.
    Payments,

    /// Get the cluster configuration
    Cluster(ClusterConfigArgs),
}

/// Cluster configuration arguments.
#[derive(Args)]
pub struct ClusterConfigArgs {
    /// Make the request for the cluster configuration to this specific node.
    #[clap(long)]
    pub node_id: Option<NodeId>,
}

/// Helper function for CLI to Parse a tuple from a string
fn parse_tuple_from_str(s: &str) -> Result<(UserId, Vec<String>), String> {
    let parts: Vec<&str> = s.split('=').collect();
    if parts.len() != 2 {
        return Err("Format must be <UserId>=<ProgramId1,ProgramId2>".into());
    }
    let key = parts[0].parse::<UserId>().map_err(|_| "Invalid UserId format")?;
    let values = parts[1].split(',').map(|s| s.to_string()).collect();

    Ok((key, values))
}

fn default_config_path() -> PathBuf {
    let Some(config_root) = config_directory() else {
        Cli::command().error(ErrorKind::Io, "no configuration directory found").exit();
    };
    config_root.join("nillion-cli.yaml")
}

#[cfg(test)]
mod test {
    use super::Cli;

    #[test]
    fn verify_cli() {
        use clap::CommandFactory;
        Cli::command().debug_assert();
    }
}

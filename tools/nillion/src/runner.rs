use crate::{
    args::{
        AddFundsArgs, AddIdentityArgs, AddNetworkArgs, BalanceCommand, Cli, ClusterConfigArgs, Command, ComputeArgs,
        ConfigCommand, DeleteValuesArgs, EditIdentityArgs, EditNetworkArgs, IdentityGenArgs, OverwritePermissionsArgs,
        PreprocessingPoolStatusArgs, RemoveIdentityArgs, RemoveNetworkArgs, RetrievePermissionsArgs,
        RetrieveValuesArgs, ShowIdentityArgs, ShowNetworkArgs, StoreProgramArgs, StoreValuesArgs,
        UpdatePermissionsArgs, UseContextArgs,
    },
    context::ContextConfig,
    parse_input_file,
    serialize::{NoOutput, SerializeAsAny},
    wrappers::{ClusterInfo, PermissionsDelta, PreprocessingPoolStatus, PrettyValue, UserFriendlyPermissions},
};
use anyhow::{anyhow, bail, Context, Result};
use chrono::{DateTime, Utc};
use clap::{error::ErrorKind, CommandFactory};
use clap_utils::shell_completions::{handle_shell_completions, ShellCompletionsArgs};
use log::{debug, info};
use nillion_client::{
    grpc::payments::AccountBalanceResponse,
    operation::{InitialState, PaidOperation, PaidVmOperation},
    payments::TxHash,
    vm::VmClient,
    Ed25519SigningKey, Secp256k1SigningKey, TokenAmount, UserId,
};
use serde::Serialize;
use serde_with::{serde_as, DisplayFromStr};
use std::{
    collections::{BTreeMap, HashMap},
    env,
    fs::{self},
    path::Path,
};
use tools_config::{
    identities::{Identity, Kind},
    networks::{NetworkConfig, PaymentsConfig},
    NamedConfig, ToolConfig,
};
use user_keypair::SigningKey;
use uuid::Uuid;

pub struct Runner {
    client: VmClient,
}

impl Runner {
    pub fn new(client: VmClient) -> Self {
        Self { client }
    }

    /// run a command
    pub async fn run(&self, command: Command) -> Result<Box<dyn SerializeAsAny>> {
        match command {
            Command::ClusterInformation => self.cluster_information().await,
            Command::Compute(args) => self.compute(args).await,
            Command::DeleteValues(args) => self.delete_values(args).await,
            Command::InspectIds => self.inspect_ids(),
            Command::PreprocessingPoolStatus(args) => self.preprocessing_pool_status(args).await,
            Command::RetrievePermissions(args) => self.retrieve_permissions(args).await,
            Command::RetrieveValues(args) => self.retrieve_value(args).await,
            Command::OverwritePermissions(args) => self.overwrite_permissions(args).await,
            Command::UpdatePermissions(args) => self.update_permissions(args).await,
            Command::ShellCompletions(args) => self.handle_shell_completions(args),
            Command::StoreProgram(args) => self.store_program(args).await,
            Command::StoreValues(args) => self.store_values(args).await,
            Command::IdentityGen(_) | Command::Identities(_) | Command::Networks(_) | Command::Context(_) => {
                unreachable!("handled in main")
            }
            Command::Balance(BalanceCommand::Show) => self.show_balance().await,
            Command::Balance(BalanceCommand::AddFunds(args)) => self.add_funds(args).await,
            Command::Config(ConfigCommand::Payments) => self.payments_config().await,
            Command::Config(ConfigCommand::Cluster(args)) => self.cluster_config(args).await,
        }
    }

    pub fn handle_shell_completions(&self, args: ShellCompletionsArgs) -> Result<Box<dyn SerializeAsAny>> {
        handle_shell_completions(args, &mut Cli::command());
        Ok(Box::new(NoOutput))
    }

    pub async fn store_values(&self, args: StoreValuesArgs) -> Result<Box<dyn SerializeAsAny>> {
        #[derive(Serialize)]
        struct Output {
            values_id: Uuid,
        }

        let values = match args.values() {
            Ok(values) => values,
            Err(e) => {
                let mut cmd = Cli::command();
                cmd.error(ErrorKind::ValueValidation, format!("Invalid values encoding: {e}")).exit();
            }
        };
        if values.is_empty() {
            bail!("need at least one secret to store");
        }
        if args.authorize_user_execution.is_empty() ^ args.program_id.is_none() {
            bail!("authorize-user-execution and program-id must both be set");
        }

        let mut builder = self.client.store_values().add_values(values);
        if let Some(ttl_days) = args.ttl_days {
            builder = builder.ttl_days(ttl_days);
        }
        if let Some(program_id) = &args.program_id {
            for user_id in args.authorize_user_execution {
                builder = builder.allow_compute(user_id.parse().context("invalid user id")?, program_id.clone());
            }
        }
        if let Some(identifier) = args.update_identifier {
            builder = builder.update_identifier(identifier);
        }
        let operation = builder.build()?;
        if args.quote {
            return self.serialize_quote(operation).await;
        }
        let values_id = operation.invoke().await?;
        Ok(Box::new(Output { values_id }))
    }

    pub async fn retrieve_value(&self, args: RetrieveValuesArgs) -> Result<Box<dyn SerializeAsAny>> {
        let operation = self.client.retrieve_values().values_id(args.values_id).build()?;
        if args.quote {
            return self.serialize_quote(operation).await;
        }
        let values = operation.invoke().await?;
        let values: HashMap<_, _> = values.into_iter().map(|(k, v)| (k, PrettyValue::from(v))).collect();
        Ok(Box::new(values))
    }

    pub async fn retrieve_permissions(&self, args: RetrievePermissionsArgs) -> Result<Box<dyn SerializeAsAny>> {
        let operation = self.client.retrieve_permissions().values_id(args.values_id).build()?;
        if args.quote {
            return self.serialize_quote(operation).await;
        }
        let permissions: UserFriendlyPermissions = (&operation.invoke().await?).into();
        Ok(Box::new(permissions))
    }

    pub async fn overwrite_permissions(&self, args: OverwritePermissionsArgs) -> Result<Box<dyn SerializeAsAny>> {
        let path = Path::new(&args.permissions_path);
        if !path.exists() {
            return Err(anyhow!(format!("cannot load file: {:?}", path)));
        }

        let permissions: UserFriendlyPermissions =
            parse_input_file(path).context("failed to parse permissions file")?;

        let mut builder = self.client.overwrite_permissions().values_id(args.values_id);
        for user in permissions.retrieve {
            builder = builder.allow_retrieve(user);
        }
        for user in permissions.delete {
            builder = builder.allow_delete(user);
        }
        for user in permissions.update {
            builder = builder.allow_update(user);
        }
        for (user, programs) in permissions.compute {
            for program in programs {
                builder = builder.allow_compute(user, program);
            }
        }
        let operation = builder.build()?;
        if args.quote {
            return self.serialize_quote(operation).await;
        }
        operation.invoke().await?;
        Ok(Box::new(format!("Permissions for values {} have been overwritten", args.values_id)))
    }

    pub async fn update_permissions(&self, args: UpdatePermissionsArgs) -> Result<Box<dyn SerializeAsAny>> {
        let mut permissions_delta = PermissionsDelta::default();

        // get permissions from file
        if let Some(path) = args.permissions_path {
            let path = Path::new(&path);
            let file_permissions: PermissionsDelta =
                parse_input_file(path).context("Failed to parse permissions delta file")?;
            permissions_delta = file_permissions;
        }

        // merge permissions from CLI
        permissions_delta.merge(args.permission_actions.into());

        if permissions_delta.is_empty() {
            bail!("no permissions to update");
        }

        let mut builder = self.client.update_permissions().values_id(args.values_id);

        // retrieve permissions
        for user in permissions_delta.retrieve.grant {
            builder = builder.grant_retrieve(user);
        }
        for user in permissions_delta.retrieve.revoke {
            builder = builder.revoke_retrieve(user);
        }

        // update permissions
        for user in permissions_delta.update.grant {
            builder = builder.grant_update(user);
        }
        for user in permissions_delta.update.revoke {
            builder = builder.revoke_update(user);
        }

        // delete permissions
        for user in permissions_delta.delete.grant {
            builder = builder.grant_delete(user);
        }
        for user in permissions_delta.delete.revoke {
            builder = builder.revoke_delete(user);
        }

        // compute permissions
        for (user, programs) in permissions_delta.compute.grant {
            for program in programs {
                builder = builder.grant_compute(user, program);
            }
        }
        for (user, programs) in permissions_delta.compute.revoke {
            for program in programs {
                builder = builder.revoke_compute(user, program);
            }
        }

        let operation = builder.build()?;
        if args.quote {
            return self.serialize_quote(operation).await;
        }
        operation.invoke().await?;
        Ok(Box::new(format!("Permissions for values {} have been updated", args.values_id)))
    }

    pub async fn store_program(&self, args: StoreProgramArgs) -> Result<Box<dyn SerializeAsAny>> {
        #[derive(Serialize)]
        struct Output {
            program_id: String,
        }

        let raw_mir = fs::read(args.program_path.clone())
            .context(format!("program not found: {}", args.program_path.to_string_lossy()))?;
        let program_name = args.program_name;
        debug!("Storing program {program_name}, raw_mir size: {}", raw_mir.len());
        let operation = self.client.store_program().name(program_name).program(raw_mir).build()?;
        if args.quote {
            return self.serialize_quote(operation).await;
        }
        let program_id = operation.invoke().await?;
        Ok(Box::new(Output { program_id }))
    }

    pub async fn compute(&self, args: ComputeArgs) -> Result<Box<dyn SerializeAsAny>> {
        let values = args.values()?;
        let mut builder =
            self.client.invoke_compute().program_id(args.program_id).add_value_ids(args.value_ids).add_values(values);
        for binding in args.input_bindings {
            builder = builder.bind_input_party(binding.name, binding.user_id);
        }
        let mut wait_for_result = false;
        for binding in args.output_bindings {
            let user_id = binding.user_id;
            wait_for_result = wait_for_result || user_id == self.client.user_id();
            builder = builder.bind_output_party(binding.name, [user_id]);
        }
        let operation = builder.build()?;
        if args.quote {
            return self.serialize_quote(operation).await;
        }
        let compute_id = operation.invoke().await?;

        #[derive(Serialize)]
        struct ComputeResult {
            compute_id: String,
            #[serde(skip_serializing_if = "Option::is_none")]
            outputs: Option<HashMap<String, PrettyValue>>,
        }

        let mut compute_result = ComputeResult { compute_id: compute_id.to_string(), outputs: None };

        info!("Computation invoked using id {compute_id}");
        // Don't wait for results if we're not an output party
        if !wait_for_result {
            return Ok(Box::new(compute_result));
        }

        info!("Waiting for compute result...");
        let outputs = self.client.retrieve_compute_results().compute_id(compute_id).build()?.invoke().await??;
        let outputs = outputs
            .into_iter()
            .map(|(name, value)| (name, PrettyValue::from(value.clone())))
            .collect::<HashMap<_, _>>();

        compute_result.outputs = Some(outputs);

        Ok(Box::new(compute_result))
    }

    pub async fn delete_values(&self, args: DeleteValuesArgs) -> Result<Box<dyn SerializeAsAny>> {
        self.client
            .delete_values()
            .values_id(args.values_id)
            .build()?
            .invoke()
            .await
            .context("failed to delete values")?;

        Ok(Box::new(format!("Values {} have been deleted", args.values_id)))
    }

    pub async fn cluster_information(&self) -> Result<Box<dyn SerializeAsAny>> {
        let info: ClusterInfo = self.client.cluster().into();
        Ok(Box::new(info))
    }

    pub async fn preprocessing_pool_status(
        &self,
        args: PreprocessingPoolStatusArgs,
    ) -> Result<Box<dyn SerializeAsAny>> {
        let operation = self.client.pool_status();
        if args.quote {
            return self.serialize_quote(operation).await;
        }
        let response = operation.invoke().await.context("querying preprocessing pool status")?;
        let status = PreprocessingPoolStatus {
            offsets: response.offsets.into_iter().map(|(element, range)| (format!("{element:?}"), range)).collect(),
            auxiliary_material_available: response.auxiliary_material_available,
            preprocessing_active: response.preprocessing_active,
        };
        Ok(Box::new(status))
    }

    pub fn inspect_ids(&self) -> Result<Box<dyn SerializeAsAny>> {
        #[serde_as]
        #[derive(Serialize)]
        struct Output {
            #[serde_as(as = "DisplayFromStr")]
            user_id: UserId,
        }
        let user_id = self.client.user_id();
        Ok(Box::new(Output { user_id }))
    }

    fn generate_key(seed: Option<String>, curve: &Kind) -> Result<SigningKey> {
        let key = match (seed, curve) {
            (Some(seed), Kind::Ed25519) => {
                info!("Generating ed25519 key using provided seed");
                Ed25519SigningKey::from_seed(&seed).into()
            }
            (None, Kind::Ed25519) => {
                info!("Generating random ed25519 key");
                Ed25519SigningKey::generate().into()
            }
            (Some(seed), Kind::Secp256k1) => {
                info!("Generating secp256k1 key using provided seed");
                Secp256k1SigningKey::try_from_seed(&seed)?.into()
            }
            (None, Kind::Secp256k1) => {
                info!("Generating random secp256k1 key");
                Secp256k1SigningKey::generate().into()
            }
        };
        Ok(key)
    }

    pub fn identities_gen(args: IdentityGenArgs) -> Result<Box<dyn SerializeAsAny>> {
        info!("Generating user identities");
        let user_key = Self::generate_key(args.seed, &args.curve)?.as_bytes();
        let identity = Identity { private_key: user_key, kind: args.curve };
        identity.write_to_file(&args.name)?;
        Ok(Box::new(format!("Identity {} generated", args.name)))
    }

    pub fn add_identity(args: AddIdentityArgs) -> Result<Box<dyn SerializeAsAny>> {
        let kind = Kind::Secp256k1;
        let user_key = Self::generate_key(args.seed, &kind)?.as_bytes();
        let identity = Identity { private_key: user_key, kind };
        identity.write_to_file(&args.name)?;
        Ok(Box::new(format!("Identity {} added", args.name)))
    }

    pub fn list_identities() -> Result<Box<dyn SerializeAsAny>> {
        #[derive(Serialize)]
        struct Output {
            identities: Vec<String>,
        }

        let identities = Identity::read_all()?;
        let identities = identities.into_iter().map(|NamedConfig { name, .. }| name).collect::<Vec<_>>();
        Ok(Box::new(Output { identities }))
    }

    pub fn edit_identity(args: EditIdentityArgs) -> Result<Box<dyn SerializeAsAny>> {
        let EditIdentityArgs { name } = args;
        let path = Identity::config_path(&name)?;
        if !fs::exists(&path).unwrap_or(false) {
            bail!("identity file does not exist");
        }
        Self::open_in_editor(&path)?;
        Ok(Box::new(NoOutput))
    }

    pub fn show_identity(args: ShowIdentityArgs) -> Result<Box<dyn SerializeAsAny>> {
        #[serde_as]
        #[derive(Serialize)]
        struct Output {
            #[serde_as(as = "DisplayFromStr")]
            user_id: UserId,

            #[serde(serialize_with = "hex::serde::serialize")]
            public_key: Vec<u8>,

            #[serde_as(as = "DisplayFromStr")]
            kind: Kind,
        }

        let Identity { private_key: user_key, kind } = Identity::read_from_config(&args.name)?;
        let private_key = match kind {
            Kind::Ed25519 => SigningKey::from(Ed25519SigningKey::try_from(user_key.as_ref())?),
            Kind::Secp256k1 => SigningKey::from(Secp256k1SigningKey::try_from(user_key.as_ref())?),
        };
        let public_key = private_key.public_key().as_bytes();
        let user_id = UserId::from_bytes(&public_key);
        Ok(Box::new(Output { public_key, user_id, kind }))
    }

    pub fn remove_identity(args: RemoveIdentityArgs) -> Result<Box<dyn SerializeAsAny>> {
        Identity::remove_config(&args.name)?;
        Ok(Box::new(format!("Identity {} removed", args.name)))
    }

    pub fn add_network(args: AddNetworkArgs) -> Result<Box<dyn SerializeAsAny>> {
        let AddNetworkArgs {
            name,
            bootnode,
            nilchain_rpc_endpoint,
            nilchain_grpc_endpoint,
            nilchain_private_key,
            nilchain_chain_id,
            nilchain_gas_price,
        } = args;
        let payments = nilchain_rpc_endpoint.map(|nilchain_rpc_endpoint| PaymentsConfig {
            nilchain_chain_id,
            nilchain_rpc_endpoint,
            nilchain_grpc_endpoint,
            // Validation is applied via the clap arguments definition
            nilchain_private_key: nilchain_private_key.expect("private key not set"),
            gas_price: nilchain_gas_price,
        });
        NetworkConfig { bootnode, payments }.write_to_file(&name)?;
        Ok(Box::new(format!("Network {} added", name)))
    }

    pub fn edit_network(args: EditNetworkArgs) -> Result<Box<dyn SerializeAsAny>> {
        let EditNetworkArgs { name } = args;
        let path = NetworkConfig::config_path(&name)?;
        if !fs::exists(&path).unwrap_or(false) {
            bail!("network file does not exist");
        }
        Self::open_in_editor(&path)?;
        Ok(Box::new(NoOutput))
    }

    pub fn list_networks() -> Result<Box<dyn SerializeAsAny>> {
        #[derive(Serialize)]
        struct Output {
            networks: Vec<String>,
        }

        let configs = NetworkConfig::read_all()?;

        let networks = configs.into_iter().map(|NamedConfig { name, .. }| name).collect::<Vec<_>>();
        Ok(Box::new(Output { networks }))
    }

    pub fn show_network(args: ShowNetworkArgs) -> Result<Box<dyn SerializeAsAny>> {
        let config = NetworkConfig::read_from_config(&args.name)?;
        let NetworkConfig { bootnode, payments } = config;
        let mut output = BTreeMap::from([("bootnode", bootnode)]);

        if let Some(payments) = payments {
            let PaymentsConfig {
                nilchain_rpc_endpoint,
                nilchain_grpc_endpoint,
                gas_price,
                nilchain_private_key,
                nilchain_chain_id,
            } = payments;
            let nilchain_private_key: String = nilchain_private_key.chars().take(3).collect();
            output.insert("nilchain_rpc_endpoint", nilchain_rpc_endpoint);
            if let Some(grpc_endpoint) = nilchain_grpc_endpoint {
                output.insert("nilchain_grpc_endpoint", grpc_endpoint);
            }
            output.insert("nilchain_private_key", nilchain_private_key);
            if let Some(chain_id) = nilchain_chain_id {
                output.insert("nilchain_chain_id", chain_id);
            }
            if let Some(gas_price) = gas_price {
                output.insert("nilchain_gas_price", gas_price.to_string());
            }
        }
        Ok(Box::new(output))
    }

    pub fn remove_network(args: RemoveNetworkArgs) -> Result<Box<dyn SerializeAsAny>> {
        NetworkConfig::remove_config(&args.name)?;
        Ok(Box::new(format!("Network {} removed", args.name)))
    }

    pub fn use_context(args: UseContextArgs) -> Result<Box<dyn SerializeAsAny>> {
        #[derive(Serialize)]
        struct Output {
            identity: String,
            network: String,
        }

        let UseContextArgs { identity, network } = args;
        if let Err(e) = Identity::read_from_config(&identity) {
            bail!("Invalid identity: {e}");
        }
        if let Err(e) = NetworkConfig::read_from_config(&network) {
            bail!("Invalid network: {e}");
        }
        let config = ContextConfig { identity: identity.clone(), network: network.clone() };
        config.store()?;
        Ok(Box::new(Output { identity, network }))
    }

    pub fn show_context() -> Result<Box<dyn SerializeAsAny>> {
        #[derive(Serialize)]
        struct Output {
            identity: String,
            network: String,
        }

        let Some(context) = ContextConfig::load() else {
            return Ok(Box::new(HashMap::<(), ()>::new()));
        };
        let ContextConfig { identity, network } = context;
        Ok(Box::new(Output { identity, network }))
    }

    pub fn open_in_editor(path: &Path) -> Result<()> {
        // Use the editor specified in VISUAL, otherwise EDITOR, otherwise default to vim.
        let editor = env::var("VISUAL").or_else(|_| env::var("EDITOR")).unwrap_or_else(|_| "vim".into());
        let mut child = std::process::Command::new(&editor)
            .arg(path)
            .spawn()
            .map_err(|e| anyhow!("failed to run {editor}: {e}"))?;
        child.wait().map_err(|e| anyhow!("failed to wait for {editor}: {e}"))?;
        Ok(())
    }

    pub async fn show_balance(&self) -> Result<Box<dyn SerializeAsAny>> {
        #[derive(Serialize)]
        struct Output {
            balance: u64,
            last_updated_at: DateTime<Utc>,
            expires_at: DateTime<Utc>,
        }
        let balance = self.client.account_balance().await?;
        let AccountBalanceResponse { balance, last_updated, expires_at } = balance;
        let output = Output { balance, last_updated_at: last_updated, expires_at };
        Ok(Box::new(output))
    }

    pub async fn add_funds(&self, args: AddFundsArgs) -> Result<Box<dyn SerializeAsAny>> {
        #[serde_as]
        #[derive(Serialize)]
        struct Output {
            #[serde_as(as = "DisplayFromStr")]
            tx_hash: TxHash,
        }

        let AddFundsArgs { recipient, amount } = args;
        let mut builder = self.client.add_funds().amount(TokenAmount::Unil(amount));
        if let Some(recipient) = recipient {
            builder = builder.recipient(recipient);
        }
        let tx_hash = builder.build()?.invoke().await?;
        Ok(Box::new(Output { tx_hash }))
    }

    async fn payments_config(&self) -> Result<Box<dyn SerializeAsAny>> {
        #[derive(Serialize)]
        struct Output {
            minimum_add_funds_payment_unil: u64,
            credits_per_nil: u64,
        }

        let config = self.client.payments_config().await?;
        Ok(Box::new(Output {
            minimum_add_funds_payment_unil: config.minimum_add_funds_payment,
            credits_per_nil: config.credits_per_nil,
        }))
    }

    async fn cluster_config(&self, args: ClusterConfigArgs) -> Result<Box<dyn SerializeAsAny>> {
        let cluster = match args.node_id {
            Some(node_id) => {
                let clients =
                    self.client.node_clients(node_id).ok_or_else(|| anyhow!("node is not part of cluster"))?;
                clients.membership.cluster().await.context("fetching cluster config")?
            }
            None => self.client.cluster().clone(),
        };
        let info = ClusterInfo::from(&cluster);
        Ok(Box::new(info))
    }

    async fn serialize_quote<'a, O: PaidVmOperation>(
        &self,
        operation: PaidOperation<'a, O, InitialState>,
    ) -> Result<Box<dyn SerializeAsAny>> {
        #[derive(Serialize)]
        struct Output {
            tokens: u64,
            credits: u64,
        }
        let operation = operation.quote().await?;
        let fees = operation.fees();
        Ok(Box::new(Output { tokens: fees.tokens, credits: fees.credits }))
    }
}

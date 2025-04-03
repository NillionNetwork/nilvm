use super::HandlerResult;
use crate::{
    args::{
        AddFundsArgs, BalanceCommand, Cli, ClusterConfigArgs, Command, ComputeArgs, ConfigCommand, DeleteValuesArgs,
        OverwritePermissionsArgs, PreprocessingPoolStatusArgs, RetrievePermissionsArgs, RetrieveValuesArgs,
        StoreProgramArgs, StoreValuesArgs, UpdatePermissionsArgs,
    },
    parse_input_file,
    serialize::NoOutput,
    wrappers::{ClusterInfo, PermissionsDelta, PreprocessingPoolStatus, PrettyValue, UserFriendlyPermissions},
};
use anyhow::{anyhow, bail, Context};
use chrono::{DateTime, Utc};
use clap::{error::ErrorKind, CommandFactory};
use clap_utils::shell_completions::{handle_shell_completions, ShellCompletionsArgs};
use log::{debug, info};
use nillion_client::{
    grpc::payments::AccountBalanceResponse,
    operation::{InitialState, PaidOperation, PaidVmOperation},
    payments::TxHash,
    vm::VmClient,
    TokenAmount, UserId,
};
use serde::Serialize;
use serde_with::{serde_as, DisplayFromStr};
use std::{
    collections::HashMap,
    fs::{self},
    path::Path,
};
use uuid::Uuid;

pub struct NilvmHandler {
    client: VmClient,
}

impl NilvmHandler {
    pub fn new(client: VmClient) -> Self {
        Self { client }
    }

    /// run a command
    pub async fn run(&self, command: Command) -> HandlerResult {
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
            Command::Balance(BalanceCommand::Show) => self.show_balance().await,
            Command::Balance(BalanceCommand::AddFunds(args)) => self.add_funds(args).await,
            Command::Config(ConfigCommand::Payments) => self.payments_config().await,
            Command::Config(ConfigCommand::Cluster(args)) => self.cluster_config(args).await,
            Command::Nilauth(_)
            | Command::IdentityGen(_)
            | Command::Identities(_)
            | Command::Networks(_)
            | Command::Context(_)
            | Command::Nuc(_) => {
                unreachable!("handled in main")
            }
        }
    }

    pub fn handle_shell_completions(&self, args: ShellCompletionsArgs) -> HandlerResult {
        handle_shell_completions(args, &mut Cli::command());
        Ok(Box::new(NoOutput))
    }

    pub async fn store_values(&self, args: StoreValuesArgs) -> HandlerResult {
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

    pub async fn retrieve_value(&self, args: RetrieveValuesArgs) -> HandlerResult {
        let operation = self.client.retrieve_values().values_id(args.values_id).build()?;
        if args.quote {
            return self.serialize_quote(operation).await;
        }
        let values = operation.invoke().await?;
        let values: HashMap<_, _> = values.into_iter().map(|(k, v)| (k, PrettyValue::from(v))).collect();
        Ok(Box::new(values))
    }

    pub async fn retrieve_permissions(&self, args: RetrievePermissionsArgs) -> HandlerResult {
        let operation = self.client.retrieve_permissions().values_id(args.values_id).build()?;
        if args.quote {
            return self.serialize_quote(operation).await;
        }
        let permissions: UserFriendlyPermissions = (&operation.invoke().await?).into();
        Ok(Box::new(permissions))
    }

    pub async fn overwrite_permissions(&self, args: OverwritePermissionsArgs) -> HandlerResult {
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

    pub async fn update_permissions(&self, args: UpdatePermissionsArgs) -> HandlerResult {
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

    pub async fn store_program(&self, args: StoreProgramArgs) -> HandlerResult {
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

    pub async fn compute(&self, args: ComputeArgs) -> HandlerResult {
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

    pub async fn delete_values(&self, args: DeleteValuesArgs) -> HandlerResult {
        self.client
            .delete_values()
            .values_id(args.values_id)
            .build()?
            .invoke()
            .await
            .context("failed to delete values")?;

        Ok(Box::new(format!("Values {} have been deleted", args.values_id)))
    }

    pub async fn cluster_information(&self) -> HandlerResult {
        let info: ClusterInfo = self.client.cluster().into();
        Ok(Box::new(info))
    }

    pub async fn preprocessing_pool_status(&self, args: PreprocessingPoolStatusArgs) -> HandlerResult {
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

    pub fn inspect_ids(&self) -> HandlerResult {
        #[serde_as]
        #[derive(Serialize)]
        struct Output {
            #[serde_as(as = "DisplayFromStr")]
            user_id: UserId,
        }
        let user_id = self.client.user_id();
        Ok(Box::new(Output { user_id }))
    }

    pub async fn show_balance(&self) -> HandlerResult {
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

    pub async fn add_funds(&self, args: AddFundsArgs) -> HandlerResult {
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

    pub async fn payments_config(&self) -> HandlerResult {
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

    pub async fn cluster_config(&self, args: ClusterConfigArgs) -> HandlerResult {
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
    ) -> HandlerResult {
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

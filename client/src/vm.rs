//! The NilVm client.

use crate::{
    builder::VmClientBuilder,
    grpc::{
        ComputeClient, LeaderQueriesClient, MembershipClient, PaymentsClient, PermissionsClient, ProgramsClient,
        ValuesClient,
    },
    operation::{
        add_funds::AddFundsOperationBuilder, delete_values::DeleteValuesOperationBuilder,
        invoke_compute::InvokeComputeOperationBuilder, overwrite_permissions::OverwritePermissionsOperationBuilder,
        pool_status::PoolStatusOperation, retrieve_compute_results::RetrieveComputeResultsOperationBuilder,
        retrieve_permissions::RetrievePermissionsOperationBuilder, retrieve_values::RetrieveValuesOperationBuilder,
        store_program::StoreProgramOperationBuilder, store_values::StoreValuesOperationBuilder,
        update_permissions::UpdatePermissionsOperationBuilder, InvokeError, PaidOperation,
    },
    payments::NilChainPayer,
    UserId,
};
use grpc_channel::AuthenticatedGrpcChannel;
use math_lib::modular::EncodedModulo;
use nillion_client_core::values::{PartyId, SecretMasker};
use node_api::{
    membership::rust::{Cluster, NodeId, NodeVersion, Prime},
    payments::{proto::config::PaymentsConfigResponse, rust::AccountBalanceResponse},
};
use std::{collections::HashMap, sync::Arc};

/// The payment mode to use.
#[derive(Clone, Debug, Default)]
pub enum PaymentMode {
    /// Attempt to pay using the in-network balance first, falling back to a payment per operation
    /// if not enough funds are available.
    #[default]
    FromBalance,

    /// Pay on every operation.
    PayPerOperation,
}

pub(crate) struct VmClientConfig {
    pub(crate) channels: HashMap<PartyId, AuthenticatedGrpcChannel>,
    pub(crate) leader_channel: AuthenticatedGrpcChannel,
    pub(crate) nilchain_payer: Arc<dyn NilChainPayer>,
    pub(crate) cluster: Cluster,
    pub(crate) masker: SecretMasker,
    pub(crate) user_id: UserId,
    pub(crate) max_payload_size: usize,
    pub(crate) payment_mode: PaymentMode,
}

/// A client to interact with the NilVm.
#[derive(Clone)]
pub struct VmClient {
    pub(crate) payments: PaymentsClient,
    pub(crate) leader_queries: LeaderQueriesClient,
    pub(crate) clients: HashMap<PartyId, GrpcClients>,
    pub(crate) nilchain_payer: Arc<dyn NilChainPayer>,
    pub(crate) masker: SecretMasker,
    pub(crate) cluster: Cluster,
    pub(crate) user_id: UserId,
    pub(crate) payment_mode: PaymentMode,
    pub(crate) modulo: EncodedModulo,
}

impl VmClient {
    pub(crate) fn new(config: VmClientConfig) -> Self {
        let VmClientConfig {
            channels,
            leader_channel,
            nilchain_payer,
            cluster,
            masker,
            user_id,
            max_payload_size,
            payment_mode,
        } = config;
        let payments = PaymentsClient::new(leader_channel.clone());
        let leader_queries = LeaderQueriesClient::new(leader_channel.clone());
        let mut clients = HashMap::new();
        for (identity, channel) in channels {
            let member_clients = GrpcClients::new(channel, max_payload_size);
            clients.insert(identity, member_clients);
        }
        let modulo = match &cluster.prime {
            Prime::Safe64Bits => EncodedModulo::U64SafePrime,
            Prime::Safe128Bits => EncodedModulo::U128SafePrime,
            Prime::Safe256Bits => EncodedModulo::U256SafePrime,
        };
        Self { payments, leader_queries, clients, nilchain_payer, cluster, masker, user_id, payment_mode, modulo }
    }

    /// Create a builder for this client.
    pub fn builder() -> VmClientBuilder {
        VmClientBuilder::default()
    }

    /// Get the user id tied to this client.
    pub fn user_id(&self) -> UserId {
        self.user_id
    }

    /// Get the cluster that this client is targeting.
    pub fn cluster(&self) -> &Cluster {
        &self.cluster
    }

    /// Create a preprocessing pool status operation.
    ///
    /// See [PaidOperation] for more information.
    pub fn pool_status(&self) -> PaidOperation<'_, PoolStatusOperation> {
        PaidOperation::new(PoolStatusOperation, self)
    }

    /// Get the node's version.
    pub async fn node_version(&self, node_id: NodeId) -> Result<NodeVersion, InvokeError> {
        let party_id = PartyId::from(Vec::from(node_id));
        match self.clients.get(&party_id) {
            Some(client) => Ok(client.membership.node_version().await?),
            None => Err(InvokeError("node not party of cluster".into())),
        }
    }

    /// Delete values.
    ///
    /// This returns a builder to delete values from the network. All required attributes in the
    /// builder must be set before invoking the operation.
    ///
    /// ```ignore
    /// client.delete_values()
    ///       .values_id(values_id)
    ///       .build()?
    ///       .invoke()
    ///       .await?;
    /// ```
    pub fn delete_values(&self) -> DeleteValuesOperationBuilder<'_> {
        DeleteValuesOperationBuilder::new(self)
    }

    /// Retrieve values from the network.
    ///
    /// This returns a builder to retrieve values from the network. All required attributes in the
    /// builder must be set before invoking the operation.
    ///
    /// ```ignore
    /// client.retrieve_values()
    ///       .values_id(values_id)
    ///       .build()?
    ///       .invoke()
    ///       .await?;
    /// ```
    pub fn retrieve_values(&self) -> RetrieveValuesOperationBuilder<'_> {
        RetrieveValuesOperationBuilder::new(self)
    }

    /// Retrieve the permissions for a set of values.
    ///
    /// This returns a builder to retrieve permissions from the network. All required attributes in the
    /// builder must be set before invoking the operation.
    ///
    /// ```ignore
    /// client.retrieve_permissions()
    ///       .values_id(values_id)
    ///       .build()?
    ///       .invoke()
    ///       .await?;
    /// ```
    pub fn retrieve_permissions(&self) -> RetrievePermissionsOperationBuilder<'_> {
        RetrievePermissionsOperationBuilder::new(self)
    }

    /// Overwrite the permissions for a set of values.
    ///
    /// This returns a builder to overwrite the permissions. All required attributes in the
    /// builder must be set before invoking the operation.
    ///
    /// ```ignore
    /// client.overwrite_permissions()
    ///       .values_id(values_id)
    ///       .allow_retrieve(user_id)
    ///       .allow_update(user_id)
    ///       .allow_delete(user_id)
    ///       .allow_compute(user_id, program_id)
    ///       .build()?
    ///       .invoke()
    ///       .await?;
    /// ```
    pub fn overwrite_permissions(&self) -> OverwritePermissionsOperationBuilder<'_> {
        OverwritePermissionsOperationBuilder::new(self)
    }

    /// Update the permissions for a set of values.
    ///
    /// This returns a builder to update the permissions. All required attributes in the
    /// builder must be set before invoking the operation.
    ///
    /// ```ignore
    /// client.update_permissions()
    ///       .values_id(values_id)
    ///       .grant_retrieve(user_id)
    ///       .revoke_update(user_id)
    ///       .grant_compute(user_id, program_id)
    ///       .build()?
    ///       .invoke()
    ///       .await?;
    /// ```
    pub fn update_permissions(&self) -> UpdatePermissionsOperationBuilder<'_> {
        UpdatePermissionsOperationBuilder::new(self)
    }

    /// Store a program in the network.
    ///
    /// This returns a builder to store a program in the network. All required attributes in the
    /// builder must be set before invoking the operation.
    ///
    /// ```ignore
    /// let program_contents = std::fs::read("/tmp/program.nada.bin")?;
    /// client.store_program()
    ///       .name("my-program")
    ///       .program(program_contents)
    ///       .build()?
    ///       .invoke()
    ///       .await?;
    /// ```
    pub fn store_program(&self) -> StoreProgramOperationBuilder<'_> {
        StoreProgramOperationBuilder::new(self)
    }

    /// Store values in the network.
    ///
    /// This returns a builder to store values in the network. All required attributes in the
    /// builder must be set before invoking the operation.
    ///
    /// ```ignore
    /// client.store_values()
    ///       .add_value("foo", NadaValue::new_secret_integer(42))
    ///       .ttl_days(7)
    ///       .build()?
    ///       .invoke()
    ///       .await?;
    /// ```
    pub fn store_values(&self) -> StoreValuesOperationBuilder<'_> {
        StoreValuesOperationBuilder::new(self)
    }

    /// Invoke a computation.
    ///
    /// This returns a builder to invoke a computation in the network. All required attributes in the
    /// builder must be set before invoking the operation.
    ///
    /// ```ignore
    /// client.invoke_compute()
    ///       .add_value("foo", NadaValue::new_secret_integer(42))
    ///       .add_value_id(value_id1)
    ///       .build()?
    ///       .invoke()
    ///       .await?;
    /// ```
    pub fn invoke_compute(&self) -> InvokeComputeOperationBuilder<'_> {
        InvokeComputeOperationBuilder::new(self)
    }

    /// Retrieve a result for a computation.
    ///
    /// This returns a builder to retrieve the results of a computation in the network. All
    /// required attributes in the builder must be set before invoking the operation.
    ///
    /// ```ignore
    /// client.retrieve_compute_results()
    ///       .compute_id(compute_id)
    ///       .build()?
    ///       .invoke()
    ///       .await?;
    /// ```
    pub fn retrieve_compute_results(&self) -> RetrieveComputeResultsOperationBuilder<'_> {
        RetrieveComputeResultsOperationBuilder::new(self)
    }

    /// Get the user's account balance.
    pub async fn account_balance(&self) -> Result<AccountBalanceResponse, InvokeError> {
        Ok(self.payments.account_balance().await?)
    }

    /// Add funds to an account.
    ///
    /// This returns a builder to add funds to an account in the network. All
    /// required attributes in the builder must be set before invoking the operation.
    ///
    /// ```ignore
    /// client.add_funds()
    ///       .amount(TokenAmount::Nil(100))
    ///       .build()?
    ///       .invoke()
    ///       .await?;
    /// ```
    pub fn add_funds(&self) -> AddFundsOperationBuilder<'_> {
        AddFundsOperationBuilder::new(self)
    }

    /// Get the payments configuration.
    pub async fn payments_config(&self) -> Result<PaymentsConfigResponse, InvokeError> {
        Ok(self.payments.payments_config().await?)
    }
}

#[derive(Clone)]
pub(crate) struct GrpcClients {
    pub(crate) compute: ComputeClient,
    pub(crate) membership: MembershipClient,
    pub(crate) permissions: PermissionsClient,
    pub(crate) programs: ProgramsClient,
    pub(crate) values: ValuesClient,
}

impl GrpcClients {
    fn new(channel: AuthenticatedGrpcChannel, max_payload_size: usize) -> Self {
        let compute = ComputeClient::new(channel.clone(), max_payload_size);
        let membership = MembershipClient::new(channel.clone());
        let permissions = PermissionsClient::new(channel.clone(), max_payload_size);
        let programs = ProgramsClient::new(channel.clone(), max_payload_size);
        let values = ValuesClient::new(channel, max_payload_size);
        Self { compute, membership, permissions, programs, values }
    }
}

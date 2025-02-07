//! Overwrite permissions operation.

use super::{BuildError, CollapseResult, InvokeError, PaidOperation, PaidVmOperation};
use crate::{grpc::PermissionsClient, retry::Retrier, vm::VmClient};
use node_api::{
    auth::rust::UserId,
    payments::rust::{OverwritePermissions, PriceQuoteRequest, SignedReceipt},
    permissions::rust::{OverwritePermissionsRequest, Permissions},
};
use tonic::async_trait;
use uuid::Uuid;

/// An overwrite permissions operation.
pub struct OverwritePermissionsOperation {
    values_id: Uuid,
    permissions: Permissions,
}

#[async_trait]
impl PaidVmOperation for OverwritePermissionsOperation {
    type Output = ();

    const NAME: &str = "set-permissions";

    fn price_quote_request(&self) -> PriceQuoteRequest {
        PriceQuoteRequest::OverwritePermissions(OverwritePermissions {
            values_id: self.values_id.into_bytes().to_vec(),
        })
    }

    async fn invoke(self, vm: &VmClient, signed_receipt: SignedReceipt) -> Result<Self::Output, InvokeError> {
        let mut retrier = Retrier::default();
        let request = OverwritePermissionsRequest { signed_receipt, permissions: self.permissions };
        for (party, clients) in vm.clients.iter() {
            retrier.add_request(party.clone(), &clients.permissions, request.clone());
        }

        let results = retrier.invoke(PermissionsClient::overwrite_permissions).await;
        results.collapse_default()?;
        Ok(())
    }
}

/// A builder for an overwrite permissions operation.
///
/// See [PaidOperation] for more information.
#[must_use]
pub struct OverwritePermissionsOperationBuilder<'a> {
    client: &'a VmClient,
    values_id: Option<Uuid>,
    permissions: Permissions,
}

impl<'a> OverwritePermissionsOperationBuilder<'a> {
    pub(crate) fn new(client: &'a VmClient) -> Self {
        let user = client.user_id();
        let permissions = Permissions {
            owner: user,
            retrieve: [user].into_iter().collect(),
            update: [user].into_iter().collect(),
            delete: [user].into_iter().collect(),
            compute: Default::default(),
        };

        Self { client, values_id: None, permissions }
    }

    /// The values identifier to look up.
    pub fn values_id(mut self, id: Uuid) -> Self {
        self.values_id = Some(id);
        self
    }

    /// Allow a user to retrieve these values
    pub fn allow_retrieve(mut self, user: UserId) -> Self {
        self.permissions.retrieve.insert(user);
        self
    }

    /// Allow a user to update these values
    pub fn allow_update(mut self, user: UserId) -> Self {
        self.permissions.update.insert(user);
        self
    }

    /// Allow a user to delete these values
    pub fn allow_delete(mut self, user: UserId) -> Self {
        self.permissions.delete.insert(user);
        self
    }

    /// Allow a user to use these values in a computation
    pub fn allow_compute(mut self, user: UserId, program_id: String) -> Self {
        self.permissions.compute.entry(user).or_default().program_ids.insert(program_id);
        self
    }

    /// Build the overwrite permissions operation.
    pub fn build(self) -> Result<PaidOperation<'a, OverwritePermissionsOperation>, BuildError> {
        let values_id = self.values_id.ok_or_else(|| BuildError("'values_id' not set".into()))?;
        let permissions = self.permissions;
        Ok(PaidOperation::new(OverwritePermissionsOperation { values_id, permissions }, self.client))
    }
}

//! Update permissions operation.

use super::{BuildError, CollapseResult, InvokeError, PaidOperation, PaidVmOperation};
use crate::{grpc::PermissionsClient, retry::Retrier, vm::VmClient, UserId};
use node_api::{
    payments::rust::{PriceQuoteRequest, SignedReceipt, UpdatePermissions},
    permissions::rust::{PermissionsDelta, UpdatePermissionsRequest},
};
use tonic::async_trait;
use uuid::Uuid;

/// An update permissions operation.
pub struct UpdatePermissionsOperation {
    values_id: Uuid,
    delta: PermissionsDelta,
}

#[async_trait]
impl PaidVmOperation for UpdatePermissionsOperation {
    type Output = ();

    const NAME: &str = "update-permissions";

    fn price_quote_request(&self) -> PriceQuoteRequest {
        PriceQuoteRequest::UpdatePermissions(UpdatePermissions { values_id: self.values_id.into_bytes().to_vec() })
    }

    async fn invoke(self, vm: &VmClient, signed_receipt: SignedReceipt) -> Result<Self::Output, InvokeError> {
        let mut retrier = Retrier::default();
        let request = UpdatePermissionsRequest { signed_receipt, delta: self.delta };
        for (party, clients) in vm.clients.iter() {
            retrier.add_request(party.clone(), &clients.permissions, request.clone());
        }

        let results = retrier.invoke(PermissionsClient::update_permissions).await;
        results.collapse_default()?;
        Ok(())
    }
}

/// A builder for an update permissions operation.
///
/// See [PaidOperation] for more information.
#[must_use]
pub struct UpdatePermissionsOperationBuilder<'a> {
    client: &'a VmClient,
    values_id: Option<Uuid>,
    delta: PermissionsDelta,
}

impl<'a> UpdatePermissionsOperationBuilder<'a> {
    pub(crate) fn new(client: &'a VmClient) -> Self {
        Self { client, values_id: None, delta: Default::default() }
    }

    /// The values identifier to look up.
    pub fn values_id(mut self, id: Uuid) -> Self {
        self.values_id = Some(id);
        self
    }

    /// Allow a user to retrieve these values
    pub fn grant_retrieve(mut self, user: UserId) -> Self {
        self.delta.retrieve.grant.insert(user);
        self
    }

    /// Revoke the permissions for a user to retrieve these values
    pub fn revoke_retrieve(mut self, user: UserId) -> Self {
        self.delta.retrieve.revoke.insert(user);
        self
    }

    /// Allow a user to update these values
    pub fn grant_update(mut self, user: UserId) -> Self {
        self.delta.update.grant.insert(user);
        self
    }

    /// Revoke the permissions for a user to update these values
    pub fn revoke_update(mut self, user: UserId) -> Self {
        self.delta.update.revoke.insert(user);
        self
    }

    /// Allow a user to delete these values
    pub fn grant_delete(mut self, user: UserId) -> Self {
        self.delta.delete.grant.insert(user);
        self
    }

    /// Revoke the permissions for a user to delete these values
    pub fn revoke_delete(mut self, user: UserId) -> Self {
        self.delta.delete.revoke.insert(user);
        self
    }

    /// Allow a user to use these values in a computation
    pub fn grant_compute(mut self, user: UserId, program_id: String) -> Self {
        self.delta.compute.grant.entry(user).or_default().program_ids.insert(program_id);
        self
    }

    /// Revoke the permissions for a user to use these values in a computation
    pub fn revoke_compute(mut self, user: UserId, program_id: String) -> Self {
        self.delta.compute.revoke.entry(user).or_default().program_ids.insert(program_id);
        self
    }

    /// Build the update permissions operation.
    pub fn build(self) -> Result<PaidOperation<'a, UpdatePermissionsOperation>, BuildError> {
        let values_id = self.values_id.ok_or_else(|| BuildError("'values_id' not set".into()))?;
        Ok(PaidOperation::new(UpdatePermissionsOperation { values_id, delta: self.delta }, self.client))
    }
}

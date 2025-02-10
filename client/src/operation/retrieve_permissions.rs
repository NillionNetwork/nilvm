//! Retrieve permissions operation.

use super::{BuildError, CollapseResult, InvokeError, PaidOperation, PaidVmOperation};
use crate::{grpc::PermissionsClient, retry::Retrier, vm::VmClient};
use node_api::{
    payments::rust::{PriceQuoteRequest, RetrievePermissions, SignedReceipt},
    permissions::rust::{Permissions, RetrievePermissionsRequest},
};
use tonic::async_trait;
use uuid::Uuid;

/// A retrieve permissions operation.
pub struct RetrievePermissionsOperation {
    values_id: Uuid,
}

#[async_trait]
impl PaidVmOperation for RetrievePermissionsOperation {
    type Output = Permissions;

    const NAME: &str = "retrieve-permissions";

    fn price_quote_request(&self) -> PriceQuoteRequest {
        PriceQuoteRequest::RetrievePermissions(RetrievePermissions { values_id: self.values_id.into_bytes().to_vec() })
    }

    async fn invoke(self, vm: &VmClient, signed_receipt: SignedReceipt) -> Result<Self::Output, InvokeError> {
        let mut retrier = Retrier::default();
        let request = RetrievePermissionsRequest { signed_receipt };
        for (party, clients) in &vm.clients {
            retrier.add_request(party.clone(), &clients.permissions, request.clone());
        }
        let results = retrier.invoke(PermissionsClient::retrieve_permissions).await;
        results.collapse_default()
    }
}

/// A builder for a retrieve permissions operation.
///
/// See [PaidOperation] for more information.
#[must_use]
pub struct RetrievePermissionsOperationBuilder<'a> {
    client: &'a VmClient,
    values_id: Option<Uuid>,
}

impl<'a> RetrievePermissionsOperationBuilder<'a> {
    pub(crate) fn new(client: &'a VmClient) -> Self {
        Self { client, values_id: None }
    }

    /// The values identifier to look up.
    pub fn values_id(mut self, id: Uuid) -> Self {
        self.values_id = Some(id);
        self
    }

    /// Build the retrieve permissions operation.
    pub fn build(mut self) -> Result<PaidOperation<'a, RetrievePermissionsOperation>, BuildError> {
        let values_id = self.values_id.take().ok_or_else(|| BuildError("'values_id' not set".into()))?;
        Ok(PaidOperation::new(RetrievePermissionsOperation { values_id }, self.client))
    }
}

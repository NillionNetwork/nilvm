//! Delete values operation.

use super::{BuildError, CollapseResult, FreeOperation, FreeVmOperation, InvokeError};
use crate::{grpc::ValuesClient, retry::Retrier, vm::VmClient};
use node_api::values::rust::DeleteValuesRequest;
use tonic::async_trait;
use uuid::Uuid;

/// A delete values operation.
pub struct DeleteValuesOperation {
    values_id: Uuid,
}

#[async_trait]
impl FreeVmOperation for DeleteValuesOperation {
    type Output = ();

    const NAME: &str = "delete-values";

    async fn invoke(self, vm: &VmClient) -> Result<Self::Output, InvokeError> {
        let mut retrier = Retrier::default();
        let request = DeleteValuesRequest { values_id: self.values_id.into_bytes().to_vec() };
        for (party, clients) in &vm.clients {
            retrier.add_request(party.clone(), &clients.values, request.clone());
        }
        let results = retrier.invoke(ValuesClient::delete_values).await;
        results.collapse_default()?;
        Ok(())
    }
}

/// A builder for a delete values operation.
///
/// See [FreeOperation] for more information.
#[must_use]
pub struct DeleteValuesOperationBuilder<'a> {
    client: &'a VmClient,
    values_id: Option<Uuid>,
}

impl<'a> DeleteValuesOperationBuilder<'a> {
    pub(crate) fn new(client: &'a VmClient) -> Self {
        Self { client, values_id: None }
    }

    /// The values identifier to look up.
    pub fn values_id(mut self, id: Uuid) -> Self {
        self.values_id = Some(id);
        self
    }

    /// Build the delete values operation.
    pub fn build(mut self) -> Result<FreeOperation<'a, DeleteValuesOperation>, BuildError> {
        let values_id = self.values_id.take().ok_or_else(|| BuildError("'values_id' not set".into()))?;
        Ok(FreeOperation::new(DeleteValuesOperation { values_id }, self.client))
    }
}

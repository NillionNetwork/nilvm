//! Retrieve values operation.

use super::{BuildError, InvokeError, PaidOperation, PaidVmOperation};
use crate::{grpc::ValuesClient, retry::Retrier, vm::VmClient};
use nada_value::protobuf::nada_values_from_protobuf;
use nillion_client_core::values::{CleartextValues, PartyJar};
use node_api::{
    payments::rust::{PriceQuoteRequest, RetrieveValues, SignedReceipt},
    values::rust::RetrieveValuesRequest,
};
use tonic::async_trait;
use uuid::Uuid;

/// A retrieve values operation.
pub struct RetrieveValuesOperation {
    values_id: Uuid,
}

#[async_trait]
impl PaidVmOperation for RetrieveValuesOperation {
    type Output = CleartextValues;

    const NAME: &str = "retrieve-values";

    fn price_quote_request(&self) -> PriceQuoteRequest {
        PriceQuoteRequest::RetrieveValues(RetrieveValues { values_id: self.values_id.into_bytes().to_vec() })
    }

    async fn invoke(self, vm: &VmClient, signed_receipt: SignedReceipt) -> Result<Self::Output, InvokeError> {
        let mut retrier = Retrier::default();
        let request = RetrieveValuesRequest { signed_receipt };
        for (party, clients) in &vm.clients {
            retrier.add_request(party.clone(), &clients.values, request.clone());
        }
        let results = retrier.invoke_mapped(ValuesClient::retrieve_values).await;
        let mut node_values = PartyJar::new(vm.cluster.members.len());
        for (node, result) in results {
            let result = result?;
            let values = nada_values_from_protobuf(result.values, &vm.modulo)
                .map_err(|e| InvokeError(format!("invalid values: {e}")))?;
            node_values.add_element(node, values).map_err(|e| InvokeError(format!("failed to unmask values: {e}")))?;
        }
        let values = vm.masker.unmask(node_values).map_err(|e| InvokeError(format!("failed to unmask values: {e}")))?;
        Ok(values)
    }
}

/// A builder for a retrieve values operation.
///
/// See [PaidOperation] for more information.
#[must_use]
pub struct RetrieveValuesOperationBuilder<'a> {
    client: &'a VmClient,
    values_id: Option<Uuid>,
}

impl<'a> RetrieveValuesOperationBuilder<'a> {
    pub(crate) fn new(client: &'a VmClient) -> Self {
        Self { client, values_id: None }
    }

    /// The values identifier to look up.
    pub fn values_id(mut self, id: Uuid) -> Self {
        self.values_id = Some(id);
        self
    }

    /// Build the retrieve values operation.
    pub fn build(mut self) -> Result<PaidOperation<'a, RetrieveValuesOperation>, BuildError> {
        let values_id = self.values_id.take().ok_or_else(|| BuildError("'values_id' not set".into()))?;
        Ok(PaidOperation::new(RetrieveValuesOperation { values_id }, self.client))
    }
}

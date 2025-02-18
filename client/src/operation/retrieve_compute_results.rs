//! Retrieve computation results.

use super::{BuildError, FreeOperation, FreeVmOperation, InvokeError};
use crate::{
    grpc::ComputeClient,
    retry::{Retrier, TokioSleeper, RETRY_CODES},
    vm::VmClient,
};
use futures::{future, StreamExt};
use nada_value::protobuf::nada_values_from_protobuf;
use nillion_client_core::values::{CleartextValues, PartyId, PartyJar};
use node_api::{compute::rust::RetrieveResultsResponse, TryIntoRust};
use tokio::time::sleep;
use tonic::{async_trait, Status};
use tracing::{info, warn};
use uuid::Uuid;

const RETRIES: usize = 10;

/// An operation that retrieves the result of a computation
pub struct RetrieveComputeResultsOperation {
    compute_id: Uuid,
}

impl RetrieveComputeResultsOperation {
    async fn wait_result(client: ComputeClient, compute_id: Uuid) -> Result<RetrieveResultsResponse, InvokeError> {
        let mut delays = Retrier::<(), (), PartyId, TokioSleeper>::retry_delays().take(RETRIES);

        loop {
            match Self::do_wait_result(&client, compute_id).await {
                Ok(result) => return Ok(result),
                Err(e) if RETRY_CODES.contains(&e.code()) => {
                    warn!("Request failed: {e}");
                    match delays.next() {
                        Some(delay) => {
                            info!("Sleeping for {delay:?}");
                            sleep(*delay).await;
                        }
                        None => return Err(InvokeError(e.to_string())),
                    };
                }
                Err(e) => return Err(InvokeError(e.to_string())),
            };
        }
    }

    async fn do_wait_result(client: &ComputeClient, compute_id: Uuid) -> tonic::Result<RetrieveResultsResponse> {
        let mut stream = client.retrieve_result(compute_id).await?;
        while let Some(event) = stream.next().await {
            let event: RetrieveResultsResponse =
                event?.try_into_rust().map_err(|e| Status::invalid_argument(e.to_string()))?;
            if matches!(event, RetrieveResultsResponse::WaitingComputation) {
                continue;
            }
            return Ok(event);
        }
        Err(Status::internal("server closed results channel"))
    }
}

#[async_trait]
impl FreeVmOperation for RetrieveComputeResultsOperation {
    // The computation itself can return an error, which is different from an error fetching the
    // computation result.
    type Output = Result<CleartextValues, ComputeError>;

    const NAME: &str = "retrieve-compute-results";

    async fn invoke(self, vm: &VmClient) -> Result<Self::Output, InvokeError> {
        let futs = vm.clients.values().map(|c| Self::wait_result(c.compute.clone(), self.compute_id));
        let results = future::join_all(futs).await;
        let mut node_values = PartyJar::new(vm.cluster.members.len());
        for (node, result) in vm.clients.keys().zip(results) {
            let result = result?;
            match result {
                RetrieveResultsResponse::WaitingComputation => {
                    // this is handled while waiting for results so it shouldn't happen
                    return Err(InvokeError("still waiting for computation".into()));
                }
                RetrieveResultsResponse::Success { values, .. } => {
                    let values = nada_values_from_protobuf(values, &vm.modulo)
                        .map_err(|e| InvokeError(format!("invalid values: {e}")))?;
                    node_values
                        .add_element(node.clone(), values)
                        .map_err(|e| InvokeError(format!("failed to unmask values: {e}")))?;
                }
                RetrieveResultsResponse::Error { error } => return Ok(Err(ComputeError(error))),
            };
        }
        let values = vm.masker.unmask(node_values).map_err(|e| InvokeError(format!("failed to unmask values: {e}")))?;
        Ok(Ok(values))
    }
}

/// An error returned during a compute operation.
#[derive(Debug, thiserror::Error)]
#[error("compute operation failed: {0}")]
pub struct ComputeError(String);

/// A builder for a retrieve results operation.
///
/// See [PaidOperation] for more information.
#[must_use]
pub struct RetrieveComputeResultsOperationBuilder<'a> {
    vm: &'a VmClient,
    compute_id: Option<Uuid>,
}

impl<'a> RetrieveComputeResultsOperationBuilder<'a> {
    pub(crate) fn new(vm: &'a VmClient) -> Self {
        Self { vm, compute_id: None }
    }

    /// The identifier of the computation to look up.
    pub fn compute_id(mut self, id: Uuid) -> Self {
        self.compute_id = Some(id);
        self
    }

    /// Build the retrieve retrieve results operation.
    pub fn build(mut self) -> Result<FreeOperation<'a, RetrieveComputeResultsOperation>, BuildError> {
        let compute_id = self.compute_id.take().ok_or_else(|| BuildError("'compute_id' not set".into()))?;
        Ok(FreeOperation::new(RetrieveComputeResultsOperation { compute_id }, self.vm))
    }
}

//! Pool status operation.

use super::{CollapseResult, InvokeError, PaidVmOperation};
use crate::{grpc::LeaderQueriesClient, retry::Retrier, vm::VmClient};
use nillion_client_core::values::PartyId;
use node_api::{
    leader_queries::rust::{PoolStatusRequest, PoolStatusResponse},
    payments::rust::{PriceQuoteRequest, SignedReceipt},
};
use tonic::async_trait;

/// A preprocessing pool status operation.
pub struct PoolStatusOperation;

#[async_trait]
impl PaidVmOperation for PoolStatusOperation {
    type Output = PoolStatusResponse;

    const NAME: &str = "pool-status";

    fn price_quote_request(&self) -> PriceQuoteRequest {
        PriceQuoteRequest::PoolStatus
    }

    async fn invoke(self, vm: &VmClient, signed_receipt: SignedReceipt) -> Result<Self::Output, InvokeError> {
        let mut retrier = Retrier::default();
        let party = PartyId::from(Vec::from(vm.cluster.leader.identity.clone()));
        let request = PoolStatusRequest { signed_receipt };
        retrier.add_request(party, &vm.leader_queries, request);

        let result = retrier.invoke(LeaderQueriesClient::pool_status).await;
        result.collapse_default()
    }
}

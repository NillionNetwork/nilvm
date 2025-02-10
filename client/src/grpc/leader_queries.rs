//! Leader queries gRPC client.

use grpc_channel::{TransportChannel, UnauthenticatedGrpcChannel};
use node_api::{leader_queries::proto, ConvertProto, TryIntoRust};
use tonic::Request;

pub use node_api::leader_queries::rust::*;

/// A client that interacts with the leader queries service.
#[derive(Clone)]
pub struct LeaderQueriesClient(
    proto::leader_queries_client::LeaderQueriesClient<<UnauthenticatedGrpcChannel as TransportChannel>::Channel>,
);

impl LeaderQueriesClient {
    /// Create a new client.
    pub fn new<T: TransportChannel>(channel: T) -> Self {
        let client =
            proto::leader_queries_client::LeaderQueriesClient::new(channel.into_unauthenticated().into_channel());
        Self(client)
    }

    /// Get the preprocessing pool status.
    pub async fn pool_status(&self, request: PoolStatusRequest) -> tonic::Result<PoolStatusResponse> {
        let request = Request::new(request.into_proto());
        let response = self.0.clone().pool_status(request).await?;
        let response = response.into_inner().try_into_rust()?;
        Ok(response)
    }
}

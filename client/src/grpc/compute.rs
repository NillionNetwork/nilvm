//! Compute gRPC client.

use grpc_channel::{AuthenticatedGrpcChannel, TransportChannel};
use node_api::{
    compute::{
        proto,
        rust::{InvokeComputeRequest, InvokeComputeResponse, RetrieveResultsRequest},
    },
    ConvertProto, TryIntoRust,
};
use tonic::{Request, Streaming};

pub use node_api::leader_queries::rust::*;
use uuid::Uuid;

/// A client that interacts with the compute service.
#[derive(Clone)]
pub struct ComputeClient(proto::compute_client::ComputeClient<<AuthenticatedGrpcChannel as TransportChannel>::Channel>);

impl ComputeClient {
    /// Create a new client.
    pub fn new(channel: AuthenticatedGrpcChannel, max_payload_size: usize) -> Self {
        let client = proto::compute_client::ComputeClient::new(channel.into_channel())
            .max_decoding_message_size(max_payload_size)
            .max_encoding_message_size(max_payload_size);
        Self(client)
    }

    /// Invoke a compute operation.
    pub async fn invoke_compute(&self, request: InvokeComputeRequest) -> tonic::Result<InvokeComputeResponse> {
        let request = Request::new(request.into_proto());
        let response = self.0.clone().invoke_compute(request).await?;
        Ok(response.into_inner().try_into_rust()?)
    }

    /// Retrieve a result for a computation.
    pub async fn retrieve_result(
        &self,
        compute_id: Uuid,
    ) -> tonic::Result<Streaming<proto::retrieve::RetrieveResultsResponse>> {
        let request = RetrieveResultsRequest { compute_id: compute_id.as_bytes().to_vec() };
        let request = Request::new(request.into_proto());
        let response = self.0.clone().retrieve_results(request).await?;
        Ok(response.into_inner())
    }
}

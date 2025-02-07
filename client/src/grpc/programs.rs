//! Programs gRPC client.

use grpc_channel::{AuthenticatedGrpcChannel, TransportChannel};
use node_api::{programs::proto, ConvertProto};
use tonic::Request;

pub use node_api::programs::rust::*;

/// A client that interacts with the payments service.
#[derive(Clone)]
pub struct ProgramsClient(
    proto::programs_client::ProgramsClient<<AuthenticatedGrpcChannel as TransportChannel>::Channel>,
);

impl ProgramsClient {
    /// Create a new client.
    pub fn new(channel: AuthenticatedGrpcChannel, max_payload_size: usize) -> Self {
        let client = proto::programs_client::ProgramsClient::new(channel.into_channel())
            .max_decoding_message_size(max_payload_size)
            .max_encoding_message_size(max_payload_size);
        Self(client)
    }

    /// Store a program in the network.
    pub async fn store_program(&self, request: StoreProgramRequest) -> tonic::Result<String> {
        let request = Request::new(request.into_proto());
        let response = self.0.clone().store_program(request).await?;
        Ok(response.into_inner().program_id)
    }
}

//! Values gRPC client.

use grpc_channel::{AuthenticatedGrpcChannel, TransportChannel};
use node_api::{values::proto, ConvertProto, TryIntoRust};
use tonic::Request;

pub use node_api::values::rust::*;

/// A client that interacts with the values service.
#[derive(Clone)]
pub struct ValuesClient(proto::values_client::ValuesClient<<AuthenticatedGrpcChannel as TransportChannel>::Channel>);

impl ValuesClient {
    /// Create a new client.
    pub fn new(channel: AuthenticatedGrpcChannel, max_payload_size: usize) -> Self {
        let client = proto::values_client::ValuesClient::new(channel.into_channel())
            .max_decoding_message_size(max_payload_size)
            .max_encoding_message_size(max_payload_size);
        Self(client)
    }

    /// Delete a set of stored values.
    pub async fn delete_values(&self, request: DeleteValuesRequest) -> tonic::Result<()> {
        let request = Request::new(request.into_proto());
        self.0.clone().delete_values(request).await?;
        Ok(())
    }

    /// Store values in the network.
    pub async fn store_values(&self, request: StoreValuesRequest) -> tonic::Result<StoreValuesResponse> {
        let request = Request::new(request.into_proto());
        let response = self.0.clone().store_values(request).await?.into_inner();
        Ok(response.try_into_rust()?)
    }

    /// Retrieve a set of values previously stored in the network.
    pub async fn retrieve_values(&self, request: RetrieveValuesRequest) -> tonic::Result<RetrieveValuesResponse> {
        let request = Request::new(request.into_proto());
        let response = self.0.clone().retrieve_values(request).await?.into_inner();
        Ok(response.try_into_rust()?)
    }
}

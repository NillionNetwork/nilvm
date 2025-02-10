//! Permissions gRPC client.

use grpc_channel::{AuthenticatedGrpcChannel, TransportChannel};
use node_api::{permissions::proto, ConvertProto, TryIntoRust};
use tonic::Request;

pub use node_api::permissions::rust::*;

/// A client that interacts with the permissions service.
#[derive(Clone)]
pub struct PermissionsClient(
    proto::permissions_client::PermissionsClient<<AuthenticatedGrpcChannel as TransportChannel>::Channel>,
);

impl PermissionsClient {
    /// Create a new client.
    pub fn new(channel: AuthenticatedGrpcChannel, max_payload_size: usize) -> Self {
        let client = proto::permissions_client::PermissionsClient::new(channel.into_channel())
            .max_decoding_message_size(max_payload_size)
            .max_encoding_message_size(max_payload_size);
        Self(client)
    }

    /// Retrieve the permissions associated with a set of values stored in the network.
    pub async fn retrieve_permissions(&self, request: RetrievePermissionsRequest) -> tonic::Result<Permissions> {
        let request = Request::new(request.into_proto());
        let response = self.0.clone().retrieve_permissions(request).await?.into_inner();
        Ok(response.try_into_rust()?)
    }

    /// Overwrite the permissions associated with a set of values stored in the network.
    pub async fn overwrite_permissions(&self, request: OverwritePermissionsRequest) -> tonic::Result<()> {
        let request = Request::new(request.into_proto());
        self.0.clone().overwrite_permissions(request).await?.into_inner();
        Ok(())
    }

    /// Update the permissions associated with a set of values stored in the network.
    pub async fn update_permissions(&self, request: UpdatePermissionsRequest) -> tonic::Result<()> {
        let request = Request::new(request.into_proto());
        self.0.clone().update_permissions(request).await?.into_inner();
        Ok(())
    }
}

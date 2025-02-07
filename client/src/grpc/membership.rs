//! A client for the membership API.

use grpc_channel::{TransportChannel, UnauthenticatedGrpcChannel};
use node_api::{membership::proto, TryIntoRust};
use tonic::Request;

pub use node_api::membership::rust::*;

/// A client that interacts with the membership service.
#[derive(Clone)]
pub struct MembershipClient(
    proto::membership_client::MembershipClient<<UnauthenticatedGrpcChannel as TransportChannel>::Channel>,
);

impl MembershipClient {
    /// Create a new client.
    pub fn new<T: TransportChannel>(channel: T) -> Self {
        let client = proto::membership_client::MembershipClient::new(channel.into_unauthenticated().into_channel());
        Self(client)
    }

    /// Get the cluster's information.
    pub async fn cluster(&self) -> tonic::Result<Cluster> {
        let response = self.0.clone().cluster(Request::new(())).await?;
        let cluster = response.into_inner().try_into_rust()?;
        Ok(cluster)
    }

    /// Get the node's version.
    pub async fn node_version(&self) -> tonic::Result<NodeVersion> {
        let response = self.0.clone().node_version(Request::new(())).await?;
        Ok(response.into_inner().try_into_rust()?)
    }
}

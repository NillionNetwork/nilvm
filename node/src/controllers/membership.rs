//! The membershipt gRPC API.

use crate::controllers::TraceRequest;
use async_trait::async_trait;
use build_info::BuildInfo;
use node_api::{
    membership::{proto, rust::Cluster},
    ConvertProto,
};
use tonic::{Request, Response};
use tracing::{error, info, instrument};

/// The membership API.
pub(crate) struct MembershipApi {
    cluster_definition: proto::cluster::Cluster,
    version: proto::version::NodeVersion,
}

impl MembershipApi {
    /// Construct a new service that serves this node's membership information.
    pub(crate) fn new(cluster_definition: Cluster) -> Self {
        let build_info = BuildInfo::default();
        let version = build_info.release_candidate_version.or(build_info.release_version);
        let version = match version {
            Some(version) => match semver::Version::parse(version.trim_start_matches('v')) {
                Ok(version) => Some(proto::version::SemverVersion {
                    major: version.major,
                    minor: version.minor,
                    patch: version.patch,
                    pre_release: version.pre.as_str().to_string(),
                }),
                Err(e) => {
                    error!("Invalid semver version: {e}");
                    None
                }
            },
            None => {
                info!("No version provided, only advertising git hash");
                None
            }
        };
        let node_info = proto::version::NodeVersion { version, git_hash: build_info.git_commit_hash.to_string() };
        Self { cluster_definition: cluster_definition.into_proto(), version: node_info }
    }
}

#[async_trait]
impl proto::membership_server::Membership for MembershipApi {
    #[instrument(name = "api.membership.cluster", skip_all, fields(user_id = _request.trace_user_id()))]
    async fn cluster(&self, _request: Request<()>) -> tonic::Result<Response<proto::cluster::Cluster>> {
        Ok(Response::new(self.cluster_definition.clone()))
    }

    #[instrument(name = "api.membership.node_version", skip_all, fields(user_id = _request.trace_user_id()))]
    async fn node_version(&self, _request: Request<()>) -> tonic::Result<Response<proto::version::NodeVersion>> {
        Ok(Response::new(self.version.clone()))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use node_api::{
        auth::rust::PublicKey,
        membership::{
            proto::membership_server::Membership,
            rust::{ClusterMember, Prime, PublicKeys},
        },
    };

    #[tokio::test]
    async fn lookup_cluster() {
        let member = ClusterMember {
            identity: b"foo".to_vec().into(),
            public_keys: PublicKeys { authentication: PublicKey::Ed25519([0; 32]) },
            grpc_endpoint: "http://host:1337".to_string(),
        };
        let cluster = Cluster {
            members: vec![member.clone()],
            leader: member.clone(),
            prime: Prime::Safe64Bits,
            polynomial_degree: 1,
            kappa: 0,
        };
        let api = MembershipApi::new(cluster.clone());
        let response = api.cluster(Request::new(())).await.expect("request failed").into_inner();
        assert_eq!(cluster.into_proto(), response);
    }
}

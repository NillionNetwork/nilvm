//! The membership API.

/// The protobuf model definitions.
pub mod proto {
    pub use crate::proto::membership::v1::*;
}

/// Rust types that can be converted from/to their protobuf counterparts.
#[cfg(feature = "rust-types")]
pub mod rust {
    use std::{fmt, str::FromStr};

    use super::proto;
    use crate::{auth::rust::PublicKey, errors::InvalidHexId, ConvertProto, ProtoError, TransparentProto, TryIntoRust};

    // A node's information.
    pub type NodeVersion = proto::version::NodeVersion;

    // A node's version
    pub type SemverVersion = proto::version::SemverVersion;

    impl TransparentProto for NodeVersion {}
    impl TransparentProto for SemverVersion {}

    /// A cluster's definition.
    #[derive(Clone, Debug, PartialEq)]
    pub struct Cluster {
        /// The members of this cluster.
        pub members: Vec<ClusterMember>,

        /// The leader of this cluster.
        pub leader: ClusterMember,

        /// The prime number used in this cluster.
        pub prime: Prime,

        /// The polynomial degree used in this cluster.
        pub polynomial_degree: u32,

        /// The security parameter kappa used in this cluster.
        pub kappa: u32,
    }

    impl ConvertProto for Cluster {
        type ProtoType = proto::cluster::Cluster;

        fn into_proto(self) -> Self::ProtoType {
            Self::ProtoType {
                members: self.members.into_proto(),
                leader: Some(self.leader.into_proto()),
                prime: self.prime.into_proto(),
                polynomial_degree: self.polynomial_degree,
                kappa: self.kappa,
            }
        }

        fn try_from_proto(model: Self::ProtoType) -> Result<Self, ProtoError> {
            let leader = model.leader.ok_or(ProtoError("'leader' not set"))?;
            Ok(Self {
                members: model.members.try_into_rust()?,
                leader: leader.try_into_rust()?,
                prime: model.prime.try_into_rust()?,
                polynomial_degree: model.polynomial_degree,
                kappa: model.kappa,
            })
        }
    }

    /// A cluster member.
    #[derive(Clone, Debug, PartialEq)]
    pub struct ClusterMember {
        /// The identity for this member.
        pub identity: NodeId,

        /// The gRPC endpoint this member can be reached at.
        pub grpc_endpoint: String,

        /// The public keys for this member.
        pub public_keys: PublicKeys,
    }

    impl ConvertProto for ClusterMember {
        type ProtoType = proto::cluster::ClusterMember;

        fn into_proto(self) -> Self::ProtoType {
            let authentication_public_key = self.public_keys.authentication.clone().into_proto();
            Self::ProtoType {
                identity: Some(self.identity.into_proto()),
                public_keys: Some(self.public_keys.into_proto()),
                // This field is deprecated
                public_key: Some(authentication_public_key),
                grpc_endpoint: self.grpc_endpoint,
            }
        }

        fn try_from_proto(model: Self::ProtoType) -> Result<Self, ProtoError> {
            let public_keys = model.public_keys.ok_or(ProtoError("'public_keys' not set"))?.try_into_rust()?;
            let identity = model.identity.ok_or(ProtoError("'identity' not set"))?.try_into_rust()?;
            Ok(Self { identity, public_keys, grpc_endpoint: model.grpc_endpoint })
        }
    }

    /// A node identifier.
    #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct NodeId(Vec<u8>);

    impl From<Vec<u8>> for NodeId {
        fn from(id: Vec<u8>) -> Self {
            Self(id)
        }
    }

    impl From<NodeId> for Vec<u8> {
        fn from(id: NodeId) -> Self {
            id.0
        }
    }

    impl ConvertProto for NodeId {
        type ProtoType = proto::cluster::NodeId;

        fn into_proto(self) -> Self::ProtoType {
            Self::ProtoType { contents: self.0 }
        }

        fn try_from_proto(model: Self::ProtoType) -> Result<Self, ProtoError> {
            Ok(Self(model.contents))
        }
    }

    impl fmt::Display for NodeId {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}", hex::encode(&self.0))
        }
    }

    impl FromStr for NodeId {
        type Err = InvalidHexId;

        fn from_str(id: &str) -> Result<Self, Self::Err> {
            let id = hex::decode(id).map_err(|_| InvalidHexId::HexEncoding)?;
            Ok(Self(id))
        }
    }

    /// The public keys for a cluster member.
    #[derive(Clone, Debug, PartialEq)]
    pub struct PublicKeys {
        /// The public key used for authentication.
        pub authentication: PublicKey,
    }

    impl ConvertProto for PublicKeys {
        type ProtoType = proto::cluster::PublicKeys;

        fn into_proto(self) -> Self::ProtoType {
            Self::ProtoType { authentication: Some(self.authentication.into_proto()) }
        }

        fn try_from_proto(model: Self::ProtoType) -> Result<Self, ProtoError> {
            let authentication = model.authentication.ok_or(ProtoError("'authentication' not set"))?.try_into_rust()?;
            Ok(Self { authentication })
        }
    }

    /// A pre-defined prime number.
    #[derive(Clone, Debug, PartialEq)]
    pub enum Prime {
        // A safe 64 bit prime number.
        Safe64Bits,

        // A safe 128 bit prime number.
        Safe128Bits,

        // A safe 256 bit prime number.
        Safe256Bits,
    }

    impl ConvertProto for Prime {
        type ProtoType = i32;

        fn into_proto(self) -> Self::ProtoType {
            type Proto = proto::cluster::Prime;
            match self {
                Prime::Safe64Bits => Proto::Safe64Bits,
                Prime::Safe128Bits => Proto::Safe128Bits,
                Prime::Safe256Bits => Proto::Safe256Bits,
            }
            .into()
        }

        fn try_from_proto(model: Self::ProtoType) -> Result<Self, ProtoError> {
            type Proto = proto::cluster::Prime;
            let model = proto::cluster::Prime::try_from(model).map_err(|_| ProtoError("invalid prime"))?;
            match model {
                Proto::Safe64Bits => Ok(Self::Safe64Bits),
                Proto::Safe128Bits => Ok(Self::Safe128Bits),
                Proto::Safe256Bits => Ok(Self::Safe256Bits),
            }
        }
    }
}

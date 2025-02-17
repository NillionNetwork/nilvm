use crate::args::PermissionActionArgs;
use nada_value::NadaType;
use nillion_client::{
    grpc::{
        membership::{Cluster, ClusterMember, Prime, PublicKeys},
        permissions::Permissions,
        PublicKey,
    },
    Clear, NadaValue, UserId,
};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr, SerializeDisplay};
use std::{
    collections::{BTreeMap, HashMap},
    fmt,
    ops::{Deref, Range},
};
use uuid::Uuid;

/// Represents a set of permissions to be associated to a secret
#[serde_as]
#[derive(Serialize, Deserialize)]
pub(crate) struct UserFriendlyPermissions {
    /// Set of retrieve permissions
    #[serde_as(as = "Vec<DisplayFromStr>")]
    pub(crate) retrieve: Vec<UserId>,

    /// Set of update permissions
    #[serde_as(as = "Vec<DisplayFromStr>")]
    pub(crate) update: Vec<UserId>,

    /// Set of delete permissions
    #[serde_as(as = "Vec<DisplayFromStr>")]
    pub(crate) delete: Vec<UserId>,

    /// Set of compute permissions
    #[serde_as(as = "HashMap<DisplayFromStr, Vec<DisplayFromStr>>")]
    pub(crate) compute: HashMap<UserId, Vec<String>>,
}

impl From<&Permissions> for UserFriendlyPermissions {
    fn from(permissions: &Permissions) -> Self {
        UserFriendlyPermissions {
            retrieve: permissions.retrieve.iter().cloned().collect(),
            update: permissions.update.iter().cloned().collect(),
            delete: permissions.delete.iter().cloned().collect(),
            compute: permissions
                .compute
                .iter()
                .map(|(key, users)| (*key, users.program_ids.iter().cloned().collect()))
                .collect(),
        }
    }
}

/// A set of permissions delta to be applied to a set of values.
#[derive(Default, Deserialize)]
pub(crate) struct PermissionsDelta {
    /// The retrieve values permissions to be granted/revoked.
    #[serde(default)]
    pub(crate) retrieve: PermissionCommand,

    /// The update values permissions to be granted/revoked.
    #[serde(default)]
    pub(crate) update: PermissionCommand,

    /// The delete values permissions to be granted/revoked.
    #[serde(default)]
    pub(crate) delete: PermissionCommand,

    /// The compute permissions to be granted.
    #[serde(default)]
    pub(crate) compute: ComputePermissionCommand,
}

impl PermissionsDelta {
    pub(crate) fn merge(&mut self, other: PermissionsDelta) {
        self.retrieve.merge(other.retrieve);
        self.update.merge(other.update);
        self.delete.merge(other.delete);
        self.compute.merge(other.compute);
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.retrieve.is_empty() && self.update.is_empty() && self.delete.is_empty() && self.compute.is_empty()
    }
}

/// A set of permissions delta to be applied to a set of values.
#[serde_as]
#[derive(Clone, Default, Deserialize)]
pub(crate) struct PermissionCommand {
    /// The list of users that we're granting permissions to.
    #[serde(default)]
    #[serde_as(as = "Vec<DisplayFromStr>")]
    pub(crate) grant: Vec<UserId>,

    /// The list of users that we're revoking permissions from.
    #[serde(default)]
    #[serde_as(as = "Vec<DisplayFromStr>")]
    pub(crate) revoke: Vec<UserId>,
}

impl PermissionCommand {
    fn merge(&mut self, other: PermissionCommand) {
        self.grant.extend(other.grant);
        self.revoke.extend(other.revoke);
    }

    fn is_empty(&self) -> bool {
        self.grant.is_empty() && self.revoke.is_empty()
    }
}

/// A set of permissions delta to be applied to a set of values.
#[serde_as]
#[derive(Clone, Default, Deserialize)]
pub(crate) struct ComputePermissionCommand {
    /// The list of users with program IDs that we're granting permissions to.
    #[serde(default)]
    #[serde_as(as = "HashMap<DisplayFromStr, Vec<DisplayFromStr>>")]
    pub(crate) grant: HashMap<UserId, Vec<String>>,

    /// The list of users with program IDs that we're revoking permissions from.
    #[serde(default)]
    #[serde_as(as = "HashMap<DisplayFromStr, Vec<DisplayFromStr>>")]
    pub(crate) revoke: HashMap<UserId, Vec<String>>,
}

impl ComputePermissionCommand {
    fn merge(&mut self, other: ComputePermissionCommand) {
        for (user_id, programs) in other.grant {
            self.grant.entry(user_id).or_default().extend(programs);
        }
        for (user_id, programs) in other.revoke {
            self.revoke.entry(user_id).or_default().extend(programs);
        }
    }

    fn is_empty(&self) -> bool {
        self.grant.is_empty() && self.revoke.is_empty()
    }
}

/// Get permissions delta from a CLI subcommand
impl From<PermissionActionArgs> for PermissionsDelta {
    fn from(action: PermissionActionArgs) -> Self {
        PermissionsDelta {
            retrieve: PermissionCommand { grant: action.grant_retrieve, revoke: action.revoke_retrieve },
            update: PermissionCommand { grant: action.grant_update, revoke: action.revoke_update },
            delete: PermissionCommand { grant: action.grant_delete, revoke: action.revoke_delete },
            compute: ComputePermissionCommand {
                grant: action.grant_compute.into_iter().collect(),
                revoke: action.revoke_compute.into_iter().collect(),
            },
        }
    }
}

/// The preprocessing pool status.
#[derive(Serialize)]
pub(crate) struct PreprocessingPoolStatus {
    pub(crate) offsets: BTreeMap<String, Range<u64>>,
    pub(crate) auxiliary_material_available: bool,
    pub(crate) preprocessing_active: bool,
}

#[allow(clippy::enum_variant_names)]
#[derive(Serialize)]
pub(crate) enum PrimeInfo {
    Safe64Bits,
    Safe128Bits,
    Safe256Bits,
}

#[derive(Serialize)]
pub(crate) struct PublicKeysInfo {
    authentication: PublicKeyInfo,
}

#[derive(Serialize)]
#[serde(untagged)]
pub(crate) enum PublicKeyInfo {
    Ed25519 {
        #[serde(rename = "Ed25519")]
        hex: String,
    },
    Secp256k1 {
        #[serde(rename = "Secp256k1")]
        hex: String,
    },
}

#[derive(Serialize)]
pub(crate) struct ClusterMemberInfo {
    pub(crate) identity: String,
    pub(crate) grpc_endpoint: String,
    pub(crate) public_keys: PublicKeysInfo,
}

#[derive(Serialize)]
pub(crate) struct ClusterInfo {
    pub(crate) members: Vec<ClusterMemberInfo>,
    pub(crate) leader: ClusterMemberInfo,
    pub(crate) prime: PrimeInfo,
    pub(crate) polynomial_degree: u32,
}

impl From<Prime> for PrimeInfo {
    fn from(prime: Prime) -> Self {
        match prime {
            Prime::Safe64Bits => PrimeInfo::Safe64Bits,
            Prime::Safe128Bits => PrimeInfo::Safe128Bits,
            Prime::Safe256Bits => PrimeInfo::Safe256Bits,
        }
    }
}

impl From<&PublicKeys> for PublicKeysInfo {
    fn from(public_keys: &PublicKeys) -> Self {
        PublicKeysInfo {
            authentication: match &public_keys.authentication {
                PublicKey::Ed25519(bytes) => PublicKeyInfo::Ed25519 { hex: hex::encode(bytes) },
                PublicKey::Secp256k1(bytes) => PublicKeyInfo::Secp256k1 { hex: hex::encode(bytes) },
            },
        }
    }
}

impl From<&ClusterMember> for ClusterMemberInfo {
    fn from(member: &ClusterMember) -> Self {
        ClusterMemberInfo {
            identity: hex::encode(Vec::<u8>::from(member.identity.clone())),
            grpc_endpoint: member.grpc_endpoint.clone(),
            public_keys: PublicKeysInfo::from(&member.public_keys),
        }
    }
}

impl From<&Cluster> for ClusterInfo {
    fn from(cluster: &Cluster) -> Self {
        ClusterInfo {
            members: cluster.members.iter().map(ClusterMemberInfo::from).collect(),
            leader: ClusterMemberInfo::from(&cluster.leader),
            prime: PrimeInfo::from(cluster.prime.clone()),
            polynomial_degree: cluster.polynomial_degree,
        }
    }
}

/// A wrapper over a `Secret` that contains a human friendly implementation of `std::fmt::Display`.
#[derive(Serialize)]
pub(crate) struct PrettyValue {
    #[serde(rename = "type")]
    ty: NadaType,

    value: DisplayFriendlyValue,
}

impl From<NadaValue<Clear>> for PrettyValue {
    fn from(value: NadaValue<Clear>) -> Self {
        let ty = value.to_type();
        let value = DisplayFriendlyValue(value);
        Self { ty, value }
    }
}

#[derive(SerializeDisplay)]
struct DisplayFriendlyValue(NadaValue<Clear>);

impl fmt::Display for DisplayFriendlyValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            NadaValue::Boolean(value) => write!(f, "{value}"),
            NadaValue::Integer(value) => write!(f, "{value}"),
            NadaValue::UnsignedInteger(value) => write!(f, "{value}"),
            NadaValue::SecretBlob(value) => write!(f, "{value:?}"),
            NadaValue::EcdsaDigestMessage(value) => write!(f, "{value:?}"),
            NadaValue::EcdsaPublicKey(value) => write!(f, "{}", hex::encode(value.0)),
            NadaValue::StoreId(value) => write!(f, "{}", Uuid::from_bytes(*value)),
            NadaValue::EddsaMessage(value) => write!(f, "{value:?}"),
            NadaValue::EddsaPublicKey(value) => write!(f, "{value:?}"),
            NadaValue::EddsaSignature(value) => {
                let r = hex::encode(value.signature.r.to_bytes());
                let z = hex::encode(value.signature.z.to_be_bytes());
                write!(f, "(r={r}, z={z})")
            }
            NadaValue::Array { values, .. } => {
                write!(f, "[")?;
                for value in values {
                    write!(f, "{}", DisplayFriendlyValue(value.clone()))?;
                }
                write!(f, "]")
            }
            NadaValue::Tuple { left, right } => {
                write!(
                    f,
                    "({},{})",
                    DisplayFriendlyValue(left.deref().clone()),
                    DisplayFriendlyValue(right.deref().clone())
                )
            }
            NadaValue::NTuple { values } => {
                write!(f, "[")?;
                for value in values {
                    write!(f, "{}", DisplayFriendlyValue(value.clone()))?;
                }
                write!(f, "]")
            }
            NadaValue::Object { values } => {
                write!(f, "[")?;
                for (key, value) in values {
                    write!(f, "{key}:{}", DisplayFriendlyValue(value.clone()))?;
                }
                write!(f, "]")
            }
            NadaValue::SecretInteger(value) => write!(f, "{value}"),
            NadaValue::SecretUnsignedInteger(value) => write!(f, "{value}"),
            NadaValue::EcdsaSignature(s) => {
                let r = hex::encode(s.r.into_inner().to_be_bytes());
                let s = hex::encode(s.s.into_inner().to_be_bytes());
                write!(f, "(r={r}, s={s})")
            }
            // Unimplemented
            NadaValue::SecretBoolean(_)
            | NadaValue::ShamirShareInteger(_)
            | NadaValue::ShamirShareUnsignedInteger(_)
            | NadaValue::ShamirShareBoolean(_)
            | NadaValue::EcdsaPrivateKey(_)
            | NadaValue::EddsaPrivateKey(_) => Err(fmt::Error),
        }
    }
}

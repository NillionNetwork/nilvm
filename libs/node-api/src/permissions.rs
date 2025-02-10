//! The permissions API.

/// The protobuf model definitions.
pub mod proto {
    pub use crate::proto::permissions::v1::*;
}

/// Rust types that can be converted from/to their protobuf counterparts.
#[cfg(feature = "rust-types")]
pub mod rust {
    use crate::{auth::rust::UserId, payments::rust::SignedReceipt, ConvertProto, ProtoError, TryIntoRust};
    use std::collections::{HashMap, HashSet};

    /// A request to retrieve the permissions for a set of values from the network.
    #[derive(Clone, Debug, PartialEq)]
    pub struct RetrievePermissionsRequest {
        /// The receipt that proves this operation was paid for.
        pub signed_receipt: SignedReceipt,
    }

    impl ConvertProto for RetrievePermissionsRequest {
        type ProtoType = super::proto::retrieve::RetrievePermissionsRequest;

        fn into_proto(self) -> Self::ProtoType {
            Self::ProtoType { signed_receipt: Some(self.signed_receipt.into_proto()) }
        }

        fn try_from_proto(model: Self::ProtoType) -> Result<Self, crate::ProtoError> {
            let signed_receipt = model.signed_receipt.ok_or(ProtoError("'signed_receipt' not set"))?.try_into_rust()?;
            Ok(Self { signed_receipt })
        }
    }

    /// A request to overwrite the permissions for a set of values from the network.
    #[derive(Clone, Debug, PartialEq)]
    pub struct OverwritePermissionsRequest {
        /// The receipt that proves this operation was paid for.
        pub signed_receipt: SignedReceipt,

        /// The permissions to update
        pub permissions: Permissions,
    }

    impl ConvertProto for OverwritePermissionsRequest {
        type ProtoType = super::proto::overwrite::OverwritePermissionsRequest;

        fn into_proto(self) -> Self::ProtoType {
            Self::ProtoType {
                signed_receipt: Some(self.signed_receipt.into_proto()),
                permissions: Some(self.permissions.into_proto()),
            }
        }

        fn try_from_proto(model: Self::ProtoType) -> Result<Self, crate::ProtoError> {
            let signed_receipt = model.signed_receipt.ok_or(ProtoError("'signed_receipt' not set"))?.try_into_rust()?;
            let permissions = model.permissions.ok_or(ProtoError("'permissions' not set"))?.try_into_rust()?;
            Ok(Self { signed_receipt, permissions })
        }
    }

    /// A request to manage the permissions for a set of values from the network.
    #[derive(Clone, Debug, PartialEq)]
    pub struct UpdatePermissionsRequest {
        /// The receipt that proves this operation was paid for.
        pub signed_receipt: SignedReceipt,

        /// The delta of changes to be applied.
        pub delta: PermissionsDelta,
    }

    impl ConvertProto for UpdatePermissionsRequest {
        type ProtoType = super::proto::update::UpdatePermissionsRequest;

        fn into_proto(self) -> Self::ProtoType {
            Self::ProtoType {
                signed_receipt: Some(self.signed_receipt.into_proto()),
                retrieve: Some(self.delta.retrieve.into_proto()),
                update: Some(self.delta.update.into_proto()),
                delete: Some(self.delta.delete.into_proto()),
                compute: Some(self.delta.compute.into_proto()),
            }
        }

        fn try_from_proto(model: Self::ProtoType) -> Result<Self, ProtoError> {
            let signed_receipt = model.signed_receipt.ok_or(ProtoError("'signed_receipt' not set"))?.try_into_rust()?;
            Ok(Self {
                signed_receipt,
                delta: PermissionsDelta {
                    retrieve: model.retrieve.map(TryIntoRust::try_into_rust).transpose()?.unwrap_or_default(),
                    update: model.update.map(TryIntoRust::try_into_rust).transpose()?.unwrap_or_default(),
                    delete: model.delete.map(TryIntoRust::try_into_rust).transpose()?.unwrap_or_default(),
                    compute: model.compute.map(TryIntoRust::try_into_rust).transpose()?.unwrap_or_default(),
                },
            })
        }
    }

    /// A set of permissions delta to be applied to a set of values.
    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct PermissionsDelta {
        /// The retrieve values permissions to be granted/revoked.
        pub retrieve: PermissionCommand,

        /// The update values permissions to be granted/revoked.
        pub update: PermissionCommand,

        /// The delete values permissions to be granted/revoked.
        pub delete: PermissionCommand,

        /// The compute permissions to be granted.
        pub compute: ComputePermissionCommand,
    }

    /// A set of permissions delta to be applied to a set of values.
    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct PermissionCommand {
        /// The list of users that we're granting permissions to.
        pub grant: HashSet<UserId>,

        /// The list of users that we're revoking permissions from.
        pub revoke: HashSet<UserId>,
    }

    impl ConvertProto for PermissionCommand {
        type ProtoType = super::proto::update::PermissionCommand;

        fn into_proto(self) -> Self::ProtoType {
            Self::ProtoType {
                grant: self.grant.into_iter().map(UserId::into_proto).collect(),
                revoke: self.revoke.into_iter().map(UserId::into_proto).collect(),
            }
        }

        fn try_from_proto(model: Self::ProtoType) -> Result<Self, ProtoError> {
            Ok(Self {
                grant: model.grant.into_iter().map(UserId::try_from_proto).collect::<Result<_, _>>()?,
                revoke: model.revoke.into_iter().map(UserId::try_from_proto).collect::<Result<_, _>>()?,
            })
        }
    }

    /// A set of permissions delta to be applied to a set of values.
    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct ComputePermissionCommand {
        /// The list of users that we're granting permissions to.
        pub grant: ComputePermissions,

        /// The list of users that we're revoking permissions from.
        pub revoke: ComputePermissions,
    }

    impl ConvertProto for ComputePermissionCommand {
        type ProtoType = super::proto::update::ComputePermissionCommand;

        fn into_proto(self) -> Self::ProtoType {
            Self::ProtoType { grant: self.grant.into_proto(), revoke: self.revoke.into_proto() }
        }

        fn try_from_proto(model: Self::ProtoType) -> Result<Self, ProtoError> {
            Ok(Self { grant: model.grant.try_into_rust()?, revoke: model.revoke.try_into_rust()? })
        }
    }

    /// The permissions associated with a set of stored values.
    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct Permissions {
        /// The user id of the owner of the associated set of values.
        pub owner: UserId,

        /// The list of users that are allowed to retrieve the associated set of values.
        pub retrieve: HashSet<UserId>,

        /// The list of users that are allowed to update the associated set of values.
        pub update: HashSet<UserId>,

        /// The list of users that are allowed to delete the associated set of values.
        pub delete: HashSet<UserId>,

        /// The compute permissions.
        pub compute: ComputePermissions,
    }

    impl ConvertProto for Permissions {
        type ProtoType = super::proto::permissions::Permissions;

        fn into_proto(self) -> Self::ProtoType {
            Self::ProtoType {
                owner: Some(self.owner.into_proto()),
                retrieve: self.retrieve.into_iter().map(UserId::into_proto).collect(),
                update: self.update.into_iter().map(UserId::into_proto).collect(),
                delete: self.delete.into_iter().map(UserId::into_proto).collect(),
                compute: self.compute.into_proto(),
            }
        }

        fn try_from_proto(model: Self::ProtoType) -> Result<Self, ProtoError> {
            let owner = model.owner.ok_or(ProtoError("'owner' not set "))?;
            Ok(Self {
                owner: owner.try_into_rust()?,
                retrieve: model.retrieve.into_iter().map(UserId::try_from_proto).collect::<Result<_, _>>()?,
                update: model.update.into_iter().map(UserId::try_from_proto).collect::<Result<_, _>>()?,
                delete: model.delete.into_iter().map(UserId::try_from_proto).collect::<Result<_, _>>()?,
                compute: model.compute.try_into_rust()?,
            })
        }
    }

    /// A record of users permissions
    pub type ComputePermissions = HashMap<UserId, ComputePermission>;

    #[derive(Clone, Debug, Eq, PartialEq, Default)]
    pub struct ComputePermission {
        /// The identifiers of the programs
        pub program_ids: HashSet<String>,
    }

    impl ConvertProto for ComputePermissions {
        type ProtoType = Vec<super::proto::permissions::ComputePermissions>;

        fn into_proto(self) -> Self::ProtoType {
            self.into_iter()
                .map(|(user, permission)| super::proto::permissions::ComputePermissions {
                    user: Some(user.into_proto()),
                    program_ids: permission.program_ids.into_iter().collect(),
                })
                .collect()
        }

        fn try_from_proto(model: Self::ProtoType) -> Result<Self, ProtoError> {
            let permissions = model
                .into_iter()
                .map(|permission| {
                    permission.user.ok_or(ProtoError("'user' not set")).and_then(UserId::try_from_proto).map(|user| {
                        (user, ComputePermission { program_ids: permission.program_ids.into_iter().collect() })
                    })
                })
                .collect::<Result<_, _>>()?;
            Ok(permissions)
        }
    }
}

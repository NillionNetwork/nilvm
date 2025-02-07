//! User Values access service.

use super::blob::BlobService;
use crate::storage::{
    models::user_values::UserValuesRecord,
    repositories::{
        blob::BlobRepositoryError,
        blob_expirations::{BlobExpirationsRepository, ExpireableBlob, ExpireableBlobKind},
    },
    sqlite::{DatabaseError, TransactionError},
};
use async_trait::async_trait;
use chrono::Utc;
use futures::{stream::FuturesUnordered, StreamExt};
use node_api::{
    auth::rust::UserId,
    permissions::rust::{Permissions, PermissionsDelta},
};
use std::sync::Arc;
use tracing::{error, info};
use uuid::Uuid;

/// Service that interacts with :qwuser values.
///
/// Every operation in this service requires the accessing user id to have the required permissions
/// to perform that action.
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub(crate) trait UserValuesService: Send + Sync + 'static {
    /// Create user values if does not exist, or throw error if it does.
    async fn create_if_not_exists(
        &self,
        values_id: Uuid,
        values: UserValuesRecord,
    ) -> Result<(), UserValuesOperationError>;

    /// Create a user values or update existing one.
    async fn upsert(&self, values_id: Uuid, values: UserValuesRecord) -> Result<(), UserValuesOperationError>;

    /// Find a user values.
    async fn find(
        &self,
        values_id: Uuid,
        accessing_user_id: &UserId,
        reason: &UserValuesAccessReason,
    ) -> Result<UserValuesRecord, UserValuesOperationError>;

    /// Find a list of user values.
    async fn find_many(
        &self,
        values_ids: &[Uuid],
        accessing_user_id: &UserId,
        reason: &UserValuesAccessReason,
    ) -> Result<Vec<UserValuesRecord>, UserValuesOperationError>;

    /// Set the permissions for a user values.
    async fn set_permissions(
        &self,
        values_id: Uuid,
        accessing_user_id: &UserId,
        permissions: Permissions,
    ) -> Result<(), UserValuesOperationError>;

    /// Apply the given permissions delta to the values with the given id.
    async fn apply_permissions_delta(
        &self,
        values_id: Uuid,
        accessing_user_id: &UserId,
        delta: PermissionsDelta,
    ) -> Result<(), UserValuesOperationError>;

    /// Delete a user values.
    async fn delete(&self, values_id: Uuid, accessing_user_id: &UserId) -> Result<(), UserValuesOperationError>;

    /// Delete expired user values
    async fn delete_expired(&self) -> Result<u64, UserValuesOperationError>;
}

pub(crate) struct DefaultUserValuesService {
    blob_service: Box<dyn BlobService<UserValuesRecord>>,
    expiry_repo: Arc<dyn BlobExpirationsRepository>,
}

impl DefaultUserValuesService {
    pub(crate) fn new(
        blob_service: Box<dyn BlobService<UserValuesRecord>>,
        expiry_repo: Arc<dyn BlobExpirationsRepository>,
    ) -> Self {
        Self { blob_service, expiry_repo }
    }

    async fn fetch(&self, values_id: Uuid) -> Result<UserValuesRecord, UserValuesOperationError> {
        match self.blob_service.find_one(&values_id.to_string()).await {
            Ok(user_values) => {
                if user_values.expires_at <= chrono::offset::Utc::now() {
                    Err(UserValuesOperationError::Expired)
                } else {
                    Ok(user_values)
                }
            }
            Err(e) => Err(e.into()),
        }
    }

    /// Fetch user values and check permissions.
    ///
    /// Note: Permissions are stored within UserValues, so we can only check after fetching UserValues.
    async fn fetch_user_values_and_check_permissions(
        &self,
        values_id: Uuid,
        accessing_user_id: &UserId,
        reason: &UserValuesAccessReason,
    ) -> Result<UserValuesRecord, UserValuesOperationError> {
        let user_values = self.fetch(values_id).await?;
        reason.validate_permissions(accessing_user_id, &user_values.permissions)?;
        Ok(user_values)
    }

    /// Fetch a list of user values and check permissions.
    async fn fetch_user_values_many_and_check_permissions(
        &self,
        user_values_ids: &[Uuid],
        accessing_user_id: &UserId,
        reason: &UserValuesAccessReason,
    ) -> Result<Vec<UserValuesRecord>, UserValuesOperationError> {
        let mut futures = FuturesUnordered::new();

        for &user_value_id in user_values_ids {
            let fetch_future = async move {
                self.fetch_user_values_and_check_permissions(user_value_id, accessing_user_id, reason).await
            };
            futures.push(fetch_future);
        }

        let mut output_user_values = Vec::new();
        while let Some(result) = futures.next().await {
            let user_values = result?;
            output_user_values.push(user_values);
        }

        Ok(output_user_values)
    }

    /// Delete user values.
    ///
    /// Note: This function does not check permissions and is only meant to be used internally.
    async fn delete_user_values_unchecked(&self, user_values_id: Uuid) -> Result<(), UserValuesOperationError> {
        match self.blob_service.delete(&user_values_id.to_string()).await {
            Ok(_) | Err(BlobRepositoryError::NotFound) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    fn do_apply_delta(
        mut permissions: Permissions,
        accessing_user: &UserId,
        delta: PermissionsDelta,
    ) -> Result<Permissions, Unauthorized> {
        let is_owner = &permissions.owner == accessing_user;
        if !is_owner {
            return Err(Unauthorized);
        }
        // Grant all new permissions.
        permissions.retrieve.extend(delta.retrieve.grant);
        permissions.update.extend(delta.update.grant);
        permissions.delete.extend(delta.delete.grant);
        for (user, programs) in delta.compute.grant {
            permissions.compute.entry(user).or_default().program_ids.extend(programs.program_ids);
        }

        // Revoke all dropped permissions.
        for user in delta.retrieve.revoke {
            permissions.retrieve.remove(&user);
        }
        for user in delta.update.revoke {
            permissions.update.remove(&user);
        }
        for user in delta.delete.revoke {
            permissions.delete.remove(&user);
        }
        for (user, programs) in delta.compute.revoke {
            let Some(user_programs) = permissions.compute.get_mut(&user) else {
                continue;
            };
            for program in programs.program_ids {
                user_programs.program_ids.remove(&program);
            }
            if user_programs.program_ids.is_empty() {
                permissions.compute.remove(&user);
            }
        }
        Ok(permissions)
    }
}

#[async_trait]
impl UserValuesService for DefaultUserValuesService {
    /// Attempts to create a user values, failing if it already exists.
    async fn create_if_not_exists(&self, id: Uuid, record: UserValuesRecord) -> Result<(), UserValuesOperationError> {
        let expires_at = record.expires_at;
        self.blob_service.create(&id.to_string(), record).await?;

        // Store expiry date in value expiry repo
        let expiry_value = ExpireableBlob::new_user_value(id, expires_at);
        self.expiry_repo.upsert(&expiry_value).await?;
        Ok(())
    }

    async fn upsert(&self, id: Uuid, record: UserValuesRecord) -> Result<(), UserValuesOperationError> {
        let expires_at = record.expires_at;
        self.blob_service.upsert(&id.to_string(), record).await?;

        // Store expiry date in value expiry repo
        let expiry_value = ExpireableBlob::new_user_value(id, expires_at);
        self.expiry_repo.upsert(&expiry_value).await?;
        Ok(())
    }

    async fn find(
        &self,
        id: Uuid,
        accessing_user_id: &UserId,
        reason: &UserValuesAccessReason,
    ) -> Result<UserValuesRecord, UserValuesOperationError> {
        self.fetch_user_values_and_check_permissions(id, accessing_user_id, reason).await
    }

    async fn find_many(
        &self,
        id: &[Uuid],
        accessing_user_id: &UserId,
        reason: &UserValuesAccessReason,
    ) -> Result<Vec<UserValuesRecord>, UserValuesOperationError> {
        self.fetch_user_values_many_and_check_permissions(id, accessing_user_id, reason).await
    }

    async fn set_permissions(
        &self,
        id: Uuid,
        accessing_user_id: &UserId,
        permissions: Permissions,
    ) -> Result<(), UserValuesOperationError> {
        let mut user_values = self
            .fetch_user_values_and_check_permissions(id, accessing_user_id, &UserValuesAccessReason::UpdatePermissions)
            .await?;
        user_values.permissions = permissions;
        match self.blob_service.upsert(&id.to_string(), user_values).await {
            Ok(_) => Ok(()),
            Err(e) => Err(UserValuesOperationError::Internal(format!(
                "failed to update permissions for user_values {id}: {e}"
            ))),
        }
    }

    async fn apply_permissions_delta(
        &self,
        id: Uuid,
        accessing_user_id: &UserId,
        delta: PermissionsDelta,
    ) -> Result<(), UserValuesOperationError> {
        let mut user_values = self.fetch(id).await?;
        user_values.permissions = Self::do_apply_delta(user_values.permissions, accessing_user_id, delta)?;
        self.upsert(id, user_values).await?;
        Ok(())
    }

    /// Delete user values only if permissions allow it.
    async fn delete(&self, id: Uuid, accessing_user_id: &UserId) -> Result<(), UserValuesOperationError> {
        self.fetch_user_values_many_and_check_permissions(
            &[id],
            accessing_user_id,
            &UserValuesAccessReason::DeleteUserValues,
        )
        .await?;
        self.delete_user_values_unchecked(id).await
    }

    /// Delete expired user values
    async fn delete_expired(&self) -> Result<u64, UserValuesOperationError> {
        let now = Utc::now();
        info!("Deleting values that expire before {now}");
        let expired_user_values = self.expiry_repo.find_expired(ExpireableBlobKind::UserValue, now).await?;
        for user_value in expired_user_values {
            info!("Deleting user value with id: {}", user_value.key);
            self.delete_user_values_unchecked(user_value.key).await?;
        }

        // Delete expired entries from sqlite DB
        let count = self.expiry_repo.delete_expired(ExpireableBlobKind::UserValue, now).await?;
        Ok(count)
    }
}

/// The reason why a user values is being accessed.
#[derive(Debug, PartialEq)]
pub enum UserValuesAccessReason {
    /// We are trying to run a computation using this user values.
    Compute {
        /// The identifier of the program we want to use this user values on.
        program_id: String,
    },

    /// We are trying to retrieve this user values and reveal its secret.
    RetrieveUserValues,

    /// We are trying to retrieve the permissions for this user values.
    RetrievePermissions,

    /// We are trying to update this user values.
    UpdateUserValues,

    /// We are trying to update the permissions for this user values.
    UpdatePermissions,

    /// We are trying to delete this user values.
    DeleteUserValues,
}

impl UserValuesAccessReason {
    fn validate_permissions(&self, user: &UserId, permissions: &Permissions) -> Result<(), Unauthorized> {
        let can_access = match self {
            Self::Compute { program_id } => {
                permissions.compute.get(user).ok_or(Unauthorized)?.program_ids.contains(program_id)
            }
            Self::RetrieveUserValues | Self::RetrievePermissions => permissions.retrieve.contains(user),
            Self::UpdatePermissions => &permissions.owner == user,
            Self::UpdateUserValues => permissions.update.contains(user),
            Self::DeleteUserValues => permissions.delete.contains(user),
        };
        match can_access {
            true => Ok(()),
            false => Err(Unauthorized),
        }
    }
}

/// An error during a fetch user values operation.
#[derive(thiserror::Error, Debug)]
pub enum UserValuesOperationError {
    /// The user is not authorized to see these user values.
    #[error("user does not have permissions for action")]
    Unauthorized,

    /// The user values was not found.
    #[error("entity not found")]
    NotFound,

    /// An entity already exists.
    #[error("entity already exists")]
    AlreadyExists,

    /// An internal error ocurred.
    #[error("internal error: {0}")]
    Internal(String),

    // Value has expired.
    #[error("entity expired")]
    Expired,
}

impl From<BlobRepositoryError> for UserValuesOperationError {
    fn from(e: BlobRepositoryError) -> Self {
        match e {
            BlobRepositoryError::AlreadyExists => Self::AlreadyExists,
            BlobRepositoryError::NotFound => Self::NotFound,
            _ => Self::Internal(e.to_string()),
        }
    }
}

#[derive(thiserror::Error, Debug)]
#[error("unauthorized")]
struct Unauthorized;

impl From<Unauthorized> for UserValuesOperationError {
    fn from(_: Unauthorized) -> Self {
        Self::Unauthorized
    }
}

impl From<DatabaseError> for UserValuesOperationError {
    fn from(e: DatabaseError) -> Self {
        Self::Internal(e.to_string())
    }
}

impl From<TransactionError> for UserValuesOperationError {
    fn from(e: TransactionError) -> Self {
        Self::Internal(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use node_api::permissions::rust::{ComputePermission, ComputePermissionCommand, PermissionCommand};
    use rstest::rstest;

    fn empty_permissions(owner: UserId) -> Permissions {
        Permissions {
            owner,
            retrieve: Default::default(),
            update: Default::default(),
            delete: Default::default(),
            compute: Default::default(),
        }
    }

    #[test]
    fn permissions_delta() {
        let user_id = UserId::from_bytes("owner");
        let permissions = Permissions {
            owner: user_id,
            retrieve: [UserId::from_bytes("r1"), UserId::from_bytes("r2")].into(),
            update: [UserId::from_bytes("u1"), UserId::from_bytes("u2")].into(),
            delete: [UserId::from_bytes("d1"), UserId::from_bytes("d2")].into(),
            compute: [
                (UserId::from_bytes("c1"), ComputePermission { program_ids: ["p1".into(), "p2".into()].into() }),
                (UserId::from_bytes("c2"), ComputePermission { program_ids: ["p3".into(), "p4".into()].into() }),
            ]
            .into(),
        };
        let delta = PermissionsDelta {
            retrieve: PermissionCommand {
                grant: [UserId::from_bytes("r3"), UserId::from_bytes("r4")].into(),
                revoke: [UserId::from_bytes("r2")].into(),
            },
            update: PermissionCommand {
                grant: [UserId::from_bytes("u3"), UserId::from_bytes("u4")].into(),
                revoke: [UserId::from_bytes("u2")].into(),
            },
            delete: PermissionCommand {
                grant: [UserId::from_bytes("d3"), UserId::from_bytes("d4")].into(),
                revoke: [UserId::from_bytes("d2")].into(),
            },
            compute: ComputePermissionCommand {
                grant: [
                    (UserId::from_bytes("c1"), ComputePermission { program_ids: ["p5".into()].into() }),
                    (UserId::from_bytes("c3"), ComputePermission { program_ids: ["p6".into()].into() }),
                ]
                .into(),
                revoke: [
                    (UserId::from_bytes("c1"), ComputePermission { program_ids: ["p1".into()].into() }),
                    (UserId::from_bytes("c2"), ComputePermission { program_ids: ["p3".into(), "p4".into()].into() }),
                ]
                .into(),
            },
        };
        let expected = Permissions {
            owner: UserId::from_bytes("owner"),
            retrieve: [UserId::from_bytes("r1"), UserId::from_bytes("r3"), UserId::from_bytes("r4")].into(),
            update: [UserId::from_bytes("u1"), UserId::from_bytes("u3"), UserId::from_bytes("u4")].into(),
            delete: [UserId::from_bytes("d1"), UserId::from_bytes("d3"), UserId::from_bytes("d4")].into(),
            compute: [
                (UserId::from_bytes("c1"), ComputePermission { program_ids: ["p2".into(), "p5".into()].into() }),
                (UserId::from_bytes("c3"), ComputePermission { program_ids: ["p6".into()].into() }),
            ]
            .into(),
        };
        let permissions = DefaultUserValuesService::do_apply_delta(permissions, &user_id, delta).expect("apply failed");
        assert_eq!(permissions, expected);
    }

    #[test]
    fn permissions_delta_permission_denied() {
        let permissions = Permissions {
            owner: UserId::from_bytes("owner"),
            retrieve: [UserId::from_bytes("r1"), UserId::from_bytes("r2")].into(),
            update: [UserId::from_bytes("u1"), UserId::from_bytes("u2")].into(),
            delete: [UserId::from_bytes("d1"), UserId::from_bytes("d2")].into(),
            compute: [
                (UserId::from_bytes("c1"), ComputePermission { program_ids: ["p1".into(), "p2".into()].into() }),
                (UserId::from_bytes("c2"), ComputePermission { program_ids: ["p3".into(), "p4".into()].into() }),
            ]
            .into(),
        };
        let delta = PermissionsDelta::default();
        DefaultUserValuesService::do_apply_delta(permissions, &UserId::from_bytes("other"), delta)
            .expect_err("apply succeeded");
    }

    #[rstest]
    #[case::compute(UserValuesAccessReason::Compute{ program_id: "".into() })]
    #[case::retrieve_values(UserValuesAccessReason::RetrieveUserValues)]
    #[case::retrieve_permissions(UserValuesAccessReason::RetrievePermissions)]
    #[case::update_values(UserValuesAccessReason::UpdateUserValues)]
    #[case::update_permissions(UserValuesAccessReason::UpdatePermissions)]
    #[case::delete_values(UserValuesAccessReason::DeleteUserValues)]
    fn no_permissions_access_denied(#[case] reason: UserValuesAccessReason) {
        let owner = UserId::from_bytes("owner");
        let permissions = empty_permissions(owner);

        let other = UserId::from_bytes("other");
        reason.validate_permissions(&other, &permissions).expect_err("permissions granted");
    }

    #[test]
    fn retrieve_values_permissions() {
        let owner = UserId::from_bytes("owner");
        let other = UserId::from_bytes("other");
        let mut permissions = empty_permissions(owner);
        permissions.retrieve.insert(other);

        UserValuesAccessReason::RetrieveUserValues
            .validate_permissions(&other, &permissions)
            .expect("permission denied");
        UserValuesAccessReason::RetrievePermissions
            .validate_permissions(&other, &permissions)
            .expect("permission denied");
    }

    #[test]
    fn update_permissions() {
        let owner = UserId::from_bytes("owner");
        let other = UserId::from_bytes("other");
        let permissions = empty_permissions(owner);

        UserValuesAccessReason::UpdatePermissions
            .validate_permissions(&owner, &permissions)
            .expect("permission denied");
        UserValuesAccessReason::UpdatePermissions
            .validate_permissions(&other, &permissions)
            .expect_err("permission granted");
    }

    #[test]
    fn update_values_permissions() {
        let owner = UserId::from_bytes("owner");
        let other = UserId::from_bytes("other");
        let mut permissions = empty_permissions(owner);
        permissions.update.insert(other);

        UserValuesAccessReason::UpdateUserValues.validate_permissions(&other, &permissions).expect("permission denied");
    }

    #[test]
    fn delete_values_permissions() {
        let owner = UserId::from_bytes("owner");
        let other = UserId::from_bytes("other");
        let mut permissions = empty_permissions(owner);
        permissions.delete.insert(other);

        UserValuesAccessReason::DeleteUserValues.validate_permissions(&other, &permissions).expect("permission denied");
    }

    #[test]
    fn compute_permissions() {
        let owner = UserId::from_bytes("owner");
        let other = UserId::from_bytes("other");
        let mut permissions = empty_permissions(owner);
        permissions.compute.insert(other, ComputePermission { program_ids: ["b".into()].into() });

        UserValuesAccessReason::Compute { program_id: "a".into() }
            .validate_permissions(&other, &permissions)
            .expect_err("permission denied");
        UserValuesAccessReason::Compute { program_id: "b".into() }
            .validate_permissions(&other, &permissions)
            .expect("permission granted");
    }
}

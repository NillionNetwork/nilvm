use crate::storage::{
    metrics::ExportMetrics,
    sqlite::{DatabaseError, SqliteDb},
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::prelude::FromRow;
use strum::{Display, EnumString};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, EnumString, Display, PartialEq, sqlx::Type)]
pub(crate) enum ExpireableBlobKind {
    UserValue,
    ComputeResult,
}

#[derive(Clone, Debug, PartialEq, FromRow)]
pub(crate) struct ExpireableBlob {
    pub(crate) key: Uuid,
    pub(crate) kind: ExpireableBlobKind,
    pub(crate) expires_at: DateTime<Utc>,
}

impl ExpireableBlob {
    pub(crate) fn new(key: Uuid, kind: ExpireableBlobKind, expires_at: DateTime<Utc>) -> Self {
        Self { key, kind, expires_at }
    }

    pub(crate) fn new_user_value(key: Uuid, expires_at: DateTime<Utc>) -> Self {
        Self::new(key, ExpireableBlobKind::UserValue, expires_at)
    }

    pub(crate) fn new_compute_result(key: Uuid, expires_at: DateTime<Utc>) -> Self {
        Self::new(key, ExpireableBlobKind::ComputeResult, expires_at)
    }
}

#[async_trait]
pub(crate) trait BlobExpirationsRepository: Send + Sync + 'static {
    /// Inserts or updates an expireable blob
    async fn upsert(&self, value: &ExpireableBlob) -> Result<(), DatabaseError>;

    /// Find all blobs that are expired based on the given timestamp.
    async fn find_expired(
        &self,
        kind: ExpireableBlobKind,
        threshold: DateTime<Utc>,
    ) -> Result<Vec<ExpireableBlob>, DatabaseError>;

    /// Delete all records of blobs that are older than the supplied threshold.
    ///
    /// That is, any blobs with `expires_at < threshold` will be removed.
    async fn delete_expired(&self, kind: ExpireableBlobKind, threshold: DateTime<Utc>) -> Result<u64, DatabaseError>;
}

pub(crate) struct SqliteBlobExpirationsRepository(SqliteDb);

impl SqliteBlobExpirationsRepository {
    pub(crate) fn new(db: SqliteDb) -> Self {
        Self(db)
    }
}

#[async_trait]
impl BlobExpirationsRepository for SqliteBlobExpirationsRepository {
    async fn upsert(&self, value: &ExpireableBlob) -> Result<(), DatabaseError> {
        let ExpireableBlob { key, kind, expires_at } = value;
        let query = sqlx::query(
            "INSERT INTO blob_expirations(key, kind, expires_at) VALUES (?, ?, ?)
             ON CONFLICT DO UPDATE SET expires_at = excluded.expires_at",
        )
        .bind(key)
        .bind(kind)
        .bind(expires_at);
        self.0.execute(query, &mut Default::default()).await?;
        Ok(())
    }

    async fn find_expired(
        &self,
        kind: ExpireableBlobKind,
        threshold: DateTime<Utc>,
    ) -> Result<Vec<ExpireableBlob>, DatabaseError> {
        let query =
            sqlx::query_as("SELECT * FROM blob_expirations WHERE kind = ? AND expires_at < ? ORDER BY expires_at ASC")
                .bind(kind)
                .bind(threshold);
        let results = self.0.fetch_all::<ExpireableBlob>(query, &mut Default::default()).await?;
        Ok(results)
    }

    async fn delete_expired(&self, kind: ExpireableBlobKind, threshold: DateTime<Utc>) -> Result<u64, DatabaseError> {
        let query =
            sqlx::query("DELETE FROM blob_expirations WHERE kind = ? AND expires_at < ?").bind(kind).bind(threshold);
        Ok(self.0.execute(query, &mut Default::default()).await?)
    }
}

#[async_trait]
impl ExportMetrics for SqliteBlobExpirationsRepository {
    async fn export_metrics(&self) -> anyhow::Result<()> {
        self.0.export_table_metrics("blob_expirations").await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use std::time::Duration;

    async fn make_repo() -> SqliteBlobExpirationsRepository {
        let db = SqliteDb::new("sqlite::memory:").await.expect("repo creation failed");
        SqliteBlobExpirationsRepository::new(db)
    }

    #[rstest]
    #[case::user_value(ExpireableBlobKind::UserValue)]
    #[case::compute_result(ExpireableBlobKind::ComputeResult)]
    #[tokio::test]
    async fn delete_expired_values(#[case] kind: ExpireableBlobKind) {
        let repo = make_repo().await;
        let now = Utc::now();
        let expired_timestamp = now - Duration::from_secs(1);
        let valid_timestamp = now + Duration::from_secs(1);

        // insert 1 expired value
        repo.upsert(&ExpireableBlob::new(Uuid::new_v4(), kind, expired_timestamp)).await.unwrap();

        // insert 2 valid values
        repo.upsert(&ExpireableBlob::new(Uuid::new_v4(), kind, valid_timestamp)).await.unwrap();
        repo.upsert(&ExpireableBlob::new(Uuid::new_v4(), kind, valid_timestamp)).await.unwrap();

        // delete expired values and expect only one to be deleted
        let deleted = repo.delete_expired(kind, now).await.expect("delete failed");
        assert_eq!(deleted, 1);
    }

    #[tokio::test]
    async fn deletion_respects_kinds() {
        let repo = make_repo().await;
        let now = Utc::now();
        let expired_timestamp = now - Duration::from_secs(1);

        // insert 1 expired value for each kind
        repo.upsert(&ExpireableBlob::new_user_value(Uuid::new_v4(), expired_timestamp)).await.unwrap();
        repo.upsert(&ExpireableBlob::new_compute_result(Uuid::new_v4(), expired_timestamp)).await.unwrap();

        // only 1 should be deleted in each case
        let deleted = repo.delete_expired(ExpireableBlobKind::UserValue, now).await.expect("delete failed");
        assert_eq!(deleted, 1);

        let deleted = repo.delete_expired(ExpireableBlobKind::ComputeResult, now).await.expect("delete failed");
        assert_eq!(deleted, 1);
    }
}

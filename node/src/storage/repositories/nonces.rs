use crate::{
    services::payments::Nonce,
    storage::{
        metrics::ExportMetrics,
        sqlite::{DatabaseError, SqliteDb, TransactionContext},
    },
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum ExpireableNonceKind {
    Quote,
    Receipt,
}

impl TryFrom<i32> for ExpireableNonceKind {
    type Error = ();

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Quote),
            1 => Ok(Self::Receipt),
            _ => Err(()),
        }
    }
}

impl From<ExpireableNonceKind> for i32 {
    fn from(kind: ExpireableNonceKind) -> Self {
        use ExpireableNonceKind::*;
        match kind {
            Quote => 0,
            Receipt => 1,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ExpireableNonce {
    pub(crate) nonce: Nonce,
    pub(crate) expires_at: DateTime<Utc>,
    pub(crate) kind: ExpireableNonceKind,
}

impl ExpireableNonce {
    pub(crate) fn new_quote(nonce: Nonce, expires_at: DateTime<Utc>) -> Self {
        Self { nonce, expires_at, kind: ExpireableNonceKind::Quote }
    }

    pub(crate) fn new_receipt(nonce: Nonce, expires_at: DateTime<Utc>) -> Self {
        Self { nonce, expires_at, kind: ExpireableNonceKind::Receipt }
    }
}

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub(crate) trait UsedNoncesRepository: Send + Sync + 'static {
    /// Inserts a nonce.
    ///
    /// This will fail if this nonce has already been previously inserted.
    async fn insert<'a>(
        &'a self,
        nonce: &ExpireableNonce,
        context: &mut TransactionContext<'a>,
    ) -> Result<(), DatabaseError>;

    /// Delete all nonces that are expired based on the given threshold.
    ///
    /// That is, any nonces with `expires_at < threshold` will be removed.
    async fn delete_expired<'a>(
        &'a self,
        threshold: DateTime<Utc>,
        context: &mut TransactionContext<'a>,
    ) -> Result<u64, DatabaseError>;
}

pub(crate) struct SqliteUsedNoncesRepository(SqliteDb);

impl SqliteUsedNoncesRepository {
    pub(crate) fn new(db: SqliteDb) -> Self {
        Self(db)
    }
}

#[async_trait]
impl UsedNoncesRepository for SqliteUsedNoncesRepository {
    async fn insert(&self, nonce: &ExpireableNonce, context: &mut TransactionContext) -> Result<(), DatabaseError> {
        let ExpireableNonce { nonce, expires_at, kind } = nonce;
        let query = sqlx::query("INSERT INTO used_nonces(nonce, kind, expires_at) VALUES (?, ?, ?)")
            .bind(&nonce.0)
            .bind(i32::from(*kind))
            .bind(expires_at);
        self.0.execute(query, context).await?;
        Ok(())
    }

    async fn delete_expired(
        &self,
        threshold: DateTime<Utc>,
        context: &mut TransactionContext,
    ) -> Result<u64, DatabaseError> {
        let query = sqlx::query("DELETE FROM used_nonces WHERE expires_at < ?").bind(threshold);
        Ok(self.0.execute(query, context).await?)
    }
}

#[async_trait]
impl ExportMetrics for SqliteUsedNoncesRepository {
    async fn export_metrics(&self) -> anyhow::Result<()> {
        self.0.export_table_metrics("used_nonces").await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    async fn make_repo() -> SqliteUsedNoncesRepository {
        let db = SqliteDb::new("sqlite::memory:").await.expect("repo creation failed");
        SqliteUsedNoncesRepository::new(db)
    }

    #[tokio::test]
    async fn quote_uniqueness() {
        let repo = make_repo().await;
        let nonce =
            ExpireableNonce { nonce: Nonce(vec![1, 2, 3]), expires_at: Utc::now(), kind: ExpireableNonceKind::Quote };
        repo.insert(&nonce, &mut Default::default()).await.expect("insertion failed");

        // inserting should cause an error to be raised
        let error = repo.insert(&nonce, &mut Default::default()).await.expect_err("already used insertion succeeded");
        assert!(matches!(error, DatabaseError::UniqueConstraint), "unexpected error {error}");
    }

    #[tokio::test]
    async fn delete_expired() {
        let repo = make_repo().await;
        let now = Utc::now();
        let expired_timestamp = now - Duration::from_secs(1);
        let valid_timestamp = now + Duration::from_secs(1);
        // insert 1 to be removed
        repo.insert(
            &ExpireableNonce { nonce: Nonce(vec![0]), expires_at: expired_timestamp, kind: ExpireableNonceKind::Quote },
            &mut Default::default(),
        )
        .await
        .unwrap();
        // and 2 that are valid
        repo.insert(
            &ExpireableNonce { nonce: Nonce(vec![1]), expires_at: valid_timestamp, kind: ExpireableNonceKind::Quote },
            &mut Default::default(),
        )
        .await
        .unwrap();
        repo.insert(
            &ExpireableNonce { nonce: Nonce(vec![2]), expires_at: valid_timestamp, kind: ExpireableNonceKind::Quote },
            &mut Default::default(),
        )
        .await
        .unwrap();

        // delete expired ones and expect only one to be deleted
        let deleted = repo.delete_expired(now, &mut Default::default()).await.expect("delete failed");
        assert_eq!(deleted, 1);
    }
}

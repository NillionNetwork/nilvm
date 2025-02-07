use crate::storage::{
    metrics::ExportMetrics,
    sqlite::{DatabaseError, SqliteDb, TransactionContext},
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use node_api::auth::rust::UserId;
use sqlx::FromRow;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct AccountBalance {
    pub(crate) account: UserId,
    pub(crate) balance: u64,
    pub(crate) updated_at: DateTime<Utc>,
}

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub(crate) trait AccountBalanceRepository: Send + Sync + 'static {
    async fn begin_transaction<'a>(&'a self) -> Result<TransactionContext<'a>, DatabaseError>;

    async fn find<'a>(
        &'a self,
        user_id: &UserId,
        context: &mut TransactionContext<'a>,
    ) -> Result<Option<AccountBalance>, DatabaseError>;

    async fn add_funds<'a>(
        &'a self,
        user_id: &UserId,
        funds: i64,
        context: &mut TransactionContext<'a>,
    ) -> Result<(), DatabaseError>;

    async fn remove_funds<'a>(
        &'a self,
        user_id: &UserId,
        funds: i64,
        context: &mut TransactionContext<'a>,
    ) -> Result<(), DatabaseError>;

    async fn remove_expired(&self, threshold: DateTime<Utc>) -> Result<u64, DatabaseError>;
}

pub(crate) struct SqliteAccountBalanceRepository(SqliteDb);

impl SqliteAccountBalanceRepository {
    pub(crate) fn new(handle: SqliteDb) -> Self {
        Self(handle)
    }
}

#[async_trait]
impl AccountBalanceRepository for SqliteAccountBalanceRepository {
    async fn begin_transaction(&self) -> Result<TransactionContext, DatabaseError> {
        let tx = self.0.begin_tx().await?;
        Ok(TransactionContext::Transaction(tx))
    }

    async fn find(
        &self,
        user_id: &UserId,
        context: &mut TransactionContext,
    ) -> Result<Option<AccountBalance>, DatabaseError> {
        #[derive(FromRow)]
        struct Row {
            account: String,
            balance: u64,
            updated_at: DateTime<Utc>,
        }
        let query = sqlx::query_as("SELECT * FROM account_balances WHERE account = ?").bind(user_id.to_string());
        let row: Option<Row> = self.0.fetch_one(query, context).await?;
        match row {
            Some(row) => {
                let Row { account, balance, updated_at } = row;
                let account = account.parse().map_err(|e| {
                    DatabaseError::Execution(sqlx::Error::ColumnDecode {
                        index: "account".to_string(),
                        source: Box::new(e),
                    })
                })?;
                Ok(Some(AccountBalance { account, balance, updated_at }))
            }
            None => Ok(None),
        }
    }

    async fn add_funds(
        &self,
        user_id: &UserId,
        funds: i64,
        context: &mut TransactionContext,
    ) -> Result<(), DatabaseError> {
        let query = sqlx::query(
            "INSERT INTO account_balances(account, balance, updated_at) VALUES(?, ?, UNIXEPOCH()) 
ON CONFLICT (account) DO UPDATE SET balance = balance + ?, updated_at = UNIXEPOCH()",
        )
        .bind(user_id.to_string())
        .bind(funds)
        .bind(funds);
        self.0.execute(query, context).await?;
        Ok(())
    }

    async fn remove_funds<'a>(
        &'a self,
        user_id: &UserId,
        funds: i64,
        context: &mut TransactionContext<'a>,
    ) -> Result<(), DatabaseError> {
        let query = sqlx::query(
            "UPDATE account_balances SET balance = balance - ?, updated_at = UNIXEPOCH() WHERE account = ?",
        )
        .bind(funds)
        .bind(user_id.to_string());
        let rows_affected = self.0.execute(query, context).await?;
        if rows_affected != 1 {
            return Err(DatabaseError::Execution(sqlx::Error::RowNotFound));
        }
        Ok(())
    }

    async fn remove_expired(&self, threshold: DateTime<Utc>) -> Result<u64, DatabaseError> {
        let query = sqlx::query("DELETE FROM account_balances WHERE updated_at < ?").bind(threshold.timestamp());
        self.0.execute(query, &mut Default::default()).await
    }
}

#[async_trait]
impl ExportMetrics for SqliteAccountBalanceRepository {
    async fn export_metrics(&self) -> anyhow::Result<()> {
        self.0.export_table_metrics("account_balances").await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::sqlite::SqliteDb;
    use std::time::Duration;

    async fn make_repo() -> SqliteAccountBalanceRepository {
        let handle = SqliteDb::new("sqlite::memory:").await.expect("db creation failed");
        SqliteAccountBalanceRepository::new(handle)
    }

    #[tokio::test]
    async fn lookup_non_existent() {
        let repo = make_repo().await;
        let result = repo.find(&UserId::from_bytes(b"foo"), &mut Default::default()).await.expect("lookup failed");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn remove_balance_non_existent() {
        let repo = make_repo().await;
        repo.remove_funds(&UserId::from_bytes(b"foo"), 1, &mut Default::default()).await.expect_err("funds removed");
    }

    #[tokio::test]
    async fn update() {
        let repo = make_repo().await;
        let user_id = UserId::from_bytes(b"foo");
        repo.add_funds(&user_id, 42, &mut Default::default()).await.expect("failed to add funds");
        repo.add_funds(&user_id, 10, &mut Default::default()).await.expect("failed to add funds");

        let result = repo.find(&user_id, &mut Default::default()).await.expect("lookup failed");
        let account = result.expect("not found");
        assert_eq!(account.account, user_id);
        assert_eq!(account.balance, 52);

        repo.remove_funds(&user_id, 5, &mut Default::default()).await.expect("failed to remove funds");

        let result = repo.find(&user_id, &mut Default::default()).await.expect("lookup failed");
        let account = result.expect("not found");
        assert_eq!(account.balance, 47);

        let err = repo
            .remove_funds(&user_id, 100, &mut Default::default())
            .await
            .expect_err("succeeded pulling off too many funds");
        assert!(matches!(err, DatabaseError::Constraint));
    }

    #[tokio::test]
    async fn updated_timestamp() {
        let repo = make_repo().await;
        let user_id = UserId::from_bytes(b"foo");
        let get_timestamp =
            || async { repo.find(&user_id, &mut Default::default()).await.expect("lookup failed").unwrap().updated_at };
        // We reset the timestamp to ensure it's updated. We could also sleep but we don't want to
        // stall tests for over a second (that's the precision on this column).
        let reset_timestamp = || async {
            let query = sqlx::query("UPDATE account_balances SET updated_at = datetime(0)");
            repo.0.execute(query, &mut Default::default()).await.expect("failed to reset timestamp");
        };
        repo.add_funds(&user_id, 1, &mut Default::default()).await.expect("failed to add funds");

        let last_timestamp = get_timestamp().await;
        reset_timestamp().await;
        repo.add_funds(&user_id, 1, &mut Default::default()).await.expect("failed to add funds");
        let current_timestamp = get_timestamp().await;
        assert!(current_timestamp >= last_timestamp, "{current_timestamp} < {last_timestamp}");

        let last_timestamp = current_timestamp;
        reset_timestamp().await;
        repo.remove_funds(&user_id, 1, &mut Default::default()).await.expect("failed to add funds");
        let current_timestamp = get_timestamp().await;
        assert!(current_timestamp >= last_timestamp, "{current_timestamp} < {last_timestamp}");
    }

    #[tokio::test]
    async fn remove_expired() {
        let repo = make_repo().await;
        let user_id1 = UserId::from_bytes(b"foo");
        let user_id2 = UserId::from_bytes(b"bar");
        // leave a bit of leeway just in case
        let threshold = Utc::now() - Duration::from_secs(3600 * 9);
        repo.add_funds(&user_id1, 1, &mut Default::default()).await.expect("failed to add funds");
        repo.add_funds(&user_id2, 1, &mut Default::default()).await.expect("failed to add funds");

        // Move user_id1's timestamp to way before
        let query =
            sqlx::query("UPDATE account_balances SET updated_at = 0 WHERE account = ?").bind(user_id1.to_string());
        repo.0.execute(query, &mut Default::default()).await.expect("failed to reset timestamp");

        // this should only remove user_id1
        let total = repo.remove_expired(threshold).await.expect("failed to remove");
        assert_eq!(total, 1);

        assert!(
            repo.find(&user_id1, &mut Default::default()).await.expect("lookup failed").is_none(),
            "user_id1 still exists"
        );
        assert!(
            repo.find(&user_id2, &mut Default::default()).await.expect("lookup failed").is_some(),
            "user_id2 does not exist"
        );
    }
}

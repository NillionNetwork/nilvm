use crate::storage::{
    metrics::ExportMetrics,
    sqlite::{DatabaseError, SqliteDb, TransactionContext},
};
use async_trait::async_trait;
use node_api::auth::rust::UserId;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Transfer {
    pub(crate) tx_hash: String,
    pub(crate) account: UserId,
    pub(crate) amount: i64,
}

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub(crate) trait TransfersRepository: Send + Sync + 'static {
    async fn insert<'a>(
        &'a self,
        transfer: Transfer,
        context: &mut TransactionContext<'a>,
    ) -> Result<(), DatabaseError>;
}

pub(crate) struct SqliteTransfersRepository(SqliteDb);

impl SqliteTransfersRepository {
    pub(crate) fn new(db: SqliteDb) -> Self {
        Self(db)
    }
}

#[async_trait]
impl TransfersRepository for SqliteTransfersRepository {
    async fn insert(&self, transfer: Transfer, context: &mut TransactionContext) -> Result<(), DatabaseError> {
        let Transfer { tx_hash, account, amount } = transfer;
        let query = sqlx::query("INSERT INTO add_funds_transfers (tx_hash, account, amount) VALUES (?, ?, ?)")
            .bind(tx_hash)
            .bind(account.to_string())
            .bind(amount);
        self.0.execute(query, context).await?;
        Ok(())
    }
}

#[async_trait]
impl ExportMetrics for SqliteTransfersRepository {
    async fn export_metrics(&self) -> anyhow::Result<()> {
        self.0.export_table_metrics("add_funds_transfers").await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::FromRow;

    #[tokio::test]
    async fn insert_transfer() {
        let db = SqliteDb::new("sqlite::memory:").await.expect("db creation failed");
        let repo = SqliteTransfersRepository::new(db);
        let transfer = Transfer { tx_hash: "foo".into(), account: UserId::from_bytes(b"bob"), amount: 42 };
        repo.insert(transfer.clone(), &mut Default::default()).await.expect("insert failed");
        // Shouldn't be allowed because of primary key on tx hash
        repo.insert(transfer.clone(), &mut Default::default()).await.expect_err("duplicate insert succeeded");

        #[derive(FromRow)]
        struct Row {
            account: String,
            amount: i64,
        }

        let row: Row = repo
            .0
            .fetch_one(
                sqlx::query_as("SELECT account, amount FROM add_funds_transfers WHERE tx_hash = ?")
                    .bind(transfer.tx_hash),
                &mut Default::default(),
            )
            .await
            .expect("failed to find")
            .expect("nothing found");
        assert_eq!(row.account, transfer.account.to_string());
        assert_eq!(row.amount, transfer.amount);
    }
}

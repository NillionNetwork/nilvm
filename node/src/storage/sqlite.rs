use metrics::prelude::*;
use once_cell::sync::Lazy;
use sqlx::{
    pool::PoolConnection,
    prelude::FromRow,
    query::{Query, QueryAs},
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
    Database, Sqlite, Transaction,
};
use std::{ops::DerefMut, path::Path, str::FromStr};
use tracing::info;

static METRICS: Lazy<Metrics> = Lazy::new(Metrics::default);
const WRITE_POOL_CONNECTIONS: u32 = 1;
const READ_POOL_MIN_CONNECTIONS: u32 = 1;
const READ_POOL_MAX_CONNECTIONS: u32 = 16;

/// A sqlite database.
#[derive(Clone)]
pub(crate) struct SqliteDb {
    read_pool: sqlx::SqlitePool,
    write_pool: sqlx::SqlitePool,
}

impl SqliteDb {
    pub(crate) async fn new(url: &str) -> Result<Self, sqlx::Error> {
        info!("Creating sqlite repository using url {url}");
        let connect_options = SqliteConnectOptions::from_str(url)?;
        let write_pool =
            Self::create_pool(connect_options.clone(), WRITE_POOL_CONNECTIONS, WRITE_POOL_CONNECTIONS).await?;
        let read_pool =
            Self::create_pool(connect_options.read_only(true), READ_POOL_MIN_CONNECTIONS, READ_POOL_MAX_CONNECTIONS)
                .await?;
        info!("Applying sqlite migrations");
        sqlx::migrate!().run(&write_pool).await?;
        info!("All sqlite migrations applied");
        Ok(Self { read_pool, write_pool })
    }

    #[cfg(test)]
    pub(crate) async fn in_memory() -> Result<Self, sqlx::Error> {
        Self::new("sqlite://:memory:").await
    }

    pub(crate) async fn begin_tx(&self) -> Result<Transaction<Sqlite>, DatabaseError> {
        self.write_pool.begin().await.map_err(DatabaseError::ConnectionAcquire)
    }

    pub(crate) async fn fetch_one<'a, O>(
        &self,
        query: QueryAs<'a, Sqlite, O, <Sqlite as Database>::Arguments<'a>>,
        context: &mut TransactionContext<'_>,
    ) -> Result<Option<O>, DatabaseError>
    where
        O: Send + Unpin + for<'r> FromRow<'r, <Sqlite as Database>::Row>,
    {
        match context {
            TransactionContext::Transaction(tx) => {
                let tx = tx.deref_mut();
                query.fetch_optional(tx).await.map_err(DatabaseError::Execution)
            }
            TransactionContext::None => {
                let mut conn = self.acquire_read().await?;
                query.fetch_optional(&mut *conn).await.map_err(DatabaseError::Execution)
            }
        }
    }

    pub(crate) async fn fetch_all<'a, O>(
        &self,
        query: QueryAs<'a, Sqlite, O, <Sqlite as Database>::Arguments<'a>>,
        context: &mut TransactionContext<'_>,
    ) -> Result<Vec<O>, DatabaseError>
    where
        O: Send + Unpin + for<'r> FromRow<'r, <Sqlite as Database>::Row>,
    {
        match context {
            TransactionContext::Transaction(tx) => {
                let tx = tx.deref_mut();
                query.fetch_all(tx).await.map_err(DatabaseError::Execution)
            }
            TransactionContext::None => {
                let mut conn = self.acquire_read().await?;
                query.fetch_all(&mut *conn).await.map_err(DatabaseError::Execution)
            }
        }
    }

    pub(crate) async fn execute<'a>(
        &self,
        query: Query<'a, Sqlite, <Sqlite as Database>::Arguments<'a>>,
        context: &mut TransactionContext<'_>,
    ) -> Result<u64, DatabaseError> {
        let result = match context {
            TransactionContext::Transaction(tx) => {
                let tx = tx.deref_mut();
                query.execute(tx).await
            }
            TransactionContext::None => {
                let mut conn = self.acquire_write().await?;
                query.execute(&mut *conn).await
            }
        };
        match result {
            Ok(result) => Ok(result.rows_affected()),
            Err(sqlx::Error::Database(e)) if e.is_unique_violation() => Err(DatabaseError::UniqueConstraint),
            Err(sqlx::Error::Database(e)) if e.is_check_violation() => Err(DatabaseError::Constraint),
            Err(e) => Err(DatabaseError::Execution(e)),
        }
    }

    pub(crate) async fn export_table_metrics(&self, table: &str) -> anyhow::Result<()> {
        #[derive(FromRow)]
        struct TableSize {
            size: i64,
        }

        let query = "SELECT sum(pgsize) as size FROM dbstat WHERE name = ?";
        let query = sqlx::query_as(query).bind(table);
        let mut conn = self.acquire_read().await?;
        let result: TableSize = query.fetch_one(&mut *conn).await?;
        METRICS.table_size.with_labels([("table", table)]).set(result.size);
        Ok(())
    }

    async fn acquire_read(&self) -> Result<PoolConnection<Sqlite>, DatabaseError> {
        self.read_pool.acquire().await.map_err(DatabaseError::ConnectionAcquire)
    }

    async fn acquire_write(&self) -> Result<PoolConnection<Sqlite>, DatabaseError> {
        self.write_pool.acquire().await.map_err(DatabaseError::ConnectionAcquire)
    }

    async fn create_pool(
        connect_options: SqliteConnectOptions,
        min_connections: u32,
        max_connections: u32,
    ) -> Result<sqlx::SqlitePool, sqlx::Error> {
        let connect_options = connect_options.create_if_missing(true).journal_mode(SqliteJournalMode::Wal);
        let mut pool_options =
            SqlitePoolOptions::new().min_connections(min_connections).max_connections(max_connections);
        if connect_options.get_filename() == Path::new(":memory:") {
            // if we don't do this eventually the database gets dropped and tables disappear.
            pool_options = pool_options.max_lifetime(None).idle_timeout(None)
        }
        pool_options.connect_with(connect_options).await
    }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum DatabaseError {
    #[error("acquiring connection: {0}")]
    ConnectionAcquire(sqlx::Error),

    #[error("entry not found")]
    NotFound,

    #[error("executing query: {0}")]
    Execution(sqlx::Error),

    #[error("unique constraint failed")]
    UniqueConstraint,

    #[error("constraint failed")]
    Constraint,

    #[error("integer overflow")]
    IntegerOverflow,
}

/// The transactional context for a repository operation.
#[derive(Debug, Default)]
pub(crate) enum TransactionContext<'a> {
    /// No transaction context.
    #[default]
    None,

    /// Execute the operation on top of a transaction.
    Transaction(Transaction<'a, Sqlite>),
}

impl<'a> TransactionContext<'a> {
    pub(crate) async fn commit(self) -> Result<(), TransactionError> {
        if let TransactionContext::Transaction(tx) = self {
            tx.commit().await?;
        }
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
#[error("transaction: {0}")]
pub(crate) struct TransactionError(#[from] sqlx::Error);

struct Metrics {
    table_size: MaybeMetric<Gauge>,
}

impl Default for Metrics {
    fn default() -> Self {
        let table_size: MaybeMetric<_> =
            Gauge::new("sqlite_table_size_bytes", "Size of each SQLite table in bytes", &["table"]).into();
        Self { table_size }
    }
}

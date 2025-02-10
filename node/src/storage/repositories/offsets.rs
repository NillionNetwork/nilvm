use crate::storage::{
    metrics::ExportMetrics,
    sqlite::{DatabaseError, SqliteDb, TransactionContext},
};
use async_trait::async_trait;
use itertools::Itertools;
use node_api::preprocessing::rust::PreprocessingElement;
use sqlx::prelude::FromRow;
use std::{iter, ops::Range};

#[derive(Clone, Debug, PartialEq, FromRow)]
pub(crate) struct PreprocessingOffsets {
    #[sqlx(try_from = "&'a str")]
    pub(crate) element: PreprocessingElement,

    #[sqlx(try_from = "i64")]
    pub(crate) target: u64,

    #[sqlx(try_from = "i64")]
    pub(crate) latest: u64,

    #[sqlx(try_from = "i64")]
    pub(crate) committed: u64,

    #[sqlx(try_from = "i64")]
    pub(crate) next_batch_id: u64,

    pub(crate) deleted_offset: i64,

    pub(crate) delete_candidate_offset: i64,
}

impl PreprocessingOffsets {
    pub(crate) fn available(&self) -> Range<u64> {
        self.committed..self.latest
    }
}

/// A repository where offsets are stored for each preprocessing element.
#[async_trait]
pub(crate) trait PreprocessingOffsetsRepository: Send + Sync + 'static {
    /// Begins a transaction on this repository.
    async fn begin_transaction(&self) -> Result<TransactionContext, DatabaseError>;

    /// Find all preprocessing elements offsets.
    async fn find(
        &self,
        elements: &[PreprocessingElement],
        context: &mut TransactionContext,
    ) -> Result<Vec<PreprocessingOffsets>, DatabaseError>;

    /// Find a single entry.
    async fn find_one(
        &self,
        element: PreprocessingElement,
        context: &mut TransactionContext,
    ) -> Result<PreprocessingOffsets, DatabaseError>;

    /// Registers a preprocessing element.
    async fn register_element(&self, element: PreprocessingElement) -> Result<(), DatabaseError>;

    /// Update the next batch id for an element.
    async fn update_next_batch_id(
        &self,
        element: PreprocessingElement,
        value: u64,
        context: &mut TransactionContext,
    ) -> Result<(), DatabaseError>;

    /// Update the latest offset for a preprocessing element.
    async fn update_latest(
        &self,
        element: PreprocessingElement,
        value: u64,
        context: &mut TransactionContext,
    ) -> Result<(), DatabaseError>;

    /// Update the target offset for a preprocessing element.
    async fn update_target(
        &self,
        element: PreprocessingElement,
        value: u64,
        context: &mut TransactionContext,
    ) -> Result<(), DatabaseError>;

    /// Update the committed offset for a preprocessing element.
    async fn update_committed(
        &self,
        element: PreprocessingElement,
        value: u64,
        context: &mut TransactionContext,
    ) -> Result<(), DatabaseError>;

    /// Update the delete candidate offset for a preprocessing element.
    async fn update_delete_candidate(
        &self,
        element: PreprocessingElement,
        value: i64,
        context: &mut TransactionContext,
    ) -> Result<(), DatabaseError>;

    /// Update the deleted offset for a preprocessing element.
    async fn update_deleted(
        &self,
        element: PreprocessingElement,
        value: i64,
        context: &mut TransactionContext,
    ) -> Result<(), DatabaseError>;
}

pub(crate) struct SqlitePreprocessingOffsetsRepository(SqliteDb);

impl SqlitePreprocessingOffsetsRepository {
    pub(crate) fn new(db: SqliteDb) -> Self {
        Self(db)
    }

    async fn update_row<T>(
        &self,
        element: PreprocessingElement,
        value: T,
        field_name: &str,
        context: &mut TransactionContext<'_>,
    ) -> Result<(), DatabaseError>
    where
        T: TryInto<i64>,
    {
        let sql_query = format!("UPDATE preprocessing_offsets SET {field_name} = ? WHERE element = ?");
        let value: i64 = value.try_into().map_err(|_| DatabaseError::IntegerOverflow)?;
        let query = sqlx::query(&sql_query).bind(value).bind(element.to_string());
        self.0.execute(query, context).await?;
        Ok(())
    }
}

#[async_trait]
impl PreprocessingOffsetsRepository for SqlitePreprocessingOffsetsRepository {
    async fn begin_transaction(&self) -> Result<TransactionContext, DatabaseError> {
        let tx = self.0.begin_tx().await?;
        Ok(TransactionContext::Transaction(tx))
    }

    async fn find(
        &self,
        elements: &[PreprocessingElement],
        context: &mut TransactionContext,
    ) -> Result<Vec<PreprocessingOffsets>, DatabaseError> {
        if elements.is_empty() {
            return Ok(Vec::new());
        }
        let placeholders = iter::repeat("?").take(elements.len()).join(",");
        let sql_query = format!("SELECT * FROM preprocessing_offsets WHERE element in ({placeholders})");
        let mut query = sqlx::query_as(&sql_query);
        for element in elements {
            query = query.bind(element.to_string());
        }
        let results = self.0.fetch_all(query, context).await?;
        Ok(results)
    }

    async fn find_one(
        &self,
        element: PreprocessingElement,
        context: &mut TransactionContext,
    ) -> Result<PreprocessingOffsets, DatabaseError> {
        let entries = self.find(&[element], context).await?;
        match entries.into_iter().next() {
            Some(entry) => Ok(entry),
            None => Err(DatabaseError::NotFound),
        }
    }

    async fn register_element(&self, element: PreprocessingElement) -> Result<(), DatabaseError> {
        let query = sqlx::query(
            "INSERT OR IGNORE INTO preprocessing_offsets(element, target, latest, committed, next_batch_id) VALUES (?, 0, 0, 0, 0)",
        )
        .bind(element.to_string());
        self.0.execute(query, &mut Default::default()).await?;
        Ok(())
    }

    async fn update_next_batch_id(
        &self,
        element: PreprocessingElement,
        value: u64,
        context: &mut TransactionContext,
    ) -> Result<(), DatabaseError> {
        self.update_row(element, value, "next_batch_id", context).await
    }

    async fn update_latest(
        &self,
        element: PreprocessingElement,
        value: u64,
        context: &mut TransactionContext,
    ) -> Result<(), DatabaseError> {
        self.update_row(element, value, "latest", context).await
    }

    async fn update_committed(
        &self,
        element: PreprocessingElement,
        value: u64,
        context: &mut TransactionContext,
    ) -> Result<(), DatabaseError> {
        self.update_row(element, value, "committed", context).await
    }

    async fn update_target(
        &self,
        element: PreprocessingElement,
        value: u64,
        context: &mut TransactionContext,
    ) -> Result<(), DatabaseError> {
        self.update_row(element, value, "target", context).await
    }

    async fn update_delete_candidate(
        &self,
        element: PreprocessingElement,
        value: i64,
        context: &mut TransactionContext,
    ) -> Result<(), DatabaseError> {
        self.update_row(element, value, "delete_candidate_offset", context).await
    }

    async fn update_deleted(
        &self,
        element: PreprocessingElement,
        value: i64,
        context: &mut TransactionContext,
    ) -> Result<(), DatabaseError> {
        self.update_row(element, value, "deleted_offset", context).await
    }
}

#[async_trait]
impl ExportMetrics for SqlitePreprocessingOffsetsRepository {
    async fn export_metrics(&self) -> anyhow::Result<()> {
        self.0.export_table_metrics("preprocessing_offsets").await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn make_repo() -> SqlitePreprocessingOffsetsRepository {
        let handle = SqliteDb::new("sqlite::memory:").await.expect("repo creation failed");
        SqlitePreprocessingOffsetsRepository::new(handle)
    }

    #[tokio::test]
    async fn register() {
        let repo = make_repo().await;
        repo.register_element(PreprocessingElement::Compare).await.expect("registration failed");
    }

    #[tokio::test]
    async fn update_columns() {
        let repo = make_repo().await;
        repo.register_element(PreprocessingElement::Compare).await.expect("registration failed");
        repo.update_committed(PreprocessingElement::Compare, 1, &mut Default::default())
            .await
            .expect("update committed failed");
        repo.update_latest(PreprocessingElement::Compare, 2, &mut Default::default())
            .await
            .expect("update latest failed");
        repo.update_target(PreprocessingElement::Compare, 3, &mut Default::default())
            .await
            .expect("update target failed");
        repo.update_delete_candidate(PreprocessingElement::Compare, 4, &mut Default::default())
            .await
            .expect("update delete_candidate failed");
        repo.update_deleted(PreprocessingElement::Compare, 5, &mut Default::default())
            .await
            .expect("update deleted failed");
        repo.update_next_batch_id(PreprocessingElement::Compare, 6, &mut Default::default())
            .await
            .expect("update next_batch_id failed");

        let entries =
            repo.find(&[PreprocessingElement::Compare], &mut Default::default()).await.expect("lookup failed");
        assert_eq!(entries.len(), 1);
        let entry = &entries[0];
        let expected = PreprocessingOffsets {
            element: PreprocessingElement::Compare,
            committed: 1,
            latest: 2,
            target: 3,
            delete_candidate_offset: 4,
            deleted_offset: 5,
            next_batch_id: 6,
        };
        assert_eq!(entry, &expected);
    }
}

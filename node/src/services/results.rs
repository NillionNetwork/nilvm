//! Computation results storage service.

use super::blob::BlobService;
use crate::storage::{
    models::result::ComputeResult,
    repositories::{
        blob::BlobRepositoryError,
        blob_expirations::{BlobExpirationsRepository, ExpireableBlob, ExpireableBlobKind},
    },
    sqlite::DatabaseError,
};
use async_trait::async_trait;
use chrono::Utc;
use node_api::{auth::rust::UserId, values::rust::NamedValue};
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::sync::{broadcast, Mutex};
use tracing::{error, info, warn};
use uuid::Uuid;

// 1 day
const COMPUTE_RESULT_EXPIRATION: Duration = Duration::from_secs(60 * 60 * 24);

#[derive(Debug, PartialEq)]
pub(crate) enum OutputPartyResult {
    Success { values: Vec<NamedValue> },
    Failure { error: String },
}

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub(crate) trait ResultsService: Send + Sync + 'static {
    /// Register a compute execution.
    ///
    /// This is used to keep state in memory about a computation and allows looking it up via
    /// `wait_execution` which waits asynchronously for the computation to end.
    async fn register_execution(&self, compute_id: Uuid);

    /// Wait for an active compute execution to end.
    ///
    /// If the execution is active, this will return `Ok(Some(...))` or `Err(_)` just like
    /// `fetch_output_party_result` does. If the execution is not active it will return `Ok(None)`,
    /// indicating nothing went wrong but we couldn't find the execution.
    ///
    /// This is used as an optimization over going straight to the repository to perform a lookup.
    async fn wait_execution(
        &self,
        compute_id: Uuid,
        user_id: &UserId,
    ) -> Result<Option<OutputPartyResult>, FetchResultError>;

    /// Store a result.
    ///
    /// This will wake up any tasks parked waiting on `wait_execution` for the given user id.
    async fn store_result(&self, compute_id: Uuid, results: ComputeResult) -> Result<(), BlobRepositoryError>;

    /// Fetch the results of a computation for the given user id.
    async fn fetch_output_party_result(
        &self,
        compute_id: Uuid,
        user_id: &UserId,
    ) -> Result<OutputPartyResult, FetchResultError>;

    /// Delete expired compute results.
    async fn delete_expired(&self) -> Result<u64, DeleteExpiredError>;
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum FetchResultError {
    #[error("user is not authorized to fetch result")]
    Unauthorized,

    #[error(transparent)]
    Blob(#[from] BlobRepositoryError),
}

struct ActiveExecution {
    sender: broadcast::Sender<ComputeResult>,
    receiver: Option<broadcast::Receiver<ComputeResult>>,
}

pub(crate) struct DefaultResultsService {
    blob_service: Box<dyn BlobService<ComputeResult>>,
    expiry_repo: Arc<dyn BlobExpirationsRepository>,
    active_executions: Mutex<HashMap<Uuid, ActiveExecution>>,
}

impl DefaultResultsService {
    pub(crate) fn new(
        blob_service: Box<dyn BlobService<ComputeResult>>,
        expiry_repo: Arc<dyn BlobExpirationsRepository>,
    ) -> Self {
        Self { blob_service, expiry_repo, active_executions: Default::default() }
    }

    fn build_party_result(result: ComputeResult, user_id: &UserId) -> Result<OutputPartyResult, FetchResultError> {
        match result {
            ComputeResult::Success { mut values } => {
                if let Some(values) = values.remove(user_id) {
                    Ok(OutputPartyResult::Success { values })
                } else {
                    Err(FetchResultError::Unauthorized)
                }
            }
            ComputeResult::Failure { error } => Ok(OutputPartyResult::Failure { error }),
        }
    }
}

#[async_trait]
impl ResultsService for DefaultResultsService {
    async fn register_execution(&self, compute_id: Uuid) {
        let mut active = self.active_executions.lock().await;
        if active.contains_key(&compute_id) {
            warn!("Not inserting active execution {compute_id} as it already existgs");
            return;
        }
        let (sender, receiver) = broadcast::channel(1);
        active.insert(compute_id, ActiveExecution { sender, receiver: Some(receiver) });
    }

    async fn store_result(&self, compute_id: Uuid, results: ComputeResult) -> Result<(), BlobRepositoryError> {
        #[allow(clippy::arithmetic_side_effects)]
        let expiration = Utc::now() + COMPUTE_RESULT_EXPIRATION;
        let key = compute_id.to_string();
        self.blob_service.upsert(&key, results.clone()).await?;
        self.expiry_repo
            .upsert(&ExpireableBlob::new_compute_result(compute_id, expiration))
            .await
            .map_err(|e| BlobRepositoryError::Internal(format!("failed to store expireable result: {e}")))?;

        let mut active = self.active_executions.lock().await;
        match active.get(&compute_id) {
            Some(channels) => {
                let sender = channels.sender.clone();
                active.remove(&compute_id);
                drop(active);
                sender.send(results).ok();
            }
            None => {
                warn!("No active execution found for {compute_id}");
            }
        };
        Ok(())
    }

    async fn wait_execution(
        &self,
        compute_id: Uuid,
        user_id: &UserId,
    ) -> Result<Option<OutputPartyResult>, FetchResultError> {
        let mut active = self.active_executions.lock().await;
        if let Some(channels) = active.get_mut(&compute_id) {
            // take the initial receiver if it exists or create a new one otherwise
            let mut receiver = match channels.receiver.take() {
                Some(r) => r,
                None => channels.sender.subscribe(),
            };
            drop(active);
            if let Ok(result) = receiver.recv().await {
                return Self::build_party_result(result, user_id).map(Some);
            } else {
                warn!("No results received");
            }
        }
        Ok(None)
    }

    async fn fetch_output_party_result(
        &self,
        compute_id: Uuid,
        user_id: &UserId,
    ) -> Result<OutputPartyResult, FetchResultError> {
        let key = compute_id.to_string();
        let result = self.blob_service.find_one(&key).await?;
        Self::build_party_result(result, user_id)
    }

    /// Delete expired user values
    async fn delete_expired(&self) -> Result<u64, DeleteExpiredError> {
        let now = Utc::now();
        info!("Deleting compute results that expire before {now}");
        let expired_entries = self
            .expiry_repo
            .find_expired(ExpireableBlobKind::ComputeResult, now)
            .await
            .map_err(|e| DeleteExpiredError(e.to_string()))?;
        for entry in expired_entries {
            info!("Deleting compute result with id: {}", entry.key);
            self.blob_service.delete(&entry.key.to_string()).await.map_err(|e| DeleteExpiredError(e.to_string()))?;
        }

        // Delete expired entries from sqlite DB
        let count = self.expiry_repo.delete_expired(ExpireableBlobKind::ComputeResult, now).await?;
        Ok(count)
    }
}

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub(crate) struct DeleteExpiredError(String);

impl From<DatabaseError> for DeleteExpiredError {
    fn from(e: DatabaseError) -> Self {
        Self(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        services::blob::DefaultBlobService,
        storage::{repositories::blob_expirations::SqliteBlobExpirationsRepository, sqlite::SqliteDb},
    };
    use futures::executor::block_on;
    use std::{sync::Arc, time::Duration};
    use tokio::time::sleep;

    fn make_service() -> Arc<DefaultResultsService> {
        let blob = DefaultBlobService::new_in_memory();
        let db = block_on(async { SqliteDb::new("sqlite::memory:").await.expect("repo creation failed") });
        let expirations_repo = Arc::new(SqliteBlobExpirationsRepository::new(db));
        Arc::new(DefaultResultsService::new(Box::new(blob), expirations_repo))
    }

    #[tokio::test]
    async fn waiting_non_existent_execution() {
        let service = make_service();
        let result = service.wait_execution(Uuid::new_v4(), &UserId::from_bytes("foo")).await.expect("lookup failed");
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn waiting_execution() {
        let service = make_service();
        let compute_id = Uuid::new_v4();
        service.register_execution(compute_id).await;

        let fut = {
            let service = service.clone();
            tokio::spawn(async move {
                service.wait_execution(compute_id, &UserId::from_bytes("foo")).await.expect("lookup failed")
            })
        };
        sleep(Duration::from_millis(200)).await;

        service
            .store_result(compute_id, ComputeResult::Failure { error: "foo".to_string() })
            .await
            .expect("storing failed");

        let result = fut.await.expect("waiting failed");
        assert_eq!(result, Some(OutputPartyResult::Failure { error: "foo".to_string() }));
    }
}

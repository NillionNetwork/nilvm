use crate::storage::{
    repositories::nonces::{ExpireableNonce, UsedNoncesRepository},
    sqlite::DatabaseError,
};
use async_trait::async_trait;
use chrono::Utc;
use std::sync::Arc;
use tracing::info;

/// A service that generates nonces.
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub(crate) trait NonceService: Send + Sync + 'static {
    async fn remove_expired_nonces(&self) -> anyhow::Result<u64>;
    async fn record_nonce(&self, nonce: &ExpireableNonce) -> Result<(), RecordNonceError>;
}

pub(crate) struct DefaultNonceService {
    used_nonces_repo: Arc<dyn UsedNoncesRepository>,
}

impl DefaultNonceService {
    pub(crate) fn new(used_nonces_repo: Arc<dyn UsedNoncesRepository>) -> Self {
        Self { used_nonces_repo }
    }
}

#[async_trait]
impl NonceService for DefaultNonceService {
    async fn remove_expired_nonces(&self) -> anyhow::Result<u64> {
        let now = Utc::now();
        info!("Deleting nonces that expire before {now}");
        let count = self.used_nonces_repo.delete_expired(now, &mut Default::default()).await?;
        Ok(count)
    }

    async fn record_nonce(&self, nonce: &ExpireableNonce) -> Result<(), RecordNonceError> {
        match self.used_nonces_repo.insert(nonce, &mut Default::default()).await {
            Ok(_) => Ok(()),
            Err(DatabaseError::UniqueConstraint) => Err(RecordNonceError::ReusedNonce),
            Err(e) => Err(RecordNonceError::Internal(e.to_string())),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum RecordNonceError {
    #[error("nonce already used")]
    ReusedNonce,

    #[error("{0}")]
    Internal(String),
}

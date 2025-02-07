use crate::storage::repositories::blob::{BinarySerde, BlobRepository, BlobRepositoryError};
use async_trait::async_trait;
use metrics::prelude::*;
use once_cell::sync::Lazy;
use std::{future::Future, time::Duration};
use tracing::info;

static METRICS: Lazy<Metrics> = Lazy::new(Metrics::default);

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub(crate) trait BlobService<T: Send + Sync + 'static>: Send + Sync + 'static {
    async fn find_one(&self, key: &str) -> Result<T, BlobRepositoryError>;
    async fn create(&self, key: &str, value: T) -> Result<(), BlobRepositoryError>;
    async fn upsert(&self, key: &str, value: T) -> Result<(), BlobRepositoryError>;
    async fn delete(&self, key: &str) -> Result<(), BlobRepositoryError>;
}

pub(crate) struct DefaultBlobService<T> {
    repository: Box<dyn BlobRepository<T>>,
    prefix: String,
}

impl<T: BinarySerde + Clone> DefaultBlobService<T> {
    pub(crate) fn new<S: Into<String>>(prefix: S, repository: Box<dyn BlobRepository<T>>) -> Self {
        let mut prefix = prefix.into();
        prefix.push('/');
        Self { repository, prefix }
    }

    #[cfg(test)]
    pub(crate) fn new_in_memory() -> Self {
        Self::new("", Box::new(crate::storage::repositories::blob::MemoryBlobRepository::default()))
    }

    fn prefix_key(&self, key: &str) -> String {
        format!("{}{}", self.prefix, key)
    }

    fn sanitize_result<U>(operation: &str, result: Result<U, BlobRepositoryError>) -> Result<U, BlobRepositoryError> {
        match result {
            Ok(value) => Ok(value),
            Err(e @ BlobRepositoryError::NotFound) => {
                METRICS.inc_operation_errors(operation, "not_found");
                Err(e)
            }
            Err(e @ BlobRepositoryError::AlreadyExists) => {
                METRICS.inc_operation_errors(operation, "already_exists");
                Err(e)
            }
            Err(e) => {
                METRICS.inc_operation_errors(operation, "internal");
                Err(e)
            }
        }
    }

    async fn invoke<F, U>(&self, name: &str, key: &str, fut: F) -> Result<U, BlobRepositoryError>
    where
        F: Future<Output = Result<U, BlobRepositoryError>>,
    {
        let _timer = METRICS.operation_timer(&self.prefix, name);
        info!("Performing {name} for key {key}");
        Self::sanitize_result(name, fut.await)
    }
}

#[async_trait]
impl<T> BlobService<T> for DefaultBlobService<T>
where
    T: BinarySerde + Clone,
{
    async fn find_one(&self, key: &str) -> Result<T, BlobRepositoryError> {
        let key = self.prefix_key(key);
        self.invoke("find_one", &key, self.repository.read(&key)).await
    }

    async fn create(&self, key: &str, value: T) -> Result<(), BlobRepositoryError> {
        let key = self.prefix_key(key);
        self.invoke("create", &key, self.repository.create(&key, value)).await
    }

    async fn upsert(&self, key: &str, value: T) -> Result<(), BlobRepositoryError> {
        let key = self.prefix_key(key);
        self.invoke("upsert", &key, self.repository.upsert(&key, value)).await
    }

    async fn delete(&self, key: &str) -> Result<(), BlobRepositoryError> {
        let key = self.prefix_key(key);
        self.invoke("delete", &key, self.repository.delete(&key)).await
    }
}

struct Metrics {
    operation_duration: MaybeMetric<Histogram<Duration>>,
    operation_errors: MaybeMetric<Counter>,
}

impl Default for Metrics {
    fn default() -> Self {
        let operation_duration = Histogram::new(
            "blob_operation_duration_seconds",
            "Duration of blob operations in seconds",
            &["prefix", "operation"],
            TimingBuckets::sub_second(),
        )
        .into();
        let operation_errors = Counter::new(
            "blob_operation_errors_total",
            "Number of errors encountered in blob operations",
            &["operation", "reason"],
        )
        .into();
        Self { operation_duration, operation_errors }
    }
}

impl Metrics {
    fn operation_timer(&self, prefix: &str, operation: &str) -> ScopedTimer<impl SingleHistogramMetric<Duration>> {
        self.operation_duration.with_labels([("prefix", prefix), ("operation", operation)]).into_timer()
    }

    fn inc_operation_errors(&self, operation: &str, reason: &str) {
        self.operation_errors.with_labels([("operation", operation), ("reason", reason)]).inc();
    }
}

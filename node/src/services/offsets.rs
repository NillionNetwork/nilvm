//! Offset tracking.

use super::scheduling::PreprocessingSchedulingService;
use crate::storage::{
    repositories::offsets::{PreprocessingOffsets, PreprocessingOffsetsRepository},
    sqlite::{DatabaseError, TransactionContext, TransactionError},
};
use async_trait::async_trait;
use metrics::prelude::*;
use node_api::preprocessing::rust::PreprocessingElement;
use once_cell::sync::Lazy;
use std::{
    collections::BTreeMap,
    ops::Range,
    sync::{Arc, Mutex},
    time::Duration,
};
use strum::IntoEnumIterator;
use tracing::{error, info};

static METRICS: Lazy<Metrics> = Lazy::new(Metrics::default);

const DELETED_LABEL: &str = "deleted";
const DELETE_CANDIDATE_LABEL: &str = "delete_candidate";
const LATEST_LABEL: &str = "latest";
const COMMITTED_LABEL: &str = "committed";
const TARGET_LABEL: &str = "target";

/// A service to interact and keep track of the offsets for every preprocessing elements.
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub(crate) trait ElementOffsetsService: Send + Sync + 'static {
    /// Update the target offset for an element.
    async fn set_target_offset(&self, element: PreprocessingElement, offset: u64) -> anyhow::Result<()>;

    /// Request preprocessing offsets for a set of preprocessing elements.
    ///
    /// This will validate that there are enough offsets available.
    async fn request_preprocessing_offsets<'a>(
        &'a self,
        amounts: Vec<(PreprocessingElement, u64)>,
        ctx: &mut TransactionContext<'a>,
    ) -> Result<BTreeMap<PreprocessingElement, Range<u64>>, RequestOffsetsError>;

    /// Gets the offsets for a preprocessing element.
    async fn offsets(&self, element: &PreprocessingElement) -> anyhow::Result<PreprocessingOffsets>;

    /// Get the offsets available for every element.
    async fn all_offsets(&self) -> anyhow::Result<BTreeMap<PreprocessingElement, PreprocessingOffsets>>;

    /// Advance the current offset for an (cluster, element).
    async fn advance_latest_offset(
        &self,
        element: PreprocessingElement,
        offset: u64,
        completed_batch_id: u64,
    ) -> anyhow::Result<()>;

    /// Set the deleted offset for a preprocessing element.
    async fn set_deleted_offset(&self, element: PreprocessingElement, offset: u64) -> anyhow::Result<()>;

    /// Set the deleted offset for a preprocessing element.
    async fn set_delete_candidate_offset(&self, element: PreprocessingElement, offset: u64) -> anyhow::Result<()>;

    /// Emit metrics for all preprocessing elements.
    async fn emit_metrics(&self) -> anyhow::Result<()>;
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum RequestOffsetsError {
    #[error("repository: {0}")]
    Repository(#[from] DatabaseError),

    #[error("not enough elements for {0:?}")]
    NotEnoughElements(PreprocessingElement),

    #[error("transaction commit: {0}")]
    Transaction(#[from] TransactionError),

    #[error("internal: {0}")]
    Internal(String),
}

pub(crate) struct DefaultElementOffsetsService {
    offsets_repository: Arc<dyn PreprocessingOffsetsRepository>,
    preprocessing_scheduling_service: Mutex<Arc<dyn PreprocessingSchedulingService>>,
}

impl DefaultElementOffsetsService {
    pub(crate) fn new(
        offsets_repository: Arc<dyn PreprocessingOffsetsRepository>,
        preprocessing_scheduling_service: Arc<dyn PreprocessingSchedulingService>,
    ) -> Self {
        Self { offsets_repository, preprocessing_scheduling_service: preprocessing_scheduling_service.into() }
    }

    pub(crate) fn set_preprocessing_scheduliing_service(&self, service: Arc<dyn PreprocessingSchedulingService>) {
        if let Ok(mut s) = self.preprocessing_scheduling_service.lock() {
            *s = service;
        }
    }
}

#[async_trait]
impl ElementOffsetsService for DefaultElementOffsetsService {
    async fn set_target_offset(&self, element: PreprocessingElement, offset: u64) -> anyhow::Result<()> {
        let _timer = METRICS.operation_timer("set_target_offset");
        self.offsets_repository.update_target(element, offset, &mut Default::default()).await?;
        METRICS.set_offset_value(&element, TARGET_LABEL, offset);
        Ok(())
    }

    async fn request_preprocessing_offsets<'a>(
        &'a self,
        mut amounts: Vec<(PreprocessingElement, u64)>,
        ctx: &mut TransactionContext<'a>,
    ) -> Result<BTreeMap<PreprocessingElement, Range<u64>>, RequestOffsetsError> {
        let _timer = METRICS.operation_timer("request_offsets");
        amounts.sort_by_key(|e| e.0);

        let elements: Vec<_> = amounts.iter().map(|(element, _)| *element).collect();
        let mut offsets = self.offsets_repository.find(&elements, ctx).await?;
        if offsets.len() != amounts.len() {
            return Err(RequestOffsetsError::Internal(format!(
                "expected {} elements, got {}",
                amounts.len(),
                offsets.len()
            )));
        }
        offsets.sort_by_key(|e| e.element);

        let mut ranges = BTreeMap::new();
        for (element, (_, amount)) in offsets.into_iter().zip(amounts) {
            let PreprocessingOffsets { element, committed, latest, .. } = element;
            let new_commit_offset = committed.wrapping_add(amount);
            if new_commit_offset > latest {
                return Err(RequestOffsetsError::NotEnoughElements(element));
            }
            info!("Updating committed offset for {element} to {new_commit_offset}");
            self.offsets_repository.update_committed(element, new_commit_offset, ctx).await?;
            ranges.insert(element, committed..new_commit_offset);
            METRICS.set_offset_value(&element, COMMITTED_LABEL, new_commit_offset);
        }
        // Note that we are notifying even if the transaction could fail but this is okay because
        // the scheduling service validates things on its own
        if let Ok(scheduling_service) = self.preprocessing_scheduling_service.lock() {
            scheduling_service.notify_used_elements(&elements);
        }
        Ok(ranges)
    }

    async fn offsets(&self, element: &PreprocessingElement) -> anyhow::Result<PreprocessingOffsets> {
        let _timer = METRICS.operation_timer("offsets");
        let element = self.offsets_repository.find_one(*element, &mut Default::default()).await?;
        Ok(element)
    }

    async fn all_offsets(&self) -> anyhow::Result<BTreeMap<PreprocessingElement, PreprocessingOffsets>> {
        let _timer = METRICS.operation_timer("element_offsets_available");
        let all: Vec<_> = PreprocessingElement::iter().collect();
        let entries = self.offsets_repository.find(&all, &mut Default::default()).await?;
        let output = entries.into_iter().map(|entry| (entry.element, entry)).collect();
        Ok(output)
    }

    async fn advance_latest_offset(
        &self,
        element: PreprocessingElement,
        offset: u64,
        completed_batch_id: u64,
    ) -> anyhow::Result<()> {
        let _timer = METRICS.operation_timer("advance_latest_offset");
        let mut tx = self.offsets_repository.begin_transaction().await?;
        let element = self.offsets_repository.find_one(element, &mut tx).await?;
        let new_latest = element.latest.wrapping_add(offset);
        info!("Setting latest offset for {} to {new_latest} (batch id {completed_batch_id})", element.element);
        self.offsets_repository
            .update_next_batch_id(element.element, completed_batch_id.wrapping_add(1), &mut tx)
            .await?;
        self.offsets_repository.update_latest(element.element, new_latest, &mut tx).await?;
        tx.commit().await?;
        METRICS.set_offset_value(&element.element, LATEST_LABEL, new_latest);
        Ok(())
    }

    async fn set_deleted_offset(&self, element: PreprocessingElement, offset: u64) -> anyhow::Result<()> {
        let _timer = METRICS.operation_timer("set_deleted_offset");
        self.offsets_repository.update_deleted(element, offset as i64, &mut Default::default()).await?;
        METRICS.set_offset_value(&element, DELETED_LABEL, offset);
        Ok(())
    }

    async fn set_delete_candidate_offset(&self, element: PreprocessingElement, offset: u64) -> anyhow::Result<()> {
        let _timer = METRICS.operation_timer("set_delete_candidate_offset");
        self.offsets_repository.update_delete_candidate(element, offset as i64, &mut Default::default()).await?;
        METRICS.set_offset_value(&element, DELETE_CANDIDATE_LABEL, offset);
        Ok(())
    }

    async fn emit_metrics(&self) -> anyhow::Result<()> {
        for (element, offsets) in self.all_offsets().await? {
            METRICS.set_offset_value(&element, DELETED_LABEL, offsets.deleted_offset.max(0) as u64);
            METRICS.set_offset_value(&element, DELETE_CANDIDATE_LABEL, offsets.delete_candidate_offset.max(0) as u64);
            METRICS.set_offset_value(&element, COMMITTED_LABEL, offsets.committed);
            METRICS.set_offset_value(&element, LATEST_LABEL, offsets.latest);
            METRICS.set_offset_value(&element, TARGET_LABEL, offsets.target);
        }
        Ok(())
    }
}

struct Metrics {
    operation_duration: MaybeMetric<Histogram<Duration>>,
    offsets: MaybeMetric<Gauge>,
}

impl Default for Metrics {
    fn default() -> Self {
        let operation_duration = Histogram::new(
            "preprocessing_offsets_operation_duration_seconds",
            "Duration of preprocessing offset operations in seconds",
            &["operation"],
            TimingBuckets::sub_second(),
        )
        .into();
        let offsets =
            Gauge::new("preprocessing_offsets", "Preprocessing offsets by offset type", &["element", "offset"]).into();
        Self { operation_duration, offsets }
    }
}

impl Metrics {
    fn operation_timer(&self, operation: &str) -> ScopedTimer<impl SingleHistogramMetric<Duration>> {
        self.operation_duration.with_labels([("operation", operation)]).into_timer()
    }

    fn set_offset_value(&self, element: &PreprocessingElement, offset: &str, value: u64) {
        let element = element.to_string().to_uppercase();
        match i64::try_from(value) {
            Ok(value) => self.offsets.with_labels([("element", &element), ("offset", offset)]).set(value),
            Err(_) => error!("offset {offset} is too large to fit in i64"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        services::scheduling::MockPreprocessingSchedulingService,
        storage::{repositories::offsets::SqlitePreprocessingOffsetsRepository, sqlite::SqliteDb},
    };
    use mockall::predicate::eq;
    use tracing_test::traced_test;

    struct ServiceBuilder {
        preprocessing_scheduling_service: MockPreprocessingSchedulingService,
    }

    impl ServiceBuilder {
        async fn build(self) -> DefaultElementOffsetsService {
            let db = SqliteDb::in_memory().await.expect("creating handle");
            let offsets_repo = Arc::new(SqlitePreprocessingOffsetsRepository::new(db));
            for element in PreprocessingElement::iter() {
                offsets_repo.register_element(element).await.expect("registering element");
            }
            DefaultElementOffsetsService::new(offsets_repo, Arc::new(self.preprocessing_scheduling_service))
        }
    }

    impl Default for ServiceBuilder {
        fn default() -> Self {
            Self { preprocessing_scheduling_service: Default::default() }
        }
    }

    #[tokio::test]
    #[traced_test]
    async fn request_offsets() {
        let mut builder = ServiceBuilder::default();
        builder
            .preprocessing_scheduling_service
            .expect_notify_used_elements()
            .with(eq(&[PreprocessingElement::Compare] as &[_]))
            .times(2)
            .returning(|_| ());
        let service = builder.build().await;
        service.advance_latest_offset(PreprocessingElement::Compare, 10, 0).await.expect("setting latest");

        // advance it twice to ensure we're moving it and not simply overwriting it
        let offsets = service
            .request_preprocessing_offsets(vec![(PreprocessingElement::Compare, 5)], &mut Default::default())
            .await
            .expect("request failed");
        assert_eq!(offsets.len(), 1);
        assert_eq!(offsets.get(&PreprocessingElement::Compare), Some(&(0..5)));

        let offsets = service
            .request_preprocessing_offsets(vec![(PreprocessingElement::Compare, 1)], &mut Default::default())
            .await
            .expect("request failed");
        assert_eq!(offsets.len(), 1);
        assert_eq!(offsets.get(&PreprocessingElement::Compare), Some(&(5..6)));

        let elements = service.all_offsets().await.expect("fetching elements");
        assert_eq!(elements.get(&PreprocessingElement::Compare).map(PreprocessingOffsets::available), Some(6..10));
    }

    #[tokio::test]
    #[traced_test]
    async fn request_too_many_offsets() {
        let service = ServiceBuilder::default().build().await;
        service.advance_latest_offset(PreprocessingElement::Compare, 10, 0).await.expect("setting latest");

        service
            .request_preprocessing_offsets(vec![(PreprocessingElement::Compare, 11)], &mut Default::default())
            .await
            .expect_err("request succeeded");
    }

    #[tokio::test]
    #[traced_test]
    async fn store_candidate() {
        let service = ServiceBuilder::default().build().await;
        service.set_delete_candidate_offset(PreprocessingElement::Compare, 10).await.expect("failed to set candidates");

        assert_eq!(
            service
                .offsets_repository
                .find_one(PreprocessingElement::Compare, &mut Default::default())
                .await
                .unwrap()
                .delete_candidate_offset,
            10
        );
    }
}

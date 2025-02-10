use crate::storage::repositories::blob::BinarySerde;

use super::{blob::BlobService, runtime_elements::PreprocessingElementOffsets};
use anyhow::{anyhow, bail};
use async_trait::async_trait;
use math_lib::modular::EncodedModularNumber;
use metrics::prelude::*;
use once_cell::sync::Lazy;
use protocols::{
    conditionals::{
        equality::offline::EncodedPrepPrivateOutputEqualityShares,
        equality_public_output::offline::EncodedPrepPublicOutputEqualityShares,
        less_than::offline::EncodedPrepCompareShares,
    },
    division::{
        division_secret_divisor::offline::EncodedPrepDivisionIntegerSecretShares,
        modulo2m_public_divisor::offline::EncodedPrepModulo2mShares,
        modulo_public_divisor::offline::EncodedPrepModuloShares,
        truncation_probabilistic::offline::EncodedPrepTruncPrShares,
    },
    random::random_bit::EncodedBitShare,
};
use serde::{Deserialize, Serialize};
use std::{sync::Arc, time::Duration};
use tokio::sync::Mutex;
use tracing::{info, warn};

static METRICS: Lazy<PreprocessingElementMetrics> = Lazy::new(PreprocessingElementMetrics::default);

pub(crate) type PrepCompareSharesService = Arc<dyn PreprocessingBlobService<EncodedPrepCompareShares>>;
pub(crate) type PrepDivisionIntegerSecretSharesService =
    Arc<dyn PreprocessingBlobService<EncodedPrepDivisionIntegerSecretShares>>;
pub(crate) type PrepEqualsIntegerSecretSharesService =
    Arc<dyn PreprocessingBlobService<EncodedPrepPrivateOutputEqualityShares>>;
pub(crate) type PrepModuloSharesService = Arc<dyn PreprocessingBlobService<EncodedPrepModuloShares>>;
pub(crate) type PrepPublicOutputEqualitySharesService =
    Arc<dyn PreprocessingBlobService<EncodedPrepPublicOutputEqualityShares>>;
pub(crate) type PrepTruncSharesService = Arc<dyn PreprocessingBlobService<EncodedPrepModulo2mShares>>;
pub(crate) type PrepTruncPrSharesService = Arc<dyn PreprocessingBlobService<EncodedPrepTruncPrShares>>;
pub(crate) type PrepRandomIntegerSharesService = Arc<dyn PreprocessingBlobService<EncodedModularNumber>>;
pub(crate) type PrepRandomBooleanSharesService = Arc<dyn PreprocessingBlobService<EncodedBitShare>>;

/// The shares of a batch of preprocessing elements.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PreprocessingElementShares<T> {
    /// The shares of each element.
    pub shares: Vec<T>,
}

#[async_trait]
pub(crate) trait PreprocessingBlobService<T>: Send + Sync + 'static {
    async fn find_by_offsets(&self, offsets: &PreprocessingElementOffsets) -> anyhow::Result<Vec<T>>;
    async fn upsert(&self, batch_id: u32, shares: Vec<T>) -> anyhow::Result<()>;
    async fn delete(&self, id: u32) -> anyhow::Result<()>;
}

pub(crate) struct DefaultPreprocessingBlobService<T> {
    blob_service: Box<dyn BlobService<PreprocessingElementShares<T>>>,
}

impl<T> DefaultPreprocessingBlobService<T> {
    pub(crate) fn new(blob_service: Box<dyn BlobService<PreprocessingElementShares<T>>>) -> Self {
        Self { blob_service }
    }
}

#[async_trait]
impl<T> PreprocessingBlobService<T> for DefaultPreprocessingBlobService<T>
where
    T: BinarySerde,
{
    async fn find_by_offsets(&self, offsets: &PreprocessingElementOffsets) -> anyhow::Result<Vec<T>> {
        if offsets.total == 0 {
            return Ok(vec![]);
        }

        let mut all_elements = Vec::new();
        let mut total_retrieved = 0;

        let _timer = METRICS.find_by_offsets_duration.timer();

        if offsets.first_batch_id > offsets.last_batch_id {
            bail!(
                "cannot retrieve preprocessing elements because first_batch_id {} is greater than last_batch_id {}",
                offsets.first_batch_id,
                offsets.last_batch_id
            );
        }

        for batch_id in offsets.first_batch_id..=offsets.last_batch_id {
            info!("Trying to find batch {batch_id}");
            let batch_elements = self.blob_service.find_one(&batch_id.to_string()).await?;

            // Skip elements in the first batch based on the start_offset
            let elements_to_skip = if batch_id == offsets.first_batch_id { offsets.start_offset as usize } else { 0 };
            if elements_to_skip > batch_elements.shares.len() {
                bail!(
                    "cannot retrieve preprocessing elements because start_offset {} is greater than the number of elements in the batch {}",
                    offsets.start_offset,
                    batch_elements.shares.len()
                );
            }
            let mut elements: Vec<T> = batch_elements.shares.into_iter().skip(elements_to_skip).collect();

            // Limit the number of elements to the 'total'
            let remaining = (offsets.total as usize)
                .checked_sub(total_retrieved)
                .ok_or_else(|| anyhow!("Cannot retrieve preprocessing elements because of remaining underflow"))?;
            if elements.len() > remaining {
                elements.truncate(remaining);
            }

            total_retrieved = total_retrieved
                .checked_add(elements.len())
                .ok_or_else(|| anyhow!("Cannot retrieve preprocessing elements because of total_retrieved overflow"))?;

            all_elements.extend(elements);
        }

        if total_retrieved < offsets.total as usize {
            bail!(
                "cannot retrieve preprocessing elements because requested quantity {} is more than total number of elements available {}",
                offsets.total,
                total_retrieved,
            );
        }

        Ok(all_elements)
    }

    async fn upsert(&self, batch_id: u32, shares: Vec<T>) -> anyhow::Result<()> {
        let shares = PreprocessingElementShares { shares };
        Ok(self.blob_service.upsert(&batch_id.to_string(), shares).await?)
    }

    async fn delete(&self, id: u32) -> anyhow::Result<()> {
        self.blob_service.delete(&id.to_string()).await?;
        Ok(())
    }
}

struct FakeInner<T> {
    share: Option<T>,
    batch_size: usize,
}

pub(crate) struct FakePreprocessingBlobService<T> {
    real_service: Arc<dyn PreprocessingBlobService<T>>,
    inner: Mutex<FakeInner<T>>,
}

impl<T> FakePreprocessingBlobService<T> {
    pub(crate) fn new(real_service: Arc<dyn PreprocessingBlobService<T>>) -> Self {
        Self { real_service, inner: Mutex::new(FakeInner { share: None, batch_size: 0 }) }
    }

    pub(crate) async fn set_batch_size(&self, size: usize) {
        self.inner.lock().await.batch_size = size;
    }
}

#[async_trait]
impl<T> PreprocessingBlobService<T> for FakePreprocessingBlobService<T>
where
    T: Clone + Send + Sync + 'static,
{
    async fn find_by_offsets(&self, offsets: &PreprocessingElementOffsets) -> anyhow::Result<Vec<T>> {
        self.real_service.find_by_offsets(offsets).await
    }

    async fn upsert(&self, batch_id: u32, shares: Vec<T>) -> anyhow::Result<()> {
        let mut inner = self.inner.lock().await;
        if batch_id == 0 {
            // on the first ever batch, save the first share for later use
            let first = shares.first().cloned().ok_or_else(|| anyhow!("no shares"))?;
            inner.share = Some(first);
        }
        if inner.batch_size == 0 {
            bail!("no batch size set");
        }
        if shares.len() > 1 {
            warn!("Unexpected number of shares in fake mode: {}", shares.len())
        }
        let share = inner.share.clone();
        let batch_size = inner.batch_size;
        drop(inner);

        match share {
            Some(share) => {
                // now for every batch replicate the first share `batch_size` times.
                let shares = vec![share; batch_size];
                self.real_service.upsert(batch_id, shares).await
            }
            None => Err(anyhow!("no shares to be written")),
        }
    }

    async fn delete(&self, id: u32) -> anyhow::Result<()> {
        self.real_service.delete(id).await
    }
}

struct PreprocessingElementMetrics {
    find_by_offsets_duration: MaybeSingleMetric<SingleHistogram<Duration>>,
}

impl Default for PreprocessingElementMetrics {
    fn default() -> Self {
        let operation_duration: MaybeMetric<_> = Histogram::new(
            "preprocessing_repository_operation_seconds",
            "Time taken by each proprocessing repository operation",
            &["operation"],
            TimingBuckets::sub_ten_seconds(),
        )
        .into();
        let find_by_offsets_duration = operation_duration.with_labels([("operation", "find_by_offsets")]);
        Self { find_by_offsets_duration }
    }
}

#[allow(clippy::indexing_slicing)]
#[cfg(test)]
mod test {
    use super::*;
    use crate::services::blob::DefaultBlobService;
    use anyhow::Error;
    use rstest::rstest;

    async fn setup_elements(
        batch_count: u32,
        batch_size: u32,
        service: &mut DefaultPreprocessingBlobService<u32>,
    ) -> Result<(), Error> {
        for batch_id in 0..batch_count {
            let mut elements: Vec<u32> = Vec::new();
            for i in 0..batch_size {
                elements.push(((batch_id + 1) * 10 + i) as u32);
            }
            service.upsert(batch_id, elements).await?;
        }

        Ok(())
    }

    #[rstest(
        first_batch_id, last_batch_id, start_offset, total, found_elements,
        case::find_zero_1(0, 0, 0, 0, vec![]),
        case::find_zero_2(1, 2, 1, 0, vec![]),
        case::find_one_1(0, 0, 0, 1, vec![10]),
        case::find_one_2(2, 2, 1, 1, vec![31]),
        case::find_multiple_1(0, 0, 0, 2, vec![10, 11]),
        case::find_multiple_2(0, 0, 0, 3, vec![10, 11, 12]),
        case::find_multiple_3(0, 1, 0, 4, vec![10, 11, 12, 20]),
        case::find_multiple_4(0, 1, 0, 5, vec![10, 11, 12, 20, 21]),
        case::find_multiple_5(0, 1, 0, 6, vec![10, 11, 12, 20, 21, 22]),
    )]
    #[tokio::test]
    async fn test_find_by_offsets(
        first_batch_id: u32,
        last_batch_id: u32,
        start_offset: u32,
        total: u32,
        found_elements: Vec<u32>,
    ) {
        let batch_count = 3;
        let batch_size = 3;
        let mut service = DefaultPreprocessingBlobService::new(Box::new(DefaultBlobService::new_in_memory()));
        setup_elements(batch_count, batch_size, &mut service).await.expect("failed to setup elements");

        let offsets = PreprocessingElementOffsets { first_batch_id, last_batch_id, start_offset, total };
        let result_found_elements = service.find_by_offsets(&offsets).await.expect("failed to find elements");
        assert_eq!(result_found_elements, found_elements);
    }

    #[tokio::test]
    async fn test_find_by_offsets_errors() {
        let batch_count = 3;
        let batch_size = 3;
        let mut service = DefaultPreprocessingBlobService::new(Box::new(DefaultBlobService::new_in_memory()));
        setup_elements(batch_count, batch_size, &mut service).await.expect("failed to setup elements");

        // First batch id is greater than last batch id
        let result = service
            .find_by_offsets(&PreprocessingElementOffsets {
                first_batch_id: 5,
                last_batch_id: 0,
                start_offset: 0,
                total: 3,
            })
            .await;
        assert!(result.is_err());

        // Single batch only contains 3 elements but 4 are requested
        let result = service
            .find_by_offsets(&PreprocessingElementOffsets {
                first_batch_id: 0,
                last_batch_id: 0,
                start_offset: 0,
                total: 4,
            })
            .await;
        assert!(result.is_err());

        // Two batches contain 6 elements but 8 are requested
        let result = service
            .find_by_offsets(&PreprocessingElementOffsets {
                first_batch_id: 0,
                last_batch_id: 1,
                start_offset: 0,
                total: 8,
            })
            .await;
        assert!(result.is_err());

        // There are only 3 batches but 5 are requested
        let result = service
            .find_by_offsets(&PreprocessingElementOffsets {
                first_batch_id: 0,
                last_batch_id: 4,
                start_offset: 0,
                total: 15,
            })
            .await;
        assert!(result.is_err());
    }
}

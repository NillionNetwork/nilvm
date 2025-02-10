use crate::{
    channels::ClusterChannels, services::offsets::ElementOffsetsService, stateful::cleanup::CLEANUP_METRICS,
    PreprocessingConfigExt,
};
use anyhow::{anyhow, bail, Context};
use futures::future;
use node_api::preprocessing::rust::{CleanupUsedElementsRequest, PreprocessingElement};
use node_config::{PreprocessingConfig, PreprocessingProtocolConfig};
use std::{sync::Arc, time::Duration};
use strum::IntoEnumIterator;
use tokio::time::{sleep, timeout};
use tracing::{error, info, instrument, warn};

const PREPROCESSING_CLEANUP_INTERVAL: Duration = Duration::from_secs(900);
const PREPROCESSING_CLEANUP_TIMEOUT: Duration = Duration::from_secs(300);

pub(crate) struct UsedPreprocessingCleanup {
    element: PreprocessingElement,
    channels: Arc<dyn ClusterChannels>,
    offsets: Arc<dyn ElementOffsetsService>,
    config: PreprocessingProtocolConfig,
}

impl UsedPreprocessingCleanup {
    pub(crate) fn spawn(
        channels: Arc<dyn ClusterChannels>,
        offsets: Arc<dyn ElementOffsetsService>,
        config: PreprocessingConfig,
    ) {
        for element in PreprocessingElement::iter() {
            let channels = channels.clone();
            let offsets = offsets.clone();
            let config = config.element_config(&element).clone();
            tokio::spawn(async move {
                let cleanup = UsedPreprocessingCleanup { element, channels, offsets, config };
                cleanup.run().await
            });
        }
    }

    #[instrument("stateful.cleanup.preprocessing", skip(self), fields(element = %format!("{:?}", self.element)))]
    async fn run(self) {
        info!("Starting cleanup loop");
        loop {
            {
                let metric_name = format!("preprocessing-{}", self.element);
                let _timer = CLEANUP_METRICS.cleanup_timer(&metric_name);
                if let Err(e) = self.try_delete().await {
                    error!("Failed delete used preprocessing chunks: {e}");
                }
            }
            info!("Sleeping for {PREPROCESSING_CLEANUP_INTERVAL:?}");
            sleep(PREPROCESSING_CLEANUP_INTERVAL).await;
        }
    }

    #[allow(clippy::arithmetic_side_effects)]
    async fn try_delete(&self) -> anyhow::Result<()> {
        let element = self.element;
        let offsets = self.offsets.offsets(&element).await?;
        let delete_candidate = offsets.committed.saturating_sub(1);
        info!("Setting delete candidate offset to {delete_candidate}");
        if let Err(e) = self.offsets.set_delete_candidate_offset(element, delete_candidate).await {
            error!("Failed to store delete candidate offsets: {e}");
        }
        if offsets.delete_candidate_offset == -1 || offsets.delete_candidate_offset == offsets.deleted_offset {
            info!("Element {element} has no offsets to be deleted");
            return Ok(());
        }
        let batch_size = i64::try_from(self.config.batch_size).map_err(|_| anyhow!("batch size overflow"))?;
        if offsets.deleted_offset.wrapping_add(1).wrapping_rem(batch_size) != 0 {
            warn!("Last deleted offset for {element} is outside of a chunk boundary: {}", offsets.deleted_offset);
        }
        info!(
            "Element {element} has deleted_offset={}, candidate={}",
            offsets.deleted_offset, offsets.delete_candidate_offset
        );
        // start from the block after the last one deleted
        let start_chunk = (offsets.deleted_offset.wrapping_add(1)).wrapping_div(batch_size);
        let end_offset = offsets.delete_candidate_offset.wrapping_add(1);
        let remainder = end_offset.wrapping_rem(batch_size);
        let end_chunk = end_offset.saturating_sub(remainder).wrapping_div(batch_size);

        if start_chunk >= end_chunk {
            info!("No whole blocks are ready for deletion for element {element}");
            return Ok(());
        }
        info!("Need to delete chunks [{start_chunk}, {end_chunk})");
        let candidate_offset = end_chunk.wrapping_mul(batch_size).saturating_sub(1) as u64;
        let start_chunk = u64::try_from(start_chunk).context("invalid start chunk")?;
        let end_chunk = u64::try_from(end_chunk).context("invalid end chunk")?;
        self.delete_chunks(start_chunk, end_chunk, candidate_offset).await
    }

    async fn delete_chunks(&self, start_chunk: u64, end_chunk: u64, candidate_offset: u64) -> anyhow::Result<()> {
        let request = CleanupUsedElementsRequest { element: self.element, start_chunk, end_chunk };
        let mut futs = Vec::new();
        let parties = self.channels.all_parties();
        for party in &parties {
            let fut = self.channels.cleanup_used_elements(party.party_id.clone(), request.clone());
            futs.push(fut);
        }
        info!("Waiting for parties to finish deletion");
        let results = match timeout(PREPROCESSING_CLEANUP_TIMEOUT, future::join_all(futs)).await {
            Ok(results) => results,
            Err(_) => {
                bail!("timed out waiting for deletion");
            }
        };
        for (party, result) in parties.into_iter().zip(results) {
            if let Err(e) = result {
                bail!("Party {} failed to cleanup old chunks: {e}", party.party_id);
            }
        }
        info!("All parties cleaned up chunks [{start_chunk}, {end_chunk}], updating deleted offset");
        if let Err(e) = self.offsets.set_deleted_offset(self.element, candidate_offset).await {
            error!("Failed to store deleted offset: {e}");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::UsedPreprocessingCleanup;
    use crate::{
        channels::{MockClusterChannels, Party},
        services::offsets::MockElementOffsetsService,
        storage::repositories::offsets::PreprocessingOffsets,
    };
    use mockall::predicate::eq;
    use node_api::preprocessing::rust::{CleanupUsedElementsRequest, PreprocessingElement};
    use node_config::PreprocessingProtocolConfig;
    use rstest::rstest;
    use std::{ops::Range, sync::Arc};

    struct UsedPreprocessingBuilder {
        element: PreprocessingElement,
        channels: MockClusterChannels,
        offsets: MockElementOffsetsService,
        config: PreprocessingProtocolConfig,
    }

    impl Default for UsedPreprocessingBuilder {
        fn default() -> Self {
            Self {
                element: PreprocessingElement::Compare,
                channels: MockClusterChannels::default(),
                offsets: MockElementOffsetsService::default(),
                config: PreprocessingProtocolConfig { batch_size: 5, generation_threshold: 0, target_offset_jump: 0 },
            }
        }
    }

    impl UsedPreprocessingBuilder {
        fn build(self) -> UsedPreprocessingCleanup {
            UsedPreprocessingCleanup {
                element: self.element,
                channels: Arc::new(self.channels),
                offsets: Arc::new(self.offsets),
                config: self.config,
            }
        }
    }

    fn make_offsets(committed: u64, deleted_offset: i64, delete_candidate_offset: i64) -> PreprocessingOffsets {
        PreprocessingOffsets {
            element: PreprocessingElement::Compare,
            target: 0,
            latest: 0,
            committed,
            next_batch_id: 1,
            deleted_offset,
            delete_candidate_offset,
        }
    }

    #[tokio::test]
    async fn update_candidate() {
        let mut builder = UsedPreprocessingBuilder::default();
        builder.offsets.expect_offsets().with(eq(builder.element)).return_once(move |_| Ok(make_offsets(5, -1, -1)));
        builder
            .offsets
            .expect_set_delete_candidate_offset()
            .with(eq(builder.element), eq(4))
            .return_once(move |_, _| Ok(()));
        builder.build().try_delete().await.expect("deletion failed")
    }

    #[rstest]
    #[case::no_candidate(-1, -1)]
    #[case::partial(-1, 0)]
    #[case::almost_entire_block(-1, 3)]
    #[case::partial_past_deleted1(4, 6)]
    #[case::partial_past_deleted2(4, 8)]
    #[tokio::test]
    async fn nothing_to_clean_up(#[case] deleted: i64, #[case] candidate: i64) {
        let mut builder = UsedPreprocessingBuilder::default();
        builder
            .offsets
            .expect_offsets()
            .with(eq(builder.element))
            .return_once(move |_| Ok(make_offsets(5, deleted, candidate)));
        builder.offsets.expect_set_delete_candidate_offset().return_once(move |_, _| Ok(()));
        builder.build().try_delete().await.expect("deletion failed")
    }

    #[rstest]
    #[case::full_block1(-1, 4, 0..1,4 )]
    #[case::full_block2(-1, 6, 0..1, 4)]
    #[case::full_block_past_deleted1(4, 9, 1..2, 9)]
    #[case::full_block_past_deleted2(4, 10, 1..2, 9)]
    #[case::more_than_one_block1(-1, 9, 0..2, 9)]
    #[case::more_than_one_block2(-1, 10, 0..2, 9)]
    #[tokio::test]
    async fn cleanup_old_chunks(
        #[case] deleted: i64,
        #[case] candidate: i64,
        #[case] deleted_range: Range<u64>,
        #[case] new_delete_offset: u64,
    ) {
        use node_api::auth::rust::UserId;

        let mut builder = UsedPreprocessingBuilder::default();
        builder
            .offsets
            .expect_offsets()
            .with(eq(builder.element))
            .return_once(move |_| Ok(make_offsets(10, deleted, candidate)));
        builder.offsets.expect_set_delete_candidate_offset().return_once(move |_, _| Ok(()));
        let party = Party { party_id: vec![1].into(), user_id: UserId::from_bytes("bob") };
        {
            let party = party.clone();
            builder.channels.expect_all_parties().return_once(move || vec![party]);
        }
        let expected_request = CleanupUsedElementsRequest {
            element: builder.element,
            start_chunk: deleted_range.start,
            end_chunk: deleted_range.end,
        };
        builder
            .channels
            .expect_cleanup_used_elements()
            .with(eq(party.party_id), eq(expected_request))
            .return_once(move |_, _| Ok(()));
        builder
            .offsets
            .expect_set_deleted_offset()
            .with(eq(builder.element), eq(new_delete_offset))
            .return_once(|_, _| Ok(()));
        builder.build().try_delete().await.expect("deletion failed")
    }
}

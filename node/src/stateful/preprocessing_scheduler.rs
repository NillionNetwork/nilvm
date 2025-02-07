use crate::{
    channels::ClusterChannels,
    services::{offsets::ElementOffsetsService, uuid::UuidService},
    stateful::wait_for_preprocessing_results,
    PreprocessingConfigExt,
};
use anyhow::anyhow;
use futures::future;
use node_api::preprocessing::rust::{GeneratePreprocessingRequest, PreprocessingElement};
use node_config::{PreprocessingConfig, PreprocessingProtocolConfig};
use std::{collections::HashMap, iter, sync::Arc, time::Duration};
use strum::IntoEnumIterator;
use tokio::{
    sync::watch::{channel, Receiver, Sender},
    time::{sleep, timeout},
};
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, instrument, warn, Instrument};
use uuid::Uuid;

const SCHEDULE_DELAYS: Retries = Retries {
    delays: &[Duration::from_millis(500), Duration::from_secs(1), Duration::from_secs(5), Duration::from_secs(10)],
};

const GENERATION_TIMEOUT: Duration = Duration::from_secs(60);

/// A preprocessing scheduler handle.
pub(crate) struct SchedulerHandle {
    // We use a `watch::channel` since we only want a single notification per preprocessing
    // element. e.g. if 100 requests came in and consumed preprocessing, we only really care about
    // advancing the offset once for all those 100.
    senders: HashMap<PreprocessingElement, Sender<()>>,
}

impl SchedulerHandle {
    /// Notify the scheduler that we used some preprocessing elements.
    pub(crate) fn notify_used_elements(&self, elements: &[PreprocessingElement]) {
        for element in elements {
            let Some(sender) = self.senders.get(element) else {
                error!("No sender registered for element {element:?}");
                continue;
            };
            if sender.send(()).is_err() {
                error!("Sender for {element:?} dropped");
            }
        }
    }
}

#[derive(Clone)]
pub(crate) struct PreprocessingSchedulerServices {
    pub(crate) offsets: Arc<dyn ElementOffsetsService>,
    pub(crate) uuid: Arc<dyn UuidService>,
}

pub(crate) struct PreprocessingScheduler {
    channels: Arc<dyn ClusterChannels>,
    config: PreprocessingProtocolConfig,
    services: PreprocessingSchedulerServices,
    element: PreprocessingElement,
    receiver: Receiver<()>,
    cancel_token: CancellationToken,
}

impl PreprocessingScheduler {
    pub(crate) fn spawn(
        channels: Arc<dyn ClusterChannels>,
        config: PreprocessingConfig,
        services: PreprocessingSchedulerServices,
        cancel_token: CancellationToken,
    ) -> SchedulerHandle {
        let mut senders = HashMap::new();
        for element in PreprocessingElement::iter() {
            let (sender, receiver) = channel(());
            let config = config.element_config(&element).clone();
            let services = services.clone();
            let cancel_token = cancel_token.clone();
            let channels = channels.clone();
            let scheduler = PreprocessingScheduler { channels, config, services, element, receiver, cancel_token };
            info!("Spawning preprocessing scheduler for {element}");
            tokio::spawn(
                scheduler
                    .run()
                    .instrument(tracing::info_span!("preprocessing.scheduler", element = element.to_string())),
            );
            senders.insert(element, sender);
        }
        SchedulerHandle { senders }
    }

    async fn run(mut self) {
        let token = self.cancel_token.clone();
        if token.run_until_cancelled(self.do_run()).await.is_none() {
            warn!("Node is shutting down, aborting");
        }
    }

    async fn do_run(&mut self) {
        info!("Checking if we need to trigger preprocessing");
        self.loop_try_trigger_generation().await;

        loop {
            info!("Waiting for more preprocessing to be needed");
            if self.receiver.changed().await.is_err() {
                info!("Sender dropped, shutting down");
                return;
            }
            info!("Received notification that elements were used, checking");
            self.loop_try_trigger_generation().await;
        }
    }

    async fn loop_try_trigger_generation(&self) {
        let mut delays = SCHEDULE_DELAYS.iter();
        // Loop trying to generate preprocessing until we fill the pool
        loop {
            let delay = delays.next().unwrap_or(Duration::from_secs(10));
            match self.try_trigger_generation().await {
                Ok(PreprocessingResult::PoolFull) => return,
                Ok(PreprocessingResult::Generated { batch_id }) => {
                    match self
                        .services
                        .offsets
                        .advance_latest_offset(self.element, self.config.batch_size, batch_id)
                        .await
                    {
                        Ok(_) => {
                            info!("Offset advanced successfully");
                            // Reset the delays so we start over in case of failure
                            delays = SCHEDULE_DELAYS.iter();
                        }
                        Err(e) => {
                            error!("Failed to advance latest offset: {e}, sleeping for {delay:?}");
                            sleep(delay).await;
                        }
                    };
                }
                Err(e) => {
                    error!("Failed to trigger generation: {e}, sleeping for {delay:?}");
                    sleep(delay).await;
                }
            };
        }
    }

    async fn try_trigger_generation(&self) -> anyhow::Result<PreprocessingResult> {
        let offsets = self.services.offsets.offsets(&self.element).await?;
        let total = offsets.latest.saturating_sub(offsets.committed);
        let threshold = self.config.generation_threshold;
        info!(
            "Offsets: total={}, target={}, committed={}, latest={}",
            total, offsets.target, offsets.committed, offsets.latest
        );
        let mut target_offset = offsets.target;
        let remaining_to_target = target_offset.saturating_sub(offsets.committed);
        if remaining_to_target < threshold {
            // move the end offset by generation_threshold + target_offset_jump. this ensures
            // the first run moves the target offset a single time rather than chasing the
            // threshold by jumping multiple times
            target_offset = offsets
                .committed
                .wrapping_add(self.config.generation_threshold)
                .wrapping_add(self.config.target_offset_jump);
            info!(
                "Total elements ({total}) is lower than threshold ({threshold}), bumping target offset to {target_offset}"
            );
            self.services.offsets.set_target_offset(self.element, target_offset).await?;
        }
        if offsets.latest < target_offset {
            info!("Triggering preprocessing as we have {total} elements left, target offset is {target_offset}");
            let generation_id = self.services.uuid.generate();
            self.start_preprocessing(target_offset, generation_id, offsets.next_batch_id).await?;
            Ok(PreprocessingResult::Generated { batch_id: offsets.next_batch_id })
        } else {
            info!("Not triggering preprocessing as we have {total} elements, threshold is {threshold}");
            Ok(PreprocessingResult::PoolFull)
        }
    }

    #[instrument("preprocessing.schedule", skip(self))]
    async fn start_preprocessing(&self, target_offset: u64, generation_id: Uuid, batch_id: u64) -> anyhow::Result<()> {
        let request = GeneratePreprocessingRequest {
            generation_id: generation_id.as_bytes().to_vec(),
            batch_id,
            batch_size: self.config.batch_size as u32,
            element: self.element,
        };
        let mut futs = Vec::new();
        let parties = self.channels.all_parties();
        info!("Asking parties to start preprocessing and generate {} elements", self.config.batch_size);
        for party in &parties {
            let fut = self.channels.generate_preprocessing(party.party_id.clone(), request.clone());
            futs.push(fut);
        }
        let results = timeout(GENERATION_TIMEOUT, future::join_all(futs))
            .await
            .map_err(|_| anyhow!("timed out invoking generation"))?;

        info!("Waiting for parties to send finished report");
        let mut streams = Vec::new();
        for (party, stream) in parties.into_iter().zip(results) {
            let stream = ReceiverStream::new(stream?);
            streams.push((party, stream));
        }
        match timeout(GENERATION_TIMEOUT, wait_for_preprocessing_results(streams, |e| e.status)).await {
            Ok(Ok(_)) => {
                info!("All parties finished generation successfully");
                Ok(())
            }
            Ok(Err(e)) => Err(e),
            Err(_) => Err(anyhow!("timed out waiting for generation to finish")),
        }
    }
}

enum PreprocessingResult {
    Generated { batch_id: u64 },
    PoolFull,
}

struct Retries {
    delays: &'static [Duration],
}

impl Retries {
    fn iter(&self) -> impl Iterator<Item = Duration> {
        // SAFETY: this is a non empty slice so `last` can't fail
        #[allow(clippy::unwrap_used)]
        self.delays.iter().chain(iter::repeat(self.delays.last().unwrap())).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        channels::{MockClusterChannels, Party},
        services::{offsets::MockElementOffsetsService, uuid::MockUuidService},
        storage::repositories::offsets::PreprocessingOffsets,
    };
    use basic_types::PartyId;
    use mockall::predicate::eq;
    use node_api::preprocessing::rust::{GeneratePreprocessingResponse, PreprocessingProtocolStatus};
    use node_config::PreprocessingProtocolConfig;
    use rstest::rstest;
    use tokio::sync::mpsc::channel;
    use uuid::Uuid;

    struct PreprocessingSchedulerBuilder {
        channels: MockClusterChannels,
        offsets: MockElementOffsetsService,
        uuid: MockUuidService,
        config: PreprocessingProtocolConfig,
        element: PreprocessingElement,
    }

    impl PreprocessingSchedulerBuilder {
        fn build(self) -> PreprocessingScheduler {
            let Self { channels, offsets, uuid, config, element } = self;
            let channels = Arc::new(channels);
            let offsets = Arc::new(offsets);
            let uuid = Arc::new(uuid);
            let (_, receiver) = tokio::sync::watch::channel(());
            PreprocessingScheduler {
                channels,
                config,
                services: PreprocessingSchedulerServices { offsets, uuid },
                element,
                receiver,
                cancel_token: Default::default(),
            }
        }
    }

    impl Default for PreprocessingSchedulerBuilder {
        fn default() -> Self {
            let config = PreprocessingProtocolConfig { batch_size: 2, generation_threshold: 10, target_offset_jump: 5 };
            Self {
                channels: Default::default(),
                offsets: Default::default(),
                uuid: Default::default(),
                config,
                element: PreprocessingElement::Compare,
            }
        }
    }

    fn make_offsets(target: u64, latest: u64, committed: u64, next_batch_id: u64) -> PreprocessingOffsets {
        PreprocessingOffsets {
            element: PreprocessingElement::Compare,
            target,
            latest,
            committed,
            next_batch_id,
            deleted_offset: 0,
            delete_candidate_offset: 0,
        }
    }

    #[rstest]
    #[case::success(&[PreprocessingProtocolStatus::FinishedSuccess], true)]
    #[case::wait_success(&[PreprocessingProtocolStatus::WaitingPeers, PreprocessingProtocolStatus::FinishedSuccess], true)]
    #[case::wait_failure(&[PreprocessingProtocolStatus::FinishedFailure], false)]
    #[case::success_after_failure(
        &[PreprocessingProtocolStatus::FinishedFailure, PreprocessingProtocolStatus::FinishedSuccess],
        false
    )]
    #[tokio::test]
    async fn trigger_initial_preprocessing(#[case] responses: &[PreprocessingProtocolStatus], #[case] success: bool) {
        use node_api::auth::rust::UserId;

        let mut builder = PreprocessingSchedulerBuilder::default();
        let batch_id = 42;
        builder
            .offsets
            .expect_offsets()
            .with(eq(PreprocessingElement::Compare))
            .return_once(move |_| Ok(make_offsets(10, 1, 1, batch_id)));

        let generation_id = Uuid::new_v4();
        let request = GeneratePreprocessingRequest {
            generation_id: generation_id.as_bytes().to_vec(),
            batch_id,
            batch_size: 2,
            element: node_api::preprocessing::proto::element::PreprocessingElement::Compare,
        };
        builder.uuid.expect_generate().return_once(move || generation_id);
        builder
            .offsets
            .expect_set_target_offset()
            .with(eq(PreprocessingElement::Compare), eq(16))
            .return_once(|_, _| Ok(()));

        let party = Party { party_id: PartyId::from(vec![1]), user_id: UserId::from_bytes("bob") };
        let party_id = party.party_id.clone();
        builder.channels.expect_all_parties().return_once(move || vec![party]);

        let (sender, receiver) = channel(responses.len());
        for &status in responses {
            let response = GeneratePreprocessingResponse { status };
            sender.send(Ok(response)).await.expect("failed to send");
        }
        builder
            .channels
            .expect_generate_preprocessing()
            .with(eq(party_id), eq(request))
            .return_once(move |_, _| Ok(receiver));

        let scheduler = builder.build();
        let result = scheduler.try_trigger_generation().await;
        match (result, success) {
            (Ok(_), true) | (Err(_), false) => (),
            (Ok(_), false) => panic!("expected failure but succeeded"),
            (Err(_), true) => panic!("expected failure but failed"),
        }
    }

    #[tokio::test]
    async fn enough_elements_dont_trigger_preprocessing() {
        let mut builder = PreprocessingSchedulerBuilder::default();
        builder
            .offsets
            .expect_offsets()
            .with(eq(PreprocessingElement::Compare))
            .return_once(|_| Ok(make_offsets(10, 10, 0, 1)));

        let scheduler = builder.build();
        scheduler.try_trigger_generation().await.expect("scheduling failed");
    }
}

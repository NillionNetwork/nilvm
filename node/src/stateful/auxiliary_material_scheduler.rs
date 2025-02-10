use crate::{
    channels::ClusterChannels,
    stateful::wait_for_preprocessing_results,
    storage::repositories::auxiliary_material_meta::{AuxiliaryMaterialMetadata, AuxiliaryMaterialMetadataRepository},
};
use anyhow::anyhow;
use futures::future;
use node_api::preprocessing::rust::{AuxiliaryMaterial, GenerateAuxiliaryMaterialRequest};
use node_config::{AuxiliaryMaterialConfig, AuxiliaryMaterialProtocolConfig};
use std::{sync::Arc, time::Duration};
use strum::IntoEnumIterator;
use tokio::time::{sleep, timeout};
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn, Instrument};
use uuid::Uuid;

const GENERATION_TIMEOUT: Duration = Duration::from_secs(120);
const RETRY_DELAY: Duration = Duration::from_secs(5);

pub(crate) struct AuxiliaryMaterialScheduler {
    channels: Arc<dyn ClusterChannels>,
    metadata_repo: Arc<dyn AuxiliaryMaterialMetadataRepository>,
    material: AuxiliaryMaterial,
    expected_version: u32,
    cancel_token: CancellationToken,
}

impl AuxiliaryMaterialScheduler {
    pub(crate) fn spawn(
        channels: Arc<dyn ClusterChannels>,
        metadata_repo: Arc<dyn AuxiliaryMaterialMetadataRepository>,
        configs: AuxiliaryMaterialConfig,
        cancel_token: CancellationToken,
    ) {
        for material in AuxiliaryMaterial::iter() {
            let AuxiliaryMaterialProtocolConfig { enabled, version } = match material {
                AuxiliaryMaterial::Cggmp21AuxiliaryInfo => configs.cggmp21_aux_info.clone(),
            };
            if !enabled {
                warn!("Generation of {material} is disabled");
                continue;
            }
            let scheduler = Self {
                channels: channels.clone(),
                metadata_repo: metadata_repo.clone(),
                material,
                expected_version: version,
                cancel_token: cancel_token.clone(),
            };
            info!("Spawning scheduler for {material} material, expected version is {version}");
            tokio::spawn(async move {
                scheduler
                    .run()
                    .instrument(tracing::info_span!("auxiliary_material.scheduler", material = material.to_string()))
                    .await
            });
        }
    }

    async fn run(self) {
        loop {
            let Some(result) = self.cancel_token.run_until_cancelled(self.try_run()).await else {
                warn!("Node is shutting down, aborting");
                return;
            };
            match result {
                Ok(_) => {
                    return;
                }
                Err(e) => {
                    error!("Failed to run, retrying in {RETRY_DELAY:?}: {e}");
                    sleep(RETRY_DELAY).await;
                }
            };
        }
    }

    async fn try_run(&self) -> anyhow::Result<()> {
        info!("Looking up existing material metadata");
        let material = self.metadata_repo.find(self.material).await?;
        match material {
            Some(meta) if meta.generated_version == self.expected_version => {
                info!("Found existing material with expected version {}", meta.generated_version);
                return Ok(());
            }
            Some(meta) => {
                warn!("Found existing material with incorrect version ({}), need to generate", meta.generated_version);
            }
            None => {
                info!("No existing material found, need to generate");
            }
        };
        self.generate().await?;
        info!("Storing metadata in repository");
        self.metadata_repo
            .insert(AuxiliaryMaterialMetadata { material: self.material, generated_version: self.expected_version })
            .await?;
        Ok(())
    }

    async fn generate(&self) -> anyhow::Result<()> {
        let request = GenerateAuxiliaryMaterialRequest {
            generation_id: Uuid::new_v4().as_bytes().to_vec(),
            material: self.material,
            version: self.expected_version,
        };
        let mut futs = Vec::new();
        let parties = self.channels.all_parties();
        info!("Asking parties to start auxiliary material generation");
        for party in &parties {
            let fut = self.channels.generate_auxiliary_material(party.party_id.clone(), request.clone());
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
            // The wait function failed.
            Ok(Err(e)) => Err(e),
            // The wait function timed out.
            Err(_) => Err(anyhow!("timed out waiting for generation to finish")),
        }
    }
}

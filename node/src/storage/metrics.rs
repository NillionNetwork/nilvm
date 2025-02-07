//! Storage metrics.

use async_trait::async_trait;
use std::{sync::Arc, time::Duration};
use tokio::time::sleep;
use tracing::{error, info};

const EXPORT_INTERVAL: Duration = Duration::from_secs(60);

/// Export metrics.
///
/// This trait can be used for repositories that want to export metrics periodically.
#[async_trait]
pub trait ExportMetrics: Send + Sync {
    /// Export .
    async fn export_metrics(&self) -> anyhow::Result<()>;
}

/// A repository that knows how to export metrics.
pub type MetricsExporterRepository = Arc<dyn ExportMetrics>;

/// An actor that periodically exports metrics for a set of repositories.
pub struct StorageMetricsExporter;

impl StorageMetricsExporter {
    /// Construct a new metrics exporter.
    pub fn spawn(repositories: Vec<MetricsExporterRepository>) {
        tokio::spawn(async move {
            Self::run(repositories).await;
        });
    }

    async fn run(repositories: Vec<MetricsExporterRepository>) {
        let interval = EXPORT_INTERVAL;
        loop {
            sleep(interval).await;
            info!("Exporting metrics for {} repositories", repositories.len());
            for repository in &repositories {
                if let Err(e) = repository.export_metrics().await {
                    error!("Failed to export repository metrics: {e}");
                }
            }
        }
    }
}

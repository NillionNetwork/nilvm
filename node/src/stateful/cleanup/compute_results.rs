use crate::{services::results::ResultsService, stateful::cleanup::CLEANUP_METRICS};
use std::{sync::Arc, time::Duration};
use tokio::{spawn, time::sleep};
use tracing::{error, info};

const EXPIRY_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);
const MODEL: &str = "expired_values";

pub(crate) struct ExpiredComputeResultsCleanup;

impl ExpiredComputeResultsCleanup {
    pub(crate) fn spawn(service: Arc<dyn ResultsService>) {
        spawn(async move {
            Self::run(service).await;
        });
    }

    async fn run(service: Arc<dyn ResultsService>) {
        loop {
            info!("Removing expired compute results");
            {
                let _timer = CLEANUP_METRICS.cleanup_timer(MODEL);
                match service.delete_expired().await {
                    Ok(count) => {
                        info!("Deleted {count} expired compute results");
                        CLEANUP_METRICS.inc_total_removed(MODEL, count);
                    }
                    Err(e) => {
                        error!("Failed to delete compute results: {e}");
                    }
                }
            }
            info!("Sleeping for {EXPIRY_INTERVAL:?}");
            sleep(EXPIRY_INTERVAL).await;
        }
    }
}

use crate::{services::nonce::NonceService, stateful::cleanup::CLEANUP_METRICS};
use std::{sync::Arc, time::Duration};
use tokio::time::sleep;
use tracing::{error, info};

const NONCE_CLEANUP_INTERVAL: Duration = Duration::from_secs(60);
const MODEL: &str = "nonce";

pub(crate) struct NonceCleanup;

impl NonceCleanup {
    pub(crate) fn spawn(repo: Arc<dyn NonceService>) {
        tokio::spawn(async move {
            Self::run(repo).await;
        });
    }

    async fn run(repo: Arc<dyn NonceService>) {
        let interval = NONCE_CLEANUP_INTERVAL;
        loop {
            info!("Removing expired nonces");
            {
                let _timer = CLEANUP_METRICS.cleanup_timer(MODEL);
                match repo.remove_expired_nonces().await {
                    Ok(count) => {
                        info!("Deleted {count} expired values");
                        CLEANUP_METRICS.inc_total_removed(MODEL, count);
                    }
                    Err(e) => {
                        error!("Failed to remove expired nonces: {e}");
                    }
                }
            }
            info!("Sleeping for {interval:?}");
            sleep(interval).await;
        }
    }
}

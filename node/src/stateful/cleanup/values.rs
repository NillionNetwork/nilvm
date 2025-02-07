use crate::{services::user_values::UserValuesService, stateful::cleanup::CLEANUP_METRICS};
use std::{sync::Arc, time::Duration};
use tokio::{spawn, time::sleep};
use tracing::{error, info};

const EXPIRY_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);
const MODEL: &str = "expired_values";

pub(crate) struct ExpiredValuesCleanup;

impl ExpiredValuesCleanup {
    pub(crate) fn spawn(service: Arc<dyn UserValuesService>) {
        spawn(async move {
            Self::run(service).await;
        });
    }

    async fn run(service: Arc<dyn UserValuesService>) {
        loop {
            info!("Removing expired values");
            {
                let _timer = CLEANUP_METRICS.cleanup_timer(MODEL);
                match service.delete_expired().await {
                    Ok(count) => {
                        info!("Deleted {count} expired values");
                        CLEANUP_METRICS.inc_total_removed(MODEL, count);
                    }
                    Err(e) => {
                        error!("Failed to delete values: {e}");
                    }
                }
            }
            info!("Sleeping for {EXPIRY_INTERVAL:?}");
            sleep(EXPIRY_INTERVAL).await;
        }
    }
}

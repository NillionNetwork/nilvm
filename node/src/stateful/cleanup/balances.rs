use crate::{stateful::cleanup::CLEANUP_METRICS, storage::repositories::balances::AccountBalanceRepository};
use chrono::{Days, Utc};
use std::{sync::Arc, time::Duration};
use tokio::time::sleep;
use tracing::{error, info};

const BALANCES_CLEANUP_INTERVAL: Duration = Duration::from_secs(3600);
const MODEL: &str = "balances";

pub(crate) struct BalancesCleanup;

impl BalancesCleanup {
    pub(crate) fn spawn(repo: Arc<dyn AccountBalanceRepository>, expiration: Days) {
        tokio::spawn(async move {
            Self::run(repo, expiration).await;
        });
    }

    async fn run(repo: Arc<dyn AccountBalanceRepository>, expiration: Days) {
        let interval = BALANCES_CLEANUP_INTERVAL;
        loop {
            if let Some(threshold) = Utc::now().checked_sub_days(expiration) {
                info!("Removing balance expired before {threshold:?}");
                {
                    let _timer = CLEANUP_METRICS.cleanup_timer(MODEL);
                    match repo.remove_expired(threshold).await {
                        Ok(count) => {
                            info!("Deleted {count} expired {MODEL}");
                            CLEANUP_METRICS.inc_total_removed(MODEL, count);
                        }
                        Err(e) => {
                            error!("Failed to remove expired {MODEL}: {e}");
                        }
                    }
                }
            } else {
                error!("Threshold timestamp underflowed, will retry on next loop");
            }
            info!("Sleeping for {interval:?}");
            sleep(interval).await;
        }
    }
}

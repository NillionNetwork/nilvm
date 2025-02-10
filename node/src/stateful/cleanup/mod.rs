use metrics::prelude::*;
use once_cell::sync::Lazy;
use std::time::Duration;

pub(crate) mod balances;
pub(crate) mod compute_results;
pub(crate) mod nonces;
pub(crate) mod preprocessing;
pub(crate) mod values;

pub(crate) use nonces::NonceCleanup;
pub(crate) use preprocessing::UsedPreprocessingCleanup;
pub(crate) use values::ExpiredValuesCleanup;

static CLEANUP_METRICS: Lazy<Metrics> = Lazy::new(Metrics::default);

struct Metrics {
    cleanup_duration: MaybeMetric<Histogram<Duration>>,
    total_removed: MaybeMetric<Counter>,
}

impl Default for Metrics {
    fn default() -> Self {
        let cleanup_duration = Histogram::new(
            "cleanup_duration_seconds",
            "Duration of cleanups in seconds",
            &["model"],
            TimingBuckets::sub_second(),
        )
        .into();
        let total_removed = Counter::new("cleanup_total", "Total cleaned up entries", &["model"]).into();
        Self { cleanup_duration, total_removed }
    }
}

impl Metrics {
    fn cleanup_timer(&self, model: &str) -> ScopedTimer<impl SingleHistogramMetric<Duration>> {
        self.cleanup_duration.with_labels([("model", model)]).into_timer()
    }

    fn inc_total_removed(&self, model: &str, count: u64) {
        self.total_removed.with_labels([("model", model)]).inc_by(count)
    }
}

//! Utilities to use metrics without worrying about errors.

use crate::metrics::{
    LabelledMetric, Observable, SingleCounterMetric, SingleFloatCounterMetric, SingleGaugeMetric, SingleHistogramMetric,
};
use std::collections::HashMap;
use tracing::{error, warn};

/// A wrapper over a metric that may or may not exist, simplifying error handling in case anything
/// fails and the caller just wants to log errors/warnings if that's the case.
pub struct MaybeMetric<M> {
    metric: Option<M>,
}

impl<M, E> From<Result<M, E>> for MaybeMetric<M>
where
    M: LabelledMetric,
    E: std::error::Error,
{
    #[allow(unused_variables)]
    fn from(maybe_metric: Result<M, E>) -> Self {
        let metric = match maybe_metric {
            Ok(metric) => Some(metric),
            Err(e) => {
                error!("Failed to initialize metric: {e}");
                None
            }
        };
        Self { metric }
    }
}

impl<M: LabelledMetric> MaybeMetric<M> {
    /// Labels the metric and returns a labelled metric.
    ///
    /// If there's an error during the labelling, an error is returned and the returned metric will
    /// act as a dummy so any operations performed on it will effectively do nothing.
    ///
    /// ```rust,no_run
    /// # use metrics::prelude::*;
    ///
    /// let metric: MaybeMetric<Counter> = Counter::new("foo_total", "Number of foos", &["type"]).into();
    /// metric.with_labels([("type", "SUPER_FOO")]).inc();
    /// ```
    #[allow(unused_variables)]
    pub fn with_labels<const N: usize>(&self, labels: [(&str, &str); N]) -> MaybeSingleMetric<M::Inner> {
        let metric = match &self.metric {
            Some(metric) => {
                let labels = HashMap::from(labels);
                match metric.with_labels(&labels) {
                    Ok(metric) => Some(metric),
                    Err(e) => {
                        warn!("Failed to set metric labels: {e}");
                        None
                    }
                }
            }
            None => None,
        };
        MaybeSingleMetric { metric }
    }
}

/// Either a single metric or nothing.
///
/// This can be used to use metrics without spreading error checking all over the code.
#[derive(Clone)]
pub struct MaybeSingleMetric<M> {
    metric: Option<M>,
}

impl<M: SingleCounterMetric> SingleCounterMetric for MaybeSingleMetric<M> {
    fn inc(&self) {
        if let Some(metric) = &self.metric {
            metric.inc();
        }
    }

    fn inc_by(&self, value: u64) {
        if let Some(metric) = &self.metric {
            metric.inc_by(value);
        }
    }

    fn get(&self) -> u64 {
        if let Some(metric) = &self.metric { metric.get() } else { 0 }
    }
}

impl<M: SingleFloatCounterMetric> SingleFloatCounterMetric for MaybeSingleMetric<M> {
    fn inc_by(&self, value: f64) {
        if let Some(metric) = &self.metric {
            metric.inc_by(value);
        }
    }
    fn get(&self) -> f64 {
        if let Some(metric) = &self.metric { metric.get() } else { 0.0 }
    }
}

impl<M: SingleGaugeMetric> SingleGaugeMetric for MaybeSingleMetric<M> {
    fn inc(&self) {
        if let Some(metric) = &self.metric {
            metric.inc();
        }
    }

    fn dec(&self) {
        if let Some(metric) = &self.metric {
            metric.dec();
        }
    }

    fn set(&self, value: i64) {
        if let Some(metric) = &self.metric {
            metric.set(value);
        }
    }
}

impl<M, O> SingleHistogramMetric<O> for MaybeSingleMetric<M>
where
    M: SingleHistogramMetric<O>,
    O: Observable,
{
    fn observe(&self, value: &O) {
        if let Some(metric) = &self.metric {
            metric.observe(value);
        }
    }
}

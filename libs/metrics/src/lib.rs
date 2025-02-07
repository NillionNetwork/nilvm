//! This is a metrics system that is intended to be used on top of prometheus. Because of this, all
//! interfaces are _very_ aligned with how prometheus metrics work.
//!
//! While the default backend is prometheus, it can be disabled by not passing in the
//! `prometheus-backend` feature flag. In that case, the metric types under the [`noop`] module are
//! used, which provide the same interface but expose nothing instead.
//!
//! The [`initialize`] function needs to be called before defining any metrics. Any metrics created
//! after doing that will be automatically registered in the registry returned by that function.
//! **Exposing the metrics in that registry is the responsibility of the user of this crate**.
//!
//! # Example use
//! ```rust,no_run
//! # use std::collections::HashMap;
//! use metrics::{initialize, prelude::*};
//!
//! // First initialize the metrics system.
//! let static_labels = HashMap::from([
//!     ("hostname".to_string(), "potato.tastyvegetables.org".to_string())
//! ]);
//! metrics::initialize(static_labels)?;
//!
//! // Now define a counter.
//! let counter = Counter::new(
//!     "foo_total",
//!     "The total number of foos in the system",
//!     &["type"]
//! )?;
//!
//! // And increment it.
//! counter.with_labels(&HashMap::from([("type", "VERY_FOO")]))?
//!        .inc();
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ````

#![deny(missing_docs)]
#![forbid(unsafe_code)]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::iterator_step_by_zero,
    clippy::invalid_regex,
    clippy::string_slice,
    clippy::unimplemented,
    clippy::todo
)]
#![allow(clippy::module_inception)]

pub mod gauge;
pub mod maybe;
pub mod metrics;
pub mod timing;

// Note: we always expose the noop module just in case someone wants to swap their implementation
// in a test.
pub mod noop;

cfg_if::cfg_if! {
    if #[cfg(feature = "prometheus-backend")] {
        mod prometheus;
        type Metrics = prometheus::PrometheusMetricsEngine;
    }
    else {
        type Metrics = noop::NoopMetricsEngine;
    }
}

use crate::metrics::MetricsEngine;
use std::collections::HashMap;

/// A counter metric. See [`CounterMetric`][crate::metrics::CounterMetric].
pub type SingleCounter = <Metrics as MetricsEngine>::SingleCounter;

/// A labelled counter metric. See [`CounterMetric`][crate::metrics::CounterMetric].
pub type Counter = <Metrics as MetricsEngine>::Counter;

/// A float counter metric. See [`FloatCounterMetric`][crate::metrics::FloatCounterMetric].
pub type SingleFloatCounter = <Metrics as MetricsEngine>::SingleFloatCounter;

/// A labelled float counter metric. See [`FloatCounterMetric`][crate::metrics::FloatCounterMetric].
pub type FloatCounter = <Metrics as MetricsEngine>::FloatCounter;

/// A gauge metric. See [`GaugeMetric`][crate::metrics::GaugeMetric].
pub type SingleGauge = <Metrics as MetricsEngine>::SingleGauge;

/// A labelled gauge metric. See [`GaugeMetric`][crate::metrics::GaugeMetric].
pub type Gauge = <Metrics as MetricsEngine>::Gauge;

/// A histogram metric. See [`HistogramMetric`][crate::metrics::HistogramMetric].
pub type SingleHistogram<O> = <Metrics as MetricsEngine>::SingleHistogram<O>;

/// A labelled histogram metric. See [`HistogramMetric`][crate::metrics::HistogramMetric].
pub type Histogram<O> = <Metrics as MetricsEngine>::Histogram<O>;

/// The type used to represent the metric registry.
pub type Registry = <Metrics as MetricsEngine>::Registry;

/// The error returned during initialization.
pub type InitializeError = <Metrics as MetricsEngine>::InitializeError;

/// Initialize the system. See [`metrics::MetricsEngine::initialize`].
pub fn initialize(static_labels: HashMap<String, String>) -> Result<Registry, InitializeError> {
    Metrics::initialize(static_labels)
}

/// A prelude that imports all important types.
pub mod prelude {
    pub use super::{
        gauge::{BuildGauge, ScopedGauge},
        maybe::{MaybeMetric, MaybeSingleMetric},
        metrics::{
            CounterMetric, FloatCounterMetric, GaugeMetric, HistogramMetric, LabelledMetric, SingleCounterMetric,
            SingleFloatCounterMetric, SingleGaugeMetric, SingleHistogramMetric,
        },
        timing::{BuildTimer, ScopedTimer, TimingBuckets},
        Counter, FloatCounter, Gauge, Histogram, SingleCounter, SingleFloatCounter, SingleGauge, SingleHistogram,
    };
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::prelude::*;
    use std::time::Duration;

    #[test]
    fn counter() {
        let metric = Counter::new("labelled_foo_total", "Total number of foos by label", &["type", "size"])
            .expect("creation failed");
        let labels = HashMap::from([("type", "BAR"), ("size", "JUMBO")]);
        metric.with_labels(&labels).expect("labelling failed").inc();
    }

    #[test]
    fn float_counter() {
        let metric = FloatCounter::new("labelled_foo_total", "Total number of foos by label", &["type", "size"])
            .expect("creation failed");
        let labels = HashMap::from([("type", "BAR"), ("size", "JUMBO")]);
        metric.with_labels(&labels).expect("labelling failed").inc_by(42.1337);
    }

    #[test]
    fn gauge() {
        let metric = Gauge::new("foo_available_total", "Total number of available foos", &[]).expect("creation failed");
        metric.with_labels(&Default::default()).unwrap().set(42);
    }

    #[test]
    fn f64_histogram() {
        let metric = Histogram::<f64>::new("foo_size_total", "Size taken by each foo", &[], &[0.01, 0.1, 0.5, 1.0])
            .expect("creation failed");
        metric.with_labels(&Default::default()).unwrap().observe(&0.7);
    }

    #[test]
    fn duration_histogram() {
        let metric = Histogram::<Duration>::new(
            "foo_latency_total",
            "Latency taken by each foo request",
            &[],
            &[Duration::from_millis(100), Duration::from_secs(2)],
        )
        .expect("creation failed");
        metric.with_labels(&Default::default()).unwrap().observe(&Duration::from_millis(800));
    }
}

//! The supported metric types.
//!
//! These traits are only meant to guarantee the backend-specific implementations (e.g. prometheus)
//! and the no-op version follow the same signatures.

use instant::Duration;
use std::collections::HashMap;

/// A metric that contains an ever increasing counter.
///
/// This is useful to count events such as:
/// * Number of requests received.
pub trait CounterMetric: Sized + LabelledMetric {
    /// The type of error returned during creation.
    type CreateError: std::error::Error;

    /// Construct a new labelled counter.
    fn new<S1, S2>(name: S1, help: S2, labels: &[&str]) -> Result<Self, Self::CreateError>
    where
        S1: Into<String>,
        S2: Into<String>;
}

/// A single counter that can be incremented. See [`CounterMetric`].
pub trait SingleCounterMetric: Sized + Clone {
    /// Increment the counter by one.
    fn inc(&self);

    /// Increment the counter by an arbitrary number.
    fn inc_by(&self, value: u64);

    /// Gets the current counter value.
    fn get(&self) -> u64;
}

/// A metric that contains an ever increasing floating point counter.
///
/// This is useful to count events such as:
/// * Number of seconds that something has cumulatively spent.
pub trait FloatCounterMetric: Sized + LabelledMetric {
    /// The type of error returned during creation.
    type CreateError: std::error::Error;

    /// Construct a new labelled counter.
    fn new<S1, S2>(name: S1, help: S2, labels: &[&str]) -> Result<Self, Self::CreateError>
    where
        S1: Into<String>,
        S2: Into<String>;
}

/// A single float counter that can be incremented. See [`FloatCounterMetric`].
pub trait SingleFloatCounterMetric: Sized + Clone {
    /// Increment the counter by an arbitrary number.
    fn inc_by(&self, value: f64);

    /// Gets the current counter value.
    fn get(&self) -> f64;
}

/// A metric that contains a number that can go up and down.
///
/// This is useful to keep track of absolute values such as:
/// * CPU/memory usage.
/// * Number of active actors for a given type.
pub trait GaugeMetric: Sized + LabelledMetric {
    /// The type of error returned during creation.
    type CreateError: std::error::Error;

    /// Construct a new labelled gauge.
    fn new<S1, S2>(name: S1, help: S2, labels: &[&str]) -> Result<Self, Self::CreateError>
    where
        S1: Into<String>,
        S2: Into<String>;
}

/// A single gauge that can be set to a number. See [`GaugeMetric`].
pub trait SingleGaugeMetric: Sized + Clone {
    /// Sets the gauge's value to a specific number.
    fn set(&self, value: i64);

    /// Increments the gauge's value by one.
    fn inc(&self);

    /// Decrements the gauge's value by one.
    fn dec(&self);
}

/// A histogram metric that allows keeping track of values store in pre-defined buckets.
///
/// This is useful to keep track of percentiles for a metric's value. For example:
/// * The average and tail latency for the handling of a request.
///
/// This is parametrized by an [`Observable`] type, which represents the type being observed by
/// this histogram. By default, this is implemented for some basic types like `f64` and `Duration`.
pub trait HistogramMetric<O: Observable>: Sized + LabelledMetric {
    /// The type of error returned during creation.
    type CreateError: std::error::Error;

    /// Construct a new labelled gauge.
    fn new<S1, S2>(name: S1, help: S2, labels: &[&str], buckets: &[O]) -> Result<Self, Self::CreateError>
    where
        S1: Into<String>,
        S2: Into<String>;
}

/// A single histogram that values can be observed into.
pub trait SingleHistogramMetric<O: Observable>: Sized + Clone {
    /// Observes a value.
    fn observe(&self, value: &O);
}

/// A trait for all metric types that can be labelled.
pub trait LabelledMetric {
    /// The underlying type after labelling is applied.
    type Inner;

    /// A labelling error.
    type LabelError: std::error::Error;

    /// Applies the given labels and returns the underlying metric after labelling.
    ///
    /// It's recommended, if possible, to do the labelling only once and to keep reusing the
    /// returned metric.
    fn with_labels(&self, label_values: &HashMap<&str, &str>) -> Result<Self::Inner, Self::LabelError>;
}

/// An observable type that can be used in a histogram.
pub trait Observable: Clone {
    /// Converts the observable into a measurement.
    fn as_measurement(&self) -> f64;
}

impl Observable for Duration {
    fn as_measurement(&self) -> f64 {
        self.as_secs_f64()
    }
}

impl Observable for f64 {
    fn as_measurement(&self) -> f64 {
        *self
    }
}

impl Observable for u32 {
    fn as_measurement(&self) -> f64 {
        f64::from(*self)
    }
}

/// A registry that collects all metrics and allows accessing them.
pub trait MetricsRegistry: Clone {
    /// The error type returned during encoding.
    type EncodeError: std::error::Error;

    /// Encode the metrics in this registry into a string.
    fn encode_metrics(&self) -> Result<String, Self::EncodeError>;
}

/// The metrics engine to be used.
///
/// This defines what specific types are used for each type of metric.
pub trait MetricsEngine {
    /// The type used for counters.
    type SingleCounter: SingleCounterMetric;

    /// The type used for labelled counters.
    type Counter: CounterMetric;

    /// The type used for float counters.
    type SingleFloatCounter: SingleFloatCounterMetric;

    /// The type used for labelled float counters.
    type FloatCounter: FloatCounterMetric;

    /// The type used for gauges.
    type SingleGauge: SingleGaugeMetric;

    /// The type used for labelled gauges.
    type Gauge: GaugeMetric;

    /// The type used for histograms.
    type SingleHistogram<O: Observable>: SingleHistogramMetric<O>;

    /// The type used for labelled histograms.
    type Histogram<O: Observable>: HistogramMetric<O>;

    /// The type used as a registry for metrics.
    type Registry: MetricsRegistry;

    /// The error returned during initialization.
    type InitializeError: std::error::Error;

    /// Initializes the engine.
    ///
    /// This function must be called before any metrics are created. Failing to do so will cause
    /// metrics to be registered nowhere and therefore be lost. This behavior is specifically
    /// chosen so that metrics don't break tests, as a single metric can only be defined once,
    /// which means any type that defines a metric can only be instantiated once during tests which
    /// is an undesirable restriction.
    fn initialize(static_labels: HashMap<String, String>) -> Result<Self::Registry, Self::InitializeError>;
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn duration_as_measurement() {
        let duration = Duration::from_millis(1500);
        assert_eq!(duration.as_measurement(), 1.5);
    }
}

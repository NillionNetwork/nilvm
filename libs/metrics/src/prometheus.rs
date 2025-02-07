//! An implementation of the metrics interface using Prometheus.

use crate::metrics::{
    CounterMetric, FloatCounterMetric, GaugeMetric, HistogramMetric, LabelledMetric, MetricsEngine, MetricsRegistry,
    Observable, SingleCounterMetric, SingleFloatCounterMetric, SingleGaugeMetric, SingleHistogramMetric,
};
use once_cell::sync::OnceCell;
use std::{collections::HashMap, marker::PhantomData};

static GLOBALS: OnceCell<Globals> = OnceCell::new();

struct Globals {
    registry: prometheus::Registry,
    static_labels: HashMap<String, String>,
}

fn register<T>(metric: &T) -> Result<(), prometheus::Error>
where
    T: prometheus::core::Collector + Clone + 'static,
{
    if let Some(globals) = GLOBALS.get() {
        globals.registry.register(Box::new(metric.clone()))?;
    }
    Ok(())
}

fn build_options<S1, S2>(name: S1, help: S2) -> prometheus::Opts
where
    S1: Into<String>,
    S2: Into<String>,
{
    let mut options = prometheus::Opts::new(name, help);
    if let Some(globals) = GLOBALS.get() {
        options = options.const_labels(globals.static_labels.clone());
    }
    options
}

/// A prometheus counter.
#[derive(Clone)]
pub struct PrometheusSingleCounter {
    metric: prometheus::IntCounter,
}

impl SingleCounterMetric for PrometheusSingleCounter {
    fn inc(&self) {
        self.metric.inc();
    }

    fn inc_by(&self, value: u64) {
        self.metric.inc_by(value);
    }

    fn get(&self) -> u64 {
        self.metric.get()
    }
}

/// A prometheus labelled counter.
pub struct PrometheusCounter {
    metric: prometheus::IntCounterVec,
}

impl CounterMetric for PrometheusCounter {
    type CreateError = prometheus::Error;

    fn new<S1, S2>(name: S1, help: S2, labels: &[&str]) -> Result<Self, Self::CreateError>
    where
        S1: Into<String>,
        S2: Into<String>,
    {
        let options = build_options(name, help);
        let metric = prometheus::IntCounterVec::new(options, labels)?;
        register(&metric)?;
        Ok(Self { metric })
    }
}

impl LabelledMetric for PrometheusCounter {
    type LabelError = prometheus::Error;
    type Inner = PrometheusSingleCounter;

    fn with_labels(&self, label_values: &HashMap<&str, &str>) -> Result<Self::Inner, Self::LabelError> {
        let counter = self.metric.get_metric_with(label_values)?;
        Ok(PrometheusSingleCounter { metric: counter })
    }
}

/// A prometheus float counter.
#[derive(Clone)]
pub struct PrometheusSingleFloatCounter {
    metric: prometheus::Counter,
}

impl SingleFloatCounterMetric for PrometheusSingleFloatCounter {
    fn inc_by(&self, value: f64) {
        self.metric.inc_by(value);
    }

    fn get(&self) -> f64 {
        self.metric.get()
    }
}

/// A prometheus labelled float counter.
pub struct PrometheusFloatCounter {
    metric: prometheus::CounterVec,
}

impl FloatCounterMetric for PrometheusFloatCounter {
    type CreateError = prometheus::Error;

    fn new<S1, S2>(name: S1, help: S2, labels: &[&str]) -> Result<Self, Self::CreateError>
    where
        S1: Into<String>,
        S2: Into<String>,
    {
        let options = build_options(name, help);
        let metric = prometheus::CounterVec::new(options, labels)?;
        register(&metric)?;
        Ok(Self { metric })
    }
}

impl LabelledMetric for PrometheusFloatCounter {
    type LabelError = prometheus::Error;
    type Inner = PrometheusSingleFloatCounter;

    fn with_labels(&self, label_values: &HashMap<&str, &str>) -> Result<Self::Inner, Self::LabelError> {
        let counter = self.metric.get_metric_with(label_values)?;
        Ok(PrometheusSingleFloatCounter { metric: counter })
    }
}

/// A prometheus gauge.
#[derive(Clone)]
pub struct PrometheusSingleGauge {
    metric: prometheus::IntGauge,
}

impl SingleGaugeMetric for PrometheusSingleGauge {
    fn set(&self, value: i64) {
        self.metric.set(value);
    }

    fn inc(&self) {
        self.metric.inc();
    }

    fn dec(&self) {
        self.metric.dec();
    }
}

/// A prometheus labelled gauge.
pub struct PrometheusGauge {
    metric: prometheus::IntGaugeVec,
}

impl GaugeMetric for PrometheusGauge {
    type CreateError = prometheus::Error;

    fn new<S1, S2>(name: S1, help: S2, labels: &[&str]) -> Result<Self, Self::CreateError>
    where
        S1: Into<String>,
        S2: Into<String>,
    {
        let options = build_options(name, help);
        let metric = prometheus::IntGaugeVec::new(options, labels)?;
        register(&metric)?;
        Ok(Self { metric })
    }
}

impl LabelledMetric for PrometheusGauge {
    type LabelError = prometheus::Error;
    type Inner = PrometheusSingleGauge;

    fn with_labels(&self, label_values: &HashMap<&str, &str>) -> Result<Self::Inner, Self::LabelError> {
        let metric = self.metric.get_metric_with(label_values)?;
        Ok(PrometheusSingleGauge { metric })
    }
}

/// A prometheus histogram.
#[derive(Clone)]
pub struct PrometheusSingleHistogram<O> {
    metric: prometheus::Histogram,
    _unused: PhantomData<O>,
}

impl<O: Observable> SingleHistogramMetric<O> for PrometheusSingleHistogram<O> {
    fn observe(&self, value: &O) {
        self.metric.observe(value.as_measurement());
    }
}

/// A labelled prometheus histogram.
pub struct PrometheusHistogram<O> {
    metric: prometheus::HistogramVec,
    _unused: PhantomData<O>,
}

impl<O: Observable> HistogramMetric<O> for PrometheusHistogram<O> {
    type CreateError = prometheus::Error;

    fn new<S1, S2>(name: S1, help: S2, labels: &[&str], buckets: &[O]) -> Result<Self, Self::CreateError>
    where
        S1: Into<String>,
        S2: Into<String>,
    {
        let buckets = buckets.iter().map(Observable::as_measurement).collect();
        let common_opts = build_options(name, help);
        let options = prometheus::HistogramOpts { common_opts, buckets };
        let metric = prometheus::HistogramVec::new(options, labels)?;
        register(&metric)?;
        Ok(Self { metric, _unused: PhantomData })
    }
}

impl<O: Observable> LabelledMetric for PrometheusHistogram<O> {
    type LabelError = prometheus::Error;
    type Inner = PrometheusSingleHistogram<O>;

    fn with_labels(&self, label_values: &HashMap<&str, &str>) -> Result<Self::Inner, Self::LabelError> {
        let metric = self.metric.get_metric_with(label_values)?;
        Ok(PrometheusSingleHistogram { metric, _unused: PhantomData })
    }
}

/// The prometheus metric registry.
#[derive(Clone)]
pub struct PrometheusRegistry {
    registry: prometheus::Registry,
}

impl MetricsRegistry for PrometheusRegistry {
    type EncodeError = prometheus::Error;

    fn encode_metrics(&self) -> Result<String, Self::EncodeError> {
        let encoder = prometheus::TextEncoder::new();
        encoder.encode_to_string(&self.registry.gather())
    }
}

/// The prometheus metrics engine.
pub struct PrometheusMetricsEngine;

impl MetricsEngine for PrometheusMetricsEngine {
    type SingleCounter = PrometheusSingleCounter;
    type Counter = PrometheusCounter;
    type SingleFloatCounter = PrometheusSingleFloatCounter;
    type FloatCounter = PrometheusFloatCounter;
    type SingleGauge = PrometheusSingleGauge;
    type Gauge = PrometheusGauge;
    type SingleHistogram<O: Observable> = PrometheusSingleHistogram<O>;
    type Histogram<O: Observable> = PrometheusHistogram<O>;
    type Registry = PrometheusRegistry;
    type InitializeError = InitializeError;

    fn initialize(static_labels: HashMap<String, String>) -> Result<Self::Registry, Self::InitializeError> {
        let registry = prometheus::Registry::default();
        let globals = Globals { registry: registry.clone(), static_labels };
        GLOBALS.set(globals).ok().ok_or(InitializeError::AlreadyInitialized)?;
        Ok(PrometheusRegistry { registry })
    }
}

/// An error during the initialization of the prometheus metrics engine.
#[derive(Debug, thiserror::Error)]
pub enum InitializeError {
    /// The system is already initialized.
    #[error("already initialized")]
    AlreadyInitialized,
}

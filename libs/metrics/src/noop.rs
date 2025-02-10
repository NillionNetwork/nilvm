//! A no-op version of the metrics engine.

use crate::metrics::{
    CounterMetric, FloatCounterMetric, GaugeMetric, HistogramMetric, LabelledMetric, MetricsEngine, MetricsRegistry,
    Observable, SingleCounterMetric, SingleFloatCounterMetric, SingleGaugeMetric, SingleHistogramMetric,
};
use std::{collections::HashMap, marker::PhantomData};

/// A no-op counter.
#[derive(Clone)]
pub struct NoopSingleCounter;

impl SingleCounterMetric for NoopSingleCounter {
    fn inc(&self) {}
    fn inc_by(&self, _value: u64) {}
    fn get(&self) -> u64 {
        0
    }
}

/// A no-op labelled counter.
pub struct NoopCounter;

impl CounterMetric for NoopCounter {
    type CreateError = NoopError;

    fn new<S1, S2>(_name: S1, _help: S2, _labels: &[&str]) -> Result<Self, Self::CreateError>
    where
        S1: Into<String>,
        S2: Into<String>,
    {
        Ok(Self)
    }
}

impl LabelledMetric for NoopCounter {
    type LabelError = NoopError;
    type Inner = NoopSingleCounter;

    fn with_labels(
        &self,
        _label_values: &std::collections::HashMap<&str, &str>,
    ) -> Result<Self::Inner, Self::LabelError> {
        Ok(NoopSingleCounter)
    }
}

/// A no-op float counter.
#[derive(Clone)]
pub struct NoopSingleFloatCounter;

impl SingleFloatCounterMetric for NoopSingleFloatCounter {
    fn inc_by(&self, _value: f64) {}
    fn get(&self) -> f64 {
        0.0
    }
}

/// A no-op labelled float counter.
pub struct NoopFloatCounter;

impl FloatCounterMetric for NoopFloatCounter {
    type CreateError = NoopError;

    fn new<S1, S2>(_name: S1, _help: S2, _labels: &[&str]) -> Result<Self, Self::CreateError>
    where
        S1: Into<String>,
        S2: Into<String>,
    {
        Ok(Self)
    }
}

impl LabelledMetric for NoopFloatCounter {
    type LabelError = NoopError;
    type Inner = NoopSingleFloatCounter;

    fn with_labels(
        &self,
        _label_values: &std::collections::HashMap<&str, &str>,
    ) -> Result<Self::Inner, Self::LabelError> {
        Ok(NoopSingleFloatCounter)
    }
}

/// A no-op gauge.
#[derive(Clone)]
pub struct NoopSingleGauge;

impl SingleGaugeMetric for NoopSingleGauge {
    fn set(&self, _value: i64) {}
    fn inc(&self) {}
    fn dec(&self) {}
}

/// A no-op labelled gauge.
pub struct NoopGauge;

impl GaugeMetric for NoopGauge {
    type CreateError = NoopError;

    fn new<S1, S2>(_name: S1, _help: S2, _labels: &[&str]) -> Result<Self, Self::CreateError>
    where
        S1: Into<String>,
        S2: Into<String>,
    {
        Ok(Self)
    }
}

impl LabelledMetric for NoopGauge {
    type LabelError = NoopError;
    type Inner = NoopSingleGauge;

    fn with_labels(
        &self,
        _label_values: &std::collections::HashMap<&str, &str>,
    ) -> Result<Self::Inner, Self::LabelError> {
        Ok(NoopSingleGauge)
    }
}

/// A no-op histogram.
#[derive(Clone)]
pub struct NoopSingleHistogram<O> {
    _unused: PhantomData<O>,
}

impl<O: Observable> SingleHistogramMetric<O> for NoopSingleHistogram<O> {
    fn observe(&self, _value: &O) {}
}

/// A no-op labelled histogram.
pub struct NoopHistogram<O> {
    _unused: PhantomData<O>,
}

impl<O: Observable> HistogramMetric<O> for NoopHistogram<O> {
    type CreateError = NoopError;

    fn new<S1, S2>(_name: S1, _help: S2, _labels: &[&str], _buckets: &[O]) -> Result<Self, Self::CreateError>
    where
        S1: Into<String>,
        S2: Into<String>,
    {
        Ok(Self { _unused: PhantomData })
    }
}

impl<O: Observable> LabelledMetric for NoopHistogram<O> {
    type LabelError = NoopError;
    type Inner = NoopSingleHistogram<O>;

    fn with_labels(
        &self,
        _label_values: &std::collections::HashMap<&str, &str>,
    ) -> Result<Self::Inner, Self::LabelError> {
        Ok(NoopSingleHistogram { _unused: PhantomData })
    }
}

/// A no-op metrics registry.
#[derive(Clone)]
pub struct NoopRegistry;

impl MetricsRegistry for NoopRegistry {
    type EncodeError = NoopError;

    fn encode_metrics(&self) -> Result<String, Self::EncodeError> {
        Err(NoopError)
    }
}

/// A no-op metrics engine.
pub struct NoopMetricsEngine;

impl MetricsEngine for NoopMetricsEngine {
    type SingleCounter = NoopSingleCounter;
    type Counter = NoopCounter;
    type SingleFloatCounter = NoopSingleFloatCounter;
    type FloatCounter = NoopFloatCounter;
    type SingleGauge = NoopSingleGauge;
    type Gauge = NoopGauge;
    type SingleHistogram<O: Observable> = NoopSingleHistogram<O>;
    type Histogram<O: Observable> = NoopHistogram<O>;
    type Registry = NoopRegistry;
    type InitializeError = NoopError;

    fn initialize(_static_labels: HashMap<String, String>) -> Result<Self::Registry, Self::InitializeError> {
        Ok(NoopRegistry)
    }
}

/// A noop error. This should never actually be instantiated.
#[derive(Debug, thiserror::Error)]
#[error("no op")]
pub struct NoopError;

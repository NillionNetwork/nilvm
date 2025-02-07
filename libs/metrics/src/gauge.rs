//! Helpers for gauges.

use crate::metrics::SingleGaugeMetric;
use std::borrow::Cow;

/// A scoped gauge that is incremented by one on creation and down by one when dropped.
pub struct ScopedGauge<'a, G: SingleGaugeMetric> {
    gauge: Cow<'a, G>,
}

impl<'a, G: SingleGaugeMetric> ScopedGauge<'a, G> {
    /// Constructs a new scoped gauge.
    pub fn new(gauge: Cow<'a, G>) -> Self {
        gauge.inc();
        Self { gauge }
    }
}

impl<'a, G: SingleGaugeMetric> Drop for ScopedGauge<'a, G> {
    fn drop(&mut self) {
        self.gauge.dec();
    }
}

/// Allows creating a scoped gauge out of a gauge.
pub trait BuildGauge: SingleGaugeMetric {
    /// Create a scoped gauge over this metric.
    fn scoped_gauge(&self) -> ScopedGauge<'_, Self>;

    /// Create a scoped gauge over this metric, taking ownership of the metric.
    fn into_scoped_gauge(self) -> ScopedGauge<'static, Self>;
}

impl<T: SingleGaugeMetric> BuildGauge for T {
    /// Create a scoped gauge over this metric.
    fn scoped_gauge(&self) -> ScopedGauge<'_, Self> {
        ScopedGauge::new(Cow::Borrowed(self))
    }

    /// Create a scoped gauge over this metric, taking ownership of the metric.
    fn into_scoped_gauge(self) -> ScopedGauge<'static, Self> {
        ScopedGauge::new(Cow::Owned(self))
    }
}

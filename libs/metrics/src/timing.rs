//! Helpers to time operations.

use crate::metrics::SingleHistogramMetric;
use instant::{Duration, Instant};
use once_cell::sync::Lazy;
use std::borrow::Cow;

/// A scoped timer that observes the time elapsed on drop.
pub struct ScopedTimer<'a, M: SingleHistogramMetric<Duration>> {
    histogram: Cow<'a, M>,
    start_time: Instant,
}

impl<'a, M: SingleHistogramMetric<Duration>> ScopedTimer<'a, M> {
    /// Constructs a timer over the given histogram.
    pub fn new(histogram: Cow<'a, M>) -> Self {
        Self { histogram, start_time: Instant::now() }
    }
}

impl<'a, M: SingleHistogramMetric<Duration>> Drop for ScopedTimer<'a, M> {
    fn drop(&mut self) {
        let elapsed = self.start_time.elapsed();
        self.histogram.observe(&elapsed);
    }
}

/// Allows constructing a timer out of a histogram.
pub trait BuildTimer: SingleHistogramMetric<Duration> {
    /// Create a timer over this metric.
    fn timer(&self) -> ScopedTimer<'_, Self>;

    /// Create a timer over this metric, taking ownership of the metric.
    fn into_timer(self) -> ScopedTimer<'static, Self>;
}

impl<T: SingleHistogramMetric<Duration>> BuildTimer for T {
    /// Create a timer over this metric.
    fn timer(&self) -> ScopedTimer<'_, Self> {
        ScopedTimer::new(Cow::Borrowed(self))
    }

    /// Create a timer over this metric, taking ownership of the metric.
    fn into_timer(self) -> ScopedTimer<'static, Self> {
        ScopedTimer::new(Cow::Owned(self))
    }
}

/// The buckets for a histogram that operates on a `Duration`.
pub struct TimingBuckets;

impl TimingBuckets {
    /// Returns buckets for an operation that is expected to take less than a second
    pub fn sub_second() -> &'static [Duration] {
        static BUCKETS: Lazy<Vec<Duration>> = Lazy::new(|| {
            vec![
                Duration::from_millis(1),
                Duration::from_millis(5),
                Duration::from_millis(10),
                Duration::from_millis(25),
                Duration::from_millis(50),
                Duration::from_millis(100),
                Duration::from_millis(250),
                Duration::from_millis(500),
                Duration::from_millis(1000),
            ]
        });
        &BUCKETS
    }

    /// Returns buckets for an operation that is expected to take less than 10 seconds.
    pub fn sub_ten_seconds() -> &'static [Duration] {
        static BUCKETS: Lazy<Vec<Duration>> = Lazy::new(|| {
            vec![
                Duration::from_millis(1),
                Duration::from_millis(25),
                Duration::from_millis(50),
                Duration::from_millis(100),
                Duration::from_millis(250),
                Duration::from_millis(500),
                Duration::from_millis(1000),
                Duration::from_millis(2000),
                Duration::from_millis(5000),
                Duration::from_millis(10000),
            ]
        });
        &BUCKETS
    }

    /// Returns buckets for an operation that is expected to take up to a minute.
    pub fn sub_minute() -> &'static [Duration] {
        static BUCKETS: Lazy<Vec<Duration>> = Lazy::new(|| {
            vec![
                Duration::from_millis(1),
                Duration::from_millis(250),
                Duration::from_millis(500),
                Duration::from_secs(1),
                Duration::from_secs(2),
                Duration::from_secs(5),
                Duration::from_secs(10),
                Duration::from_secs(20),
                Duration::from_secs(30),
                Duration::from_secs(45),
                Duration::from_secs(60),
            ]
        });
        &BUCKETS
    }
}

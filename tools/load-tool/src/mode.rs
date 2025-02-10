//! The runner mode.

use crate::{report::FlowReport, spec::WorkerIncrementMode};
use anyhow::bail;
use std::time::{Duration, Instant};

const DURATION_CHECK_WINDOW_SIZE: usize = 10;
const MAX_DURATION_CHECK_FREQUENCY: Duration = Duration::from_secs(5);
const DEFAULT_AUTOMATIC_MODE_WORKERS: u32 = 5;

// Automatic mode increments the number of workers every 10 seconds by 20%.
const DEFAULT_AUTOMATIC_MODE_INCREMENT_RATIO: f64 = 0.2;
const DEFAULT_AUTOMATIC_MODE_INCREMENT_FREQUENCY: Duration = Duration::from_secs(10);

/// The mode the runner is using.
pub struct RunnerMode {
    worker_increment_mode: WorkerIncrementMode,
    target_workers: u32,
    max_test_duration: Option<Duration>,
    max_flow_duration: Duration,
    started_at: Instant,
    last_increment_at: Instant,
    last_latency_validation_at: Instant,
}

impl RunnerMode {
    /// Construct a new runner mode.
    pub fn new(
        worker_increment_mode: WorkerIncrementMode,
        max_test_duration: Option<Duration>,
        max_flow_duration: Duration,
    ) -> Self {
        use WorkerIncrementMode::*;
        let target_workers = match &worker_increment_mode {
            Manual { initial_workers, .. } => *initial_workers,
            Automatic => DEFAULT_AUTOMATIC_MODE_WORKERS,
            Steady { workers, .. } => *workers,
        };
        let now = Instant::now();
        Self {
            worker_increment_mode,
            target_workers,
            max_test_duration,
            max_flow_duration,
            started_at: now,
            last_increment_at: now,
            last_latency_validation_at: now,
        }
    }

    /// The current target worker count.
    pub fn target_workers(&self) -> u32 {
        self.target_workers
    }

    /// Ticks the runner mode to perform any state updates.
    pub fn tick(&mut self, reports: &[FlowReport]) -> anyhow::Result<()> {
        let target_frequency = match self.worker_increment_mode {
            WorkerIncrementMode::Manual { worker_increment_frequency, .. } => worker_increment_frequency,
            WorkerIncrementMode::Automatic => DEFAULT_AUTOMATIC_MODE_INCREMENT_FREQUENCY,
            WorkerIncrementMode::Steady { .. } => Duration::MAX,
        };
        if self.last_increment_at.elapsed() >= target_frequency {
            self.increment_workers();
            self.last_increment_at = Instant::now();
        }
        if self.last_latency_validation_at.elapsed() >= MAX_DURATION_CHECK_FREQUENCY {
            self.validate_max_duration(reports)?;
            self.last_latency_validation_at = Instant::now();
        }
        let elapsed = self.started_at.elapsed();
        let max_duration = self.max_test_duration.unwrap_or(Duration::MAX);
        if elapsed > max_duration {
            bail!("test has been running for longer than the configured time limit: {elapsed:?} > {max_duration:?}");
        }
        Ok(())
    }

    // Validate that the last N reports don't average more than our max allowed duration.
    fn validate_max_duration(&self, reports: &[FlowReport]) -> anyhow::Result<()> {
        let window_size = reports.len().min(DURATION_CHECK_WINDOW_SIZE) as u32;
        if window_size == 0 {
            return Ok(());
        }
        let reports_window = reports.iter().rev().take(DURATION_CHECK_WINDOW_SIZE);
        let window_latency = reports_window.map(|report| report.summary.elapsed).sum::<Duration>();
        let window_mean = window_latency / window_size;
        if window_mean >= self.max_flow_duration {
            bail!("flow duration ({:?}) is beyond the maximum allowed ({:?})", window_mean, self.max_flow_duration)
        } else {
            Ok(())
        }
    }

    fn increment_workers(&mut self) {
        use WorkerIncrementMode::*;
        let increment = match self.worker_increment_mode {
            Manual { worker_increment, .. } => worker_increment,
            Automatic => (self.target_workers as f64 * DEFAULT_AUTOMATIC_MODE_INCREMENT_RATIO).ceil() as u32,
            Steady { .. } => 0,
        };
        self.target_workers += increment;
    }
}

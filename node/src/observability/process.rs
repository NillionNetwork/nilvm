//! Collector for process metrics.

use metrics::prelude::*;
use std::time::Duration;
use tokio::time::sleep;
use tracing::warn;

#[cfg(target_os = "linux")]
use {
    once_cell::sync::Lazy,
    procfs::{net::TcpState, process::Process, WithCurrentSystemInfo},
};

#[cfg(target_os = "linux")]
static TICKS_PER_SECOND: Lazy<f64> = Lazy::new(|| procfs::ticks_per_second() as f64);

/// Metrics about the node process.
#[allow(dead_code)]
pub struct ProcessMetricsCollector {
    cpu_seconds: MaybeMetric<FloatCounter>,
    resident_memory: MaybeMetric<Gauge>,
    threads: MaybeMetric<Gauge>,
    open_fds: MaybeMetric<Gauge>,
    storage_io_bytes: MaybeMetric<Counter>,
    storage_io_syscalls: MaybeMetric<Counter>,
    tcp_connections: MaybeMetric<Gauge>,
}

impl Default for ProcessMetricsCollector {
    fn default() -> Self {
        let cpu_seconds =
            FloatCounter::new("process_cpu_seconds_total", "Total user and system CPU time spent", &[]).into();
        let resident_memory = Gauge::new("process_resident_memory_bytes", "Resident memory size in bytes", &[]).into();
        let threads = Gauge::new("process_threads", "Number of OS threads in the process", &[]).into();
        let open_fds = Gauge::new("open_file_descriptors", "Number of open file descriptors", &[]).into();
        let storage_io_bytes =
            Counter::new("storage_io_bytes_total", "Number of bytes read/written from/to storage", &["operation"])
                .into();
        let storage_io_syscalls =
            Counter::new("storage_io_syscalls_total", "Number of read/write syscalls issued", &["operation"]).into();
        let tcp_connections =
            Gauge::new("established_tcp_connections", "Number of established TCP connections", &[]).into();
        Self { cpu_seconds, resident_memory, threads, open_fds, storage_io_bytes, storage_io_syscalls, tcp_connections }
    }
}

impl ProcessMetricsCollector {
    /// Run the process metrics collector.
    pub async fn run(self, interval: Duration) {
        loop {
            self.collect_metrics();
            sleep(interval).await;
        }
    }

    #[cfg(not(target_os = "linux"))]
    fn collect_metrics(&self) {
        warn!("Process metrics are not supported on this platform");
    }

    #[cfg(target_os = "linux")]
    fn collect_metrics(&self) {
        let metrics = match Process::myself() {
            Ok(metrics) => metrics,
            Err(e) => {
                warn!("Failed to load procfs entry: {e}");
                return;
            }
        };
        let stat = match metrics.stat() {
            Ok(stat) => stat,
            Err(e) => {
                warn!("Failed to load procfs stat: {e}");
                return;
            }
        };
        let tick_rate = *TICKS_PER_SECOND;
        // CPU time is cumulative so we need to subtract the last value. We sort of need a
        // counter that supports `set` but that can break the counter contract, so we do it
        // ourselves.
        let metric = self.cpu_seconds.with_labels([]);
        let current = metric.get();
        match stat.utime.checked_add(stat.stime) {
            Some(total_ticks) => {
                let total_seconds = total_ticks as f64 / tick_rate;
                let delta = total_seconds - current;
                metric.inc_by(delta);
            }
            None => warn!("CPU time calculation overflowed"),
        };
        let rss = stat.rss_bytes().get() as i64;
        self.resident_memory.with_labels([]).set(rss);

        if let Some(count) = metrics.fd_count().ok().and_then(|c| i64::try_from(c).ok()) {
            self.open_fds.with_labels([]).set(count);
        }
        self.threads.with_labels([]).set(stat.num_threads);

        if let Ok(io) = metrics.io() {
            let operation_values = [("read", io.read_bytes, io.syscr), ("write", io.write_bytes, io.syscw)];
            for (operation, bytes, syscalls) in operation_values {
                // See notes on gauge vs counter semantics needed for CPU time.
                self.increment_by_delta(self.storage_io_bytes.with_labels([("operation", operation)]), bytes);
                self.increment_by_delta(self.storage_io_syscalls.with_labels([("operation", operation)]), syscalls);
            }
        }

        if let Ok(net) = metrics.tcp() {
            let established_count = net.iter().filter(|connection| connection.state == TcpState::Established).count();
            self.tcp_connections.with_labels([]).set(i64::try_from(established_count).unwrap_or(0));
        }
    }

    #[cfg(target_os = "linux")]
    fn increment_by_delta<C: SingleCounterMetric>(&self, counter: C, latest_value: u64) {
        let current = counter.get();
        if let Some(delta) = latest_value.checked_sub(current) {
            counter.inc_by(delta);
        }
    }
}

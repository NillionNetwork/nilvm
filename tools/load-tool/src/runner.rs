//! Runtime implementation

use crate::{
    clients_pool::{Clients, ClientsPool},
    flow::{ErrorPolicy, ExecutionContext, Flow, FlowStatus, FlowSummary},
    mode::RunnerMode,
    report::{FlowReport, ReportGenerator},
    spec::WorkerIncrementMode,
    worker::Worker,
};
use anyhow::Context;
use futures::channel::mpsc::{channel, Receiver, Sender};
use log::{error, info, warn};
use serde::Deserialize;
use std::{
    path::PathBuf,
    process,
    sync::atomic::{AtomicBool, AtomicU32, Ordering},
    time::Duration,
};
use tokio::time::{sleep, Instant};

/// Runner context.
pub struct RunnerContext {
    /// The nillion client to be used.
    pub clients_pool: ClientsPool,

    /// The channel in which results should be sent.
    pub sender: Sender<FlowSummary>,
}

/// Load testing runner implementation.
pub struct LoadTestRunner {
    config: RunnerConfig,
    sender: Sender<FlowSummary>,
    receiver: Receiver<FlowSummary>,
    total_workers: &'static AtomicU32,
    running: &'static AtomicBool,
    started_at: Instant,
    mode: RunnerMode,
    window_last_differences: Vec<Duration>,
}

/// Runner configuration.
pub struct RunnerConfig {
    /// Report output path.
    pub output_path: PathBuf,

    /// The maximum amount of time we want to run this for.
    pub max_test_duration: Option<Duration>,

    /// The maximum average flow duration we're willing to tolerate.
    pub max_flow_duration: Duration,

    /// The maximum error rate we're willing to tolerate.
    pub max_error_rate: f64,

    /// The worker increment mode.
    pub mode: WorkerIncrementMode,

    /// The start policy to use.
    pub start_policy: StartPolicy,

    /// The error policy to use in the executed flow.
    pub error_policy: ErrorPolicy,
}

/// A runner error.
#[derive(Debug, thiserror::Error)]
#[error("runner error: {0}")]
pub struct RunnerError(String);

impl LoadTestRunner {
    /// Create a new instance of the Load Test Runtime from a descriptor file
    pub fn new(config: RunnerConfig) -> Self {
        let (sender, receiver) = channel(10000);
        LoadTestRunner {
            mode: RunnerMode::new(config.mode.clone(), config.max_test_duration, config.max_flow_duration),
            config,
            sender,
            receiver,
            total_workers: Box::leak(Box::default()),
            running: Box::leak(Box::new(AtomicBool::new(true))),
            started_at: Instant::now(),
            window_last_differences: vec![],
        }
    }

    /// Run load testing
    pub async fn run(mut self, mut clients_pool: ClientsPool, flow: Flow) -> anyhow::Result<()> {
        let mut clients = clients_pool.next().context("no clients available")?;

        self.apply_start_policy(&mut clients).await?;

        let cluster_info = clients.vm.cluster().clone();
        let context = RunnerContext { clients_pool, sender: self.sender.clone() };
        let flow_metadata = flow.metadata(cluster_info).await?;

        match self.run_tests(flow, context).await {
            Ok(reports) => {
                ReportGenerator::write_report(&self.config.output_path, flow_metadata, reports)?;
                Ok(())
            }
            Err(e) => {
                error!("Test run failed with an error: {e}");
                process::exit(1);
            }
        }
    }

    async fn apply_start_policy(&self, clients: &mut Clients) -> anyhow::Result<()> {
        match self.config.start_policy {
            StartPolicy::StartImmediately => (),
            StartPolicy::WaitForPreprocessing => loop {
                let status = clients.vm.pool_status().invoke().await?;
                if !status.preprocessing_active {
                    info!("Preprocessing pool is full, starting test");
                    break;
                }
                info!("Preprocessing pool status: {status:?}");
                info!("Waiting for preprocessing pool to be full...");
                sleep(Duration::from_secs(10)).await;
            },
        };
        Ok(())
    }

    async fn run_tests(&mut self, flow: Flow, mut context: RunnerContext) -> anyhow::Result<Vec<FlowReport>> {
        let mut reports = Vec::new();
        let mut total_failed_reports = 0;
        let mut error_rate = 0.0;

        self.started_at = Instant::now();
        while self.running.load(Ordering::Acquire) {
            if let Err(e) = self.mode.tick(&reports) {
                warn!("Stopping execution: {e}");
                break;
            }
            self.try_spawn_workers(&flow, &mut context).await?;

            let report_count = reports.len();
            self.try_receive_all(&mut reports);
            if reports.len() > report_count {
                // Figure out how many of the new ones failed and keep the error rate updated.
                total_failed_reports += reports[report_count..reports.len()]
                    .iter()
                    .filter(|report| matches!(report.summary.status, FlowStatus::Failure))
                    .count();
                error_rate = total_failed_reports as f64 / reports.len() as f64;
                if error_rate > self.config.max_error_rate {
                    warn!(
                        "Error rate {error_rate:.2} is higher than maximum allowed ({:.2}), stopping execution",
                        self.config.max_error_rate
                    );
                    break;
                }
            }
            sleep(Duration::from_millis(10)).await;
        }
        self.running.store(false, Ordering::Release);

        info!("Waiting for all workers to finish...");
        while self.total_workers.load(Ordering::Acquire) > 0 {
            sleep(Duration::from_millis(10)).await;
        }
        let total_reports = reports.len();
        let error_rate_percent = error_rate * 100.0;
        info!("All workers finished, {total_failed_reports}/{total_reports} ({error_rate_percent:.2}%) flows failed");
        if let Err(e) = context.clients_pool.log_balances().await {
            error!("Failed to report used nilchain balances: {e}");
        }

        if error_rate > self.config.max_error_rate {
            Err(RunnerError("error rate exceeded".to_string()).into())
        } else {
            Ok(reports)
        }
    }

    async fn try_spawn_workers(&mut self, flow: &Flow, context: &mut RunnerContext) -> anyhow::Result<()> {
        let total_workers = self.total_workers.swap(self.mode.target_workers(), Ordering::AcqRel);
        let missing_workers = self.mode.target_workers().saturating_sub(total_workers);
        if missing_workers == 0 {
            return Ok(());
        }

        info!("Have {total_workers} workers, need to spawn {missing_workers} more");
        for _ in 0..missing_workers {
            let flow = flow.clone();
            let context = ExecutionContext {
                clients: context.clients_pool.next().context("no clients available")?,
                sender: self.sender.clone(),
            };
            let policy = self.config.error_policy.clone();
            let total_workers = self.total_workers;
            let running = self.running;
            tokio::spawn(async move { Self::launch_worker(flow, context, policy, running, total_workers).await });
        }

        Ok(())
    }

    async fn launch_worker(
        flow: Flow,
        context: ExecutionContext,
        policy: ErrorPolicy,
        running: &'static AtomicBool,
        total_workers: &AtomicU32,
    ) {
        let worker = Worker::new(context, policy, running);
        log::info!("Spawning new worker...");

        match worker.run(flow).await {
            Ok(_) => info!("Worker finished successfully"),
            Err(e) => warn!("Worker finished with an error: {e}"),
        };
        total_workers.fetch_sub(1, Ordering::AcqRel);

        if running.swap(false, Ordering::Release) {
            info!("Worker finished, stopping execution...");
        }
    }

    fn try_receive_all(&mut self, reports: &mut Vec<FlowReport>) {
        while let Ok(summary) = self.receiver.try_next() {
            if let Some(summary) = summary {
                self.process_test_report(summary, reports);
            }
        }
    }

    fn process_test_report(&mut self, summary: FlowSummary, reports: &mut Vec<FlowReport>) {
        let total_workers = self.total_workers.load(Ordering::Acquire);
        log::info!(
            "Finished flow in {:?}, status: {:?}, total workers: {}",
            summary.elapsed,
            summary.status,
            total_workers
        );
        if matches!(summary.status, FlowStatus::Success) {
            self.window_last_differences.push(summary.elapsed);
        };

        let report = FlowReport { summary, elapsed_since_start: self.started_at.elapsed(), total_workers };
        reports.push(report);
    }
}

/// The policy used to start the testing process.
#[derive(Deserialize, Debug)]
pub enum StartPolicy {
    /// Start immediately.
    StartImmediately,

    /// Wait for preprocessing pools to be full.
    WaitForPreprocessing,
}

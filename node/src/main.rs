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

use anyhow::Error;
use clap::Parser;
use clap_utils::ParserExt;
use node::{
    builder::{NodeBuilder, NodeHandle, PreprocessingMode},
    observability::tracing::TracingConsumer,
};
use node_config::Config;
use std::path::PathBuf;
use tokio::{
    select,
    signal::unix::{SignalKind, signal},
};
use tracing::{Instrument, error, info, info_span};

/// The Nillion node.
#[derive(Parser)]
struct Cli {
    #[clap(env)]
    config_path: PathBuf,

    #[clap(long, hide = true, env = "FAKE_PREPROCESSING")]
    fake_preprocessing: bool,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let cli = Cli::parse_with_version();
    let _ = std::env::var("RUST_LOG").map_err(|_| std::env::set_var("RUST_LOG", "node=debug"));
    let config = Config::new(cli.config_path)?;
    let _tracing_consumer = match config.tracing.clone() {
        Some(config) => TracingConsumer::new(config)?,
        None => {
            let consumer = TracingConsumer::default();
            info!("Using default tracing configuration");
            consumer
        }
    };

    match &config.metrics {
        Some(config) => NodeBuilder::initialize_metrics(config).await?,
        None => info!("Disabling prometheus metrics as no endpoint was provided"),
    };
    let preprocessing_mode = if cli.fake_preprocessing {
        info!("Using fake preprocessing");
        PreprocessingMode::Fake
    } else {
        PreprocessingMode::Real
    };
    let handle = NodeBuilder::new(config).preprocessing_mode(preprocessing_mode).launch()?;
    if let Err(e) = run_until_signal(handle).instrument(info_span!(parent: None, "node.signal_handlers")).await {
        error!("Failed to run node: {e}");
        Err(e)
    } else {
        Ok(())
    }
}

async fn run_until_signal(handle: NodeHandle) -> anyhow::Result<()> {
    let mut term_signal = signal(SignalKind::terminate())?;
    let mut interrupt_signal = signal(SignalKind::interrupt())?;
    let mut hangup_signal = signal(SignalKind::hangup())?;

    select! {
        _ = term_signal.recv() => info!("Signal TERM received"),
        _ = interrupt_signal.recv() => info!("Signal INT received"),
        _ = hangup_signal.recv() => info!("Signal HANG received"),
    };

    info!("Stopping the node gracefully");
    handle.shutdown().await;
    Ok(())
}

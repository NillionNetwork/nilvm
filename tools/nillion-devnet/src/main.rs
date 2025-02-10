use anyhow::Result;
use clap_utils::ParserExt;
use client_metrics::ClientMetrics;
use log::LevelFilter;
use nillion_devnet::{args::Cli, cluster::ClusterOrchestrator};
use node::{config::TracingConfig, observability::tracing::TracingConsumer};

async fn run(cli: Cli) -> Result<()> {
    let orchestrator = ClusterOrchestrator::new(cli)?;
    orchestrator.run().await?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse_with_version();
    let _tracing_guard = match cli.enable_tracing {
        true => {
            let tracing_config = TracingConfig { stdout: true, json_path: None };
            Some(TracingConsumer::new(tracing_config)?)
        }
        false => {
            // Enable logging but disable logging in metrics crate as it fails to register the same metric
            // twice when they're enabled.
            env_logger::builder().filter(Some("metrics"), LevelFilter::Off).init();
            None
        }
    };

    let client_metrics = ClientMetrics::new_default("nillion-devnet");
    let handler = client_metrics.send_event("run", None);

    let result = run(cli).await;

    let _ = handler.await;
    match result {
        Ok(_) => Ok(()),
        Err(e) => {
            let _ = client_metrics.send_error("error", &e, None).await;
            eprintln!("Failed to run devnet: {e:#}");
            std::process::exit(1);
        }
    }
}

//! Tracing setup.

use anyhow::{Context, Error};
use node_config::TracingConfig;
use std::{
    io,
    path::{Path, PathBuf},
};
use tracing_appender::non_blocking::{NonBlocking, WorkerGuard};
use tracing_subscriber::{
    fmt::{
        format::{FmtSpan, Format, Json, JsonFields},
        Layer,
    },
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter,
};

type JsonLayer<S> = Layer<S, JsonFields, Format<Json>, NonBlocking>;

/// Helper to set up tracing.
#[must_use]
pub struct TracingConsumer {
    _json_appender_guard: Option<WorkerGuard>,
}

impl Default for TracingConsumer {
    fn default() -> Self {
        let stdout_layer = tracing_subscriber::fmt::layer().with_writer(io::stdout);
        let registry = tracing_subscriber::registry().with(EnvFilter::from_default_env()).with(stdout_layer);
        registry.init();
        Self { _json_appender_guard: None }
    }
}

impl TracingConsumer {
    /// Set up tracing.
    pub fn new(config: TracingConfig) -> Result<Self, Error> {
        let TracingConfig { json_path, stdout } = config;
        let (json_layer, json_guard) = Self::setup_json_layer(json_path)?;
        let flat = match stdout {
            true => Some(tracing_subscriber::fmt::layer().with_writer(io::stdout)),
            false => None,
        };

        let registry = tracing_subscriber::registry().with(EnvFilter::from_default_env()).with(json_layer).with(flat);
        registry.init();
        Ok(Self { _json_appender_guard: json_guard })
    }

    fn setup_json_layer<S>(json_path: Option<PathBuf>) -> Result<(Option<JsonLayer<S>>, Option<WorkerGuard>), Error> {
        if let Some(json_path) = json_path {
            let json_log = Path::new(&json_path);

            let appender = tracing_appender::rolling::never(
                json_log.parent().with_context(|| format!("failed to start log on file {:#?}", json_path))?,
                json_log.file_name().with_context(|| format!("failed to start log on file {:#?}", json_path))?,
            );

            let (non_blocking_appender, guard) = tracing_appender::non_blocking(appender);
            let json_layer = tracing_subscriber::fmt::layer()
                .json()
                .with_span_events(FmtSpan::CLOSE)
                .with_writer(non_blocking_appender);
            Ok((Some(json_layer), Some(guard)))
        } else {
            Ok((None, None))
        }
    }
}

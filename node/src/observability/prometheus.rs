//! The prometheus initialization and metrics exporting code lives here.

use anyhow::Error;
use axum::{extract::Extension, http::StatusCode, response::IntoResponse, routing::get, Router};
use metrics::metrics::MetricsRegistry;
use std::{collections::HashMap, net::SocketAddr};
use tokio::net::TcpListener;
use tracing::{error, info, warn};

/// Exports prometheus metrics defined by the `metrics` crate.
pub struct PrometheusExporter {
    router: Router,
}

impl PrometheusExporter {
    /// Initializes the exporter to be run on the given endpoint.
    pub fn new(static_labels: HashMap<String, String>) -> Result<Self, Error> {
        let registry = metrics::initialize(static_labels)?;
        let router = Router::new().route("/metrics", get(get_metrics)).layer(Extension(registry));
        Ok(Self { router })
    }

    /// Launches the exporter in the specified address.
    ///
    /// This will spawn a future and keep it running in the background.
    pub fn launch(self, address: SocketAddr) {
        info!("Launching prometheus metrics exporter on {address}");
        tokio::spawn(async move {
            let listener = match TcpListener::bind(&address).await {
                Ok(listener) => listener,
                Err(e) => {
                    error!("Error binding to metrics endpoint: {e}");
                    return;
                }
            };
            let result = axum::serve(listener, self.router.into_make_service()).await;
            match result {
                Ok(_) => (),
                Err(e) => error!("Error serving metrics: {e}"),
            }
        });
    }
}

async fn get_metrics(Extension(registry): Extension<metrics::Registry>) -> Result<impl IntoResponse, StatusCode> {
    match registry.encode_metrics() {
        Ok(t) => Ok(t),
        Err(e) => {
            warn!("Failed to encode metrics: {e}");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

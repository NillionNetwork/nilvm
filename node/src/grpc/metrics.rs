use async_trait::async_trait;
use http::HeaderMap;
use metrics::prelude::*;
use once_cell::sync::Lazy;
use std::time::{Duration, Instant};
use tonic::{
    body::BoxBody,
    codegen::http::{Request, Response},
    Code,
};
use tonic_middleware::{Middleware, ServiceBound};
use tracing::info;

static METRICS: Lazy<Metrics> = Lazy::new(Metrics::default);

#[derive(Default, Clone)]
pub(crate) struct MetricsMiddleware;

impl MetricsMiddleware {
    fn extract_status_code(headers: &HeaderMap) -> Code {
        match headers.get("grpc-status") {
            Some(value) => {
                // default to internal error if we can't turn this into a string
                let value = value.to_str().unwrap_or("13");
                Code::from_bytes(value.as_bytes())
            }
            // tonic won't set this on success
            None => Code::Ok,
        }
    }
}

#[async_trait]
impl<S> Middleware<S> for MetricsMiddleware
where
    S: ServiceBound,
    S::Future: Send,
{
    async fn call(&self, req: Request<BoxBody>, mut service: S) -> Result<Response<BoxBody>, S::Error> {
        let uri = req.uri().path().to_string();
        let start_time = Instant::now();
        let response = service.call(req).await?;
        let elapsed = start_time.elapsed();
        let headers = response.headers();
        let status_code = Self::extract_status_code(headers);
        // ignore unimplemented so we don't pollute prometheus with random strings
        if status_code != Code::Unimplemented {
            let status_code = match status_code {
                Code::Ok => "Ok",
                Code::Cancelled => "Cancelled",
                Code::Unknown => "Unknown",
                Code::InvalidArgument => "InvalidArgument",
                Code::DeadlineExceeded => "DeadlineExceeded",
                Code::NotFound => "NotFound",
                Code::AlreadyExists => "AlreadyExists",
                Code::PermissionDenied => "PermissionDenied",
                Code::ResourceExhausted => "ResourceExhausted",
                Code::FailedPrecondition => "FailedPrecondition",
                Code::Aborted => "Aborted",
                Code::OutOfRange => "OutOfRange",
                Code::Unimplemented => "Unimplemented",
                Code::Internal => "Internal",
                Code::Unavailable => "Unavailable",
                Code::DataLoss => "DataLoss",
                Code::Unauthenticated => "Unauthenticated",
            };
            METRICS.observe_request_duration(&uri, status_code, elapsed);
            info!("Request to {uri} processed in {elapsed:?}, status code: {status_code}");
        }
        Ok(response)
    }
}

struct Metrics {
    request_duration: MaybeMetric<Histogram<Duration>>,
}

impl Default for Metrics {
    fn default() -> Self {
        let request_duration = Histogram::new(
            "grpc_request_duration_seconds",
            "Duration of each grpc request in seconds",
            &["method", "status_code"],
            TimingBuckets::sub_ten_seconds(),
        )
        .into();

        Self { request_duration }
    }
}

impl Metrics {
    fn observe_request_duration(&self, uri: &str, status_code: &str, elapsed: Duration) {
        self.request_duration.with_labels([("method", uri), ("status_code", status_code)]).observe(&elapsed);
    }
}

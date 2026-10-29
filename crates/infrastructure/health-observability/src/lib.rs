//! Telemetry for central, edge and desktop (ADR 0047; deployment.md § Observability).
//!
//! - [`init`] installs the process's `tracing` subscriber: an env filter and the
//!   [`ExportLayer`], which exports spans over OTLP (when an endpoint is configured) and events
//!   as JSON log lines on stdout, both through the [`allowlist`].
//! - No other layer exports anything, so names, phone numbers, NINs and free text never leave
//!   the node in telemetry. The redaction test in `tests/redaction.rs` checks this.

pub mod allowlist;
mod layer;

use std::sync::Arc;

use opentelemetry::trace::TracerProvider as _;
use opentelemetry_otlp::{SpanExporter, WithExportConfig};
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::trace::SdkTracerProvider;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;

pub use layer::{ExportLayer, LogSink, Stdout};

/// How a process reports telemetry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TelemetryConfig {
    /// `service.name`, e.g. `central-server`.
    pub service: &'static str,
    /// The OTLP/HTTP traces endpoint of the in-country collector, e.g.
    /// `http://otel-collector:4318/v1/traces`. `None` exports logs only.
    pub otlp_endpoint: Option<String>,
    /// An `EnvFilter` directive, e.g. `info` or `info,health_surrealdb=debug`.
    pub filter: String,
}

/// Why telemetry could not start.
#[derive(Debug, thiserror::Error)]
pub enum TelemetryError {
    /// The filter directive is invalid.
    #[error("invalid log filter: {0}")]
    Filter(String),
    /// The OTLP exporter could not be built.
    #[error("cannot build the OTLP exporter: {0}")]
    Exporter(String),
    /// A global subscriber is already installed.
    #[error("a tracing subscriber is already installed")]
    AlreadyInstalled,
}

/// Flushes and shuts down the exporter when dropped; keep it alive for the process.
#[derive(Debug)]
#[must_use = "dropping the guard shuts telemetry down"]
pub struct Telemetry {
    provider: Option<SdkTracerProvider>,
}

impl Drop for Telemetry {
    fn drop(&mut self) {
        if let Some(provider) = self.provider.take() {
            let _ = provider.shutdown();
        }
    }
}

/// A tracer provider exporting to the OTLP/HTTP endpoint in batches.
pub fn otlp_provider(
    service: &'static str,
    endpoint: &str,
) -> Result<SdkTracerProvider, TelemetryError> {
    let exporter = SpanExporter::builder()
        .with_http()
        .with_endpoint(endpoint)
        .build()
        .map_err(|e| TelemetryError::Exporter(e.to_string()))?;
    Ok(SdkTracerProvider::builder()
        .with_resource(Resource::builder().with_service_name(service).build())
        .with_batch_exporter(exporter)
        .build())
}

/// Installs the global subscriber described by `config`.
pub fn init(config: &TelemetryConfig) -> Result<Telemetry, TelemetryError> {
    let filter =
        EnvFilter::try_new(&config.filter).map_err(|e| TelemetryError::Filter(e.to_string()))?;
    let provider = config
        .otlp_endpoint
        .as_deref()
        .map(|endpoint| otlp_provider(config.service, endpoint))
        .transpose()?;
    let tracer = provider.as_ref().map(|p| p.tracer(config.service));
    let subscriber = tracing_subscriber::registry()
        .with(filter)
        .with(ExportLayer::new(tracer, Arc::new(Stdout)));
    tracing::subscriber::set_global_default(subscriber)
        .map_err(|_| TelemetryError::AlreadyInstalled)?;
    Ok(Telemetry { provider })
}

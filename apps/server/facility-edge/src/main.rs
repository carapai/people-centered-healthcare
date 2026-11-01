//! Facility Edge Node composition root (ADR 0007, facility-edge.md): one OS service, one
//! process, embedding SurrealDB. `health-edge-runtime` runs the node.
//!
//! - `facility-edge`: foreground, for development in a terminal.
//! - `facility-edge --service`: under the service manager. On Linux that is systemd,
//!   `Type=notify` (`packaging/facility-edge.service`).
//!
//! Exit codes, for the service manager and the update rollback:
//!
//! | Code | Meaning |
//! |---|---|
//! | 0 | stopped on request or on UPS low battery |
//! | 2 | the configuration is missing or malformed |
//! | 3 | the store cannot be opened (another process holds it, or the disk failed) |
//! | 4 | a migration failed, or the store was migrated by a newer binary |
//! | 5 | a supervised task kept failing |
//! | 1 | anything else (the listener, telemetry, serving) |

use std::process::ExitCode;
use std::sync::Arc;

use health_edge_runtime::{EdgeConfig, EdgeError, Foreground, ServiceManager, Systemd, run};
use health_observability::TelemetryConfig;

/// The service manager for `--service`.
fn service_manager() -> Result<Arc<dyn ServiceManager>, &'static str> {
    Systemd::from_env(|name| std::env::var(name).ok())
        .map(|systemd| Arc::new(systemd) as Arc<dyn ServiceManager>)
        .ok_or("--service needs a service manager: NOTIFY_SOCKET is unset")
}

#[tokio::main]
async fn main() -> ExitCode {
    let service = match std::env::args().nth(1).as_deref() {
        None => false,
        Some("--service") => true,
        Some(_) => {
            eprintln!("usage: facility-edge [--service]");
            return ExitCode::from(2);
        }
    };
    let config = match EdgeConfig::from_lookup(|name| std::env::var(name).ok()) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("facility-edge: {error}");
            return ExitCode::from(2);
        }
    };
    let manager: Arc<dyn ServiceManager> = if service {
        match service_manager() {
            Ok(manager) => manager,
            Err(error) => {
                eprintln!("facility-edge: {error}");
                return ExitCode::from(2);
            }
        }
    } else {
        Arc::new(Foreground)
    };
    let _telemetry = match health_observability::init(&TelemetryConfig {
        service: "facility-edge",
        otlp_endpoint: config.otlp_endpoint.clone(),
        filter: config.log.clone(),
    }) {
        Ok(telemetry) => telemetry,
        Err(error) => {
            eprintln!("facility-edge: {error}");
            return ExitCode::from(2);
        }
    };
    match run(&config, manager, health_edge_runtime::shutdown_signal()).await {
        Ok(stop) => {
            tracing::info!(reason = ?stop, "facility edge exited");
            ExitCode::SUCCESS
        }
        Err(error) => {
            tracing::error!(error.kind = %error, "facility edge failed");
            ExitCode::from(match error {
                EdgeError::Store => 3,
                EdgeError::Migration => 4,
                EdgeError::Task(_) => 5,
                EdgeError::Listen(_) | EdgeError::Serve(_) => 1,
            })
        }
    }
}

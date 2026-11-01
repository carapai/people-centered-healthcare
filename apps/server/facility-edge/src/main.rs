//! Facility Edge Node composition root (ADR 0007, facility-edge.md): one OS service, one
//! process, embedding SurrealDB. `health-edge-runtime` runs the node.
//!
//! - `facility-edge`: foreground, for development in a terminal.
//! - `facility-edge --service`: under the service manager ([`service`]). On Linux that is
//!   systemd, `Type=notify` (`packaging/facility-edge.service`); on Windows, the Service Control
//!   Manager (`packaging/install-service.ps1`).
//! - `facility-edge update …`: signed updates ([`update`]).
//! - `facility-edge backup …`: backups on demand, to USB, verify and restore ([`backup`]).
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
//! | 6 | a failed update was rolled back; the next start runs the previous binary |
//! | 7 | an update step failed, or an `update` command was refused: needs a person |
//! | 8 | a failed update's store is restored; the service manager restores the previous binary |
//! | 9 | a `backup` command failed |
//! | 1 | anything else (the listener, telemetry, serving) |

mod backup;
mod service;
mod update;

use std::future::Future;
use std::process::ExitCode;
use std::sync::Arc;

use health_edge_runtime::admin::{self, Buffer};
use health_edge_runtime::{EdgeConfig, EdgeError, Foreground, ServiceManager, run};
use health_observability::{LogSink, Stdout, TelemetryConfig};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let config = match EdgeConfig::from_lookup(|name| std::env::var(name).ok()) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("facility-edge: {error}");
            return ExitCode::from(2);
        }
    };
    match args.first().map(String::as_str) {
        None => ExitCode::from(block_on(&config, Arc::new(Foreground), None)),
        Some("--service") if args.len() == 1 => service::run(config),
        Some("update") => match update::run(&config, &args[1..]) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("facility-edge: {error}");
                ExitCode::from(7)
            }
        },
        Some("backup") => match backup::run(&config, &args[1..]) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("facility-edge: {error}");
                ExitCode::from(9)
            }
        },
        Some(_) => {
            eprintln!("usage: facility-edge [--service | update … | backup …]");
            ExitCode::from(2)
        }
    }
}

/// Runs the node on a new runtime until `stop` resolves, or the stop signals when `stop` is
/// `None`; returns the exit code.
fn block_on(
    config: &EdgeConfig,
    manager: Arc<dyn ServiceManager>,
    stop: Option<std::pin::Pin<Box<dyn Future<Output = ()> + Send>>>,
) -> u8 {
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(_) => {
            eprintln!("facility-edge: cannot start the async runtime");
            return 1;
        }
    };
    runtime.block_on(async {
        let stop = stop.unwrap_or_else(|| Box::pin(health_edge_runtime::shutdown_signal()));
        serve(config, manager, stop).await
    })
}

/// Telemetry, then the node; the exit code (see the table above).
async fn serve(
    config: &EdgeConfig,
    manager: Arc<dyn ServiceManager>,
    stop: impl Future<Output = ()>,
) -> u8 {
    // Log lines go to standard output and the bounded buffer, shipped when online. Without a
    // usable buffer directory they go to standard output alone.
    let buffer = Buffer::open(&config.telemetry_dir, admin::DEFAULT_CAP)
        .map(Arc::new)
        .ok();
    let logs: Arc<dyn LogSink> = match &buffer {
        Some(buffer) => buffer.clone(),
        None => Arc::new(Stdout),
    };
    let telemetry = TelemetryConfig {
        service: "facility-edge",
        otlp_endpoint: config.otlp_endpoint.clone(),
        filter: config.log.clone(),
    };
    let _telemetry = match health_observability::init_with(&telemetry, logs) {
        Ok(telemetry) => telemetry,
        Err(error) => {
            eprintln!("facility-edge: {error}");
            return 2;
        }
    };
    if buffer.is_none() {
        tracing::warn!(
            alert = "telemetry-unbuffered",
            "cannot open HEALTH_TELEMETRY_DIR: telemetry is not buffered"
        );
    }
    match run(config, manager, buffer, stop).await {
        Ok(stop) => {
            tracing::info!(reason = ?stop, "facility edge exited");
            0
        }
        Err(error) => {
            tracing::error!(error.kind = %error, "facility edge failed");
            match error {
                EdgeError::Store => 3,
                EdgeError::Migration => 4,
                EdgeError::Task(_) => 5,
                EdgeError::RolledBack => 6,
                EdgeError::Update(_) => 7,
                EdgeError::RollbackPending => 8,
                EdgeError::Listen(_) | EdgeError::Serve(_) => 1,
            }
        }
    }
}

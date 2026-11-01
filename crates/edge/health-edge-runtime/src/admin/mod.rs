//! Health monitoring (facility-edge.md § Health monitoring; deployment.md § Observability).
//!
//! - **Indicators:** node identity and version, store size, free disk on the data and backup
//!   volumes, the backup schedule, the UPS, supervised-task restarts and the telemetry buffer.
//!   The report judges them against the alert thresholds.
//! - **The admin page:** `GET /api/admin/status` returns the [`Report`] as JSON for the shared
//!   React app's edge status page. Until the edge's local authorization server can require an
//!   admin (#75), it answers only requests from the node itself.
//! - **Telemetry:** every interval the indicators are logged, and each alert is logged when
//!   raised and when cleared. Log lines go to the bounded [`Buffer`] and are shipped to the
//!   in-country collector over OTLP/HTTP when online, so the indicators are visible offline on the
//!   page and reach central once the link is back. Everything logged passes the telemetry
//!   allowlist; the report holds no personal health data.

mod buffer;
mod report;
mod ship;

use std::collections::BTreeSet;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::extract::{ConnectInfo, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use health_time::Timestamp;

use crate::backup::Backups;
use crate::supervisor::Restarts;
use crate::ups::PowerState;

pub use buffer::{Buffer, DEFAULT_CAP};
pub use report::{Alert, Backup, Node, Report, Task, Telemetry, Ups, Volume};
pub(crate) use ship::{ShipState, ship};

/// Where the report's facts come from.
#[derive(Clone)]
pub(crate) struct Monitor {
    pub(crate) node_id: String,
    pub(crate) started: Timestamp,
    pub(crate) data_dir: PathBuf,
    pub(crate) backups: Option<(Backups, Duration)>,
    pub(crate) power: Option<PowerState>,
    pub(crate) restarts: Restarts,
    pub(crate) buffer: Option<Arc<Buffer>>,
    pub(crate) shipping: ShipState,
}

impl Monitor {
    /// Measures and judges the node now.
    pub(crate) async fn report(&self) -> Report {
        let monitor = self.clone();
        let facts = tokio::task::spawn_blocking(move || monitor.facts()).await;
        match facts {
            Ok(facts) => report::assess(facts, Timestamp::now()),
            Err(_) => report::assess(self.facts_without_disk(), Timestamp::now()),
        }
    }

    /// The facts, reading the disk: blocking.
    fn facts(&self) -> report::Facts {
        let mut facts = self.facts_without_disk();
        facts.store_bytes = size_of(&self.data_dir);
        let backup_dir = self.backups.as_ref().map(|(b, _)| b.root().to_path_buf());
        for (name, path) in [
            ("data", Some(self.data_dir.clone())),
            ("backup", backup_dir),
        ] {
            if let Some(volume) = path.as_deref().and_then(|p| volume(name, p)) {
                facts.volumes.push(volume);
            }
        }
        if let Some((status, _)) = &mut facts.backup
            && let Some((backups, _)) = &self.backups
        {
            *status = backups.status();
        }
        facts
    }

    fn facts_without_disk(&self) -> report::Facts {
        report::Facts {
            node_id: self.node_id.clone(),
            version: env!("CARGO_PKG_VERSION"),
            started: self.started,
            store_bytes: None,
            volumes: Vec::new(),
            backup: self
                .backups
                .as_ref()
                .map(|(_, every)| (Default::default(), *every)),
            ups: self
                .power
                .as_ref()
                .map(|p| *p.lock().unwrap_or_else(|e| e.into_inner())),
            tasks: self.restarts.recent(),
            telemetry: ship::Shipping::telemetry(&self.shipping, self.buffer.as_deref()),
        }
    }

    /// Every `every`, logs the indicators, and each alert as it is raised and cleared.
    pub(crate) async fn watch(self, every: Duration) -> Result<(), String> {
        let mut raised = BTreeSet::new();
        loop {
            let report = self.report().await;
            log_indicators(&report);
            let now: BTreeSet<Alert> = report.alerts.iter().cloned().collect();
            for alert in now.difference(&raised) {
                tracing::warn!(
                    alert = alert.code,
                    check = alert.subject.unwrap_or("node"),
                    "alert raised"
                );
            }
            for alert in raised.difference(&now) {
                tracing::info!(
                    alert = alert.code,
                    check = alert.subject.unwrap_or("node"),
                    outcome = "cleared",
                    "alert cleared"
                );
            }
            raised = now;
            tokio::time::sleep(every).await;
        }
    }
}

/// One `indicators` line, carrying each measured indicator as an allowlisted field.
fn log_indicators(report: &Report) {
    let span = tracing::info_span!(
        "indicators",
        store_bytes = tracing::field::Empty,
        data_free_bytes = tracing::field::Empty,
        backup_free_bytes = tracing::field::Empty,
        backup_age_seconds = tracing::field::Empty,
        state = tracing::field::Empty,
        telemetry_buffered_bytes = report.telemetry.buffered_bytes,
        telemetry_dropped_bytes = report.telemetry.dropped_bytes,
    );
    if let Some(bytes) = report.store_bytes {
        span.record("store_bytes", bytes);
    }
    for volume in &report.volumes {
        let field = match volume.name {
            "data" => "data_free_bytes",
            _ => "backup_free_bytes",
        };
        span.record(field, volume.free_bytes);
    }
    if let Some(last) = report.backup.as_ref().and_then(|b| b.last_backup) {
        let age = report.at.duration_since(last).as_secs().max(0);
        span.record("backup_age_seconds", age);
    }
    if let Some(power) = report.ups.as_ref().and_then(|u| u.power) {
        span.record("state", power.as_str());
    }
    let restarts: usize = report.tasks.iter().map(|t| t.recent_failures).sum();
    span.in_scope(|| {
        tracing::info!(
            alert_count = report.alerts.len(),
            restart_count = restarts,
            "indicators"
        );
    });
}

/// The bytes under `dir`, or `None` if it cannot be read.
fn size_of(dir: &Path) -> Option<u64> {
    let mut total = 0;
    let mut pending = vec![dir.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir).ok()? {
            let entry = entry.ok()?;
            let meta = entry.metadata().ok()?;
            if meta.is_dir() {
                pending.push(entry.path());
            } else {
                total += meta.len();
            }
        }
    }
    Some(total)
}

/// The volume holding `path`, or `None` if it cannot be measured.
fn volume(name: &'static str, path: &Path) -> Option<Volume> {
    Some(Volume {
        name,
        total_bytes: fs4::total_space(path).ok()?,
        free_bytes: fs4::available_space(path).ok()?,
    })
}

/// `GET /api/admin/status`.
pub(crate) fn routes(monitor: Monitor) -> Router {
    Router::new()
        .route("/api/admin/status", get(status))
        .with_state(monitor)
}

async fn status(
    State(monitor): State<Monitor>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
) -> Response {
    if !local(peer) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let mut response = axum::Json(monitor.report().await).into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

/// Whether `peer` is this machine.
fn local(peer: SocketAddr) -> bool {
    match peer.ip() {
        std::net::IpAddr::V4(ip) => ip.is_loopback(),
        std::net::IpAddr::V6(ip) => {
            ip.is_loopback() || ip.to_ipv4_mapped().is_some_and(|v4| v4.is_loopback())
        }
    }
}

#[cfg(test)]
mod tests;

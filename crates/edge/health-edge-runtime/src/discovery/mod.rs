//! How LAN clients find and pin the edge (lan-security.md § Addressing and discovery; #74).
//!
//! - **Pairing is the primary path.** `GET /api/admin/pairing`, for the edge's own screen,
//!   gives what the admin page shows as a QR code and as text: the node's URL, its node ID and
//!   the SPKI fingerprint of its facility intermediate, which Tauri clients pin. Clients identify
//!   the node by node ID and fingerprint, never by address.
//! - **mDNS is only a hint.** While the node serves HTTPS it announces `_health-edge._tcp` with
//!   its node ID and protocol version, at the addresses its leaf names. Only Tauri apps browse it,
//!   and a spoofed announcement fails the fingerprint check.

use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::extract::State;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use mdns_sd::{ServiceDaemon, ServiceInfo};
use serde_json::json;

use crate::config::EdgeConfig;
use crate::tls::{CHAIN, CertificateState, ROOT};

/// The mDNS service type.
pub(crate) const SERVICE: &str = "_health-edge._tcp.local.";
/// The pairing and discovery protocol's version, in the QR code and the TXT record.
pub(crate) const PROTOCOL: &str = "1";

/// What the pairing payload is made from.
#[derive(Debug, Clone)]
pub(crate) struct Pairing {
    node_id: String,
    origins: Vec<String>,
    pki_dir: PathBuf,
}

impl Pairing {
    pub(crate) fn new(config: &EdgeConfig) -> Self {
        Self {
            node_id: config.node_id.clone(),
            origins: config.origins.clone(),
            pki_dir: config.pki_dir.clone(),
        }
    }

    /// The payload, or why there is none yet.
    fn payload(&self) -> Result<serde_json::Value, (&'static str, &'static str)> {
        let not_provisioned = (
            "not-provisioned",
            "this edge has no facility intermediate yet: provision it first",
        );
        if !self.pki_dir.join(ROOT).is_file() {
            return Err(not_provisioned);
        }
        let chain =
            std::fs::read_to_string(self.pki_dir.join(CHAIN)).map_err(|_| not_provisioned)?;
        let spki = health_pki::inspect(&chain)
            .map_err(|_| ("identity-invalid", "the installed chain cannot be read"))?
            .spki_sha256;
        // Devices sign their DPoP proofs for this origin, so it is one the node accepts.
        let url = self
            .origins
            .iter()
            .find(|o| o.starts_with("https://"))
            .ok_or(("no-https-origin", "HEALTH_ORIGINS names no https origin"))?;
        Ok(json!({
            "version": PROTOCOL,
            "node_id": self.node_id,
            "url": url,
            "spki": spki,
            "uri": uri(&self.node_id, url, &spki),
        }))
    }
}

/// The pairing URI the QR code carries: `health-edge://pair?v=…&node=…&url=…&spki=…`.
fn uri(node_id: &str, url: &str, spki: &str) -> String {
    format!(
        "health-edge://pair?v={PROTOCOL}&node={}&url={}&spki={}",
        encode(node_id),
        encode(url),
        encode(spki)
    )
}

/// Percent-encodes everything but RFC 3986's unreserved characters.
fn encode(value: &str) -> String {
    value
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                char::from(b).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// `GET /api/admin/pairing`, for the caller to guard.
pub(crate) fn routes(pairing: Pairing) -> Router {
    Router::new()
        .route("/api/admin/pairing", get(pairing_payload))
        .with_state(Arc::new(pairing))
}

async fn pairing_payload(State(pairing): State<Arc<Pairing>>) -> Response {
    let no_store = [(header::CACHE_CONTROL, HeaderValue::from_static("no-store"))];
    match pairing.payload() {
        Ok(payload) => (no_store, axum::Json(payload)).into_response(),
        Err((error, detail)) => (
            StatusCode::CONFLICT,
            no_store,
            axum::Json(json!({ "error": error, "detail": detail })),
        )
            .into_response(),
    }
}

/// The announcement for `node_id` on `port` at `addresses`; with none, at every interface's.
fn service(node_id: &str, addresses: &[IpAddr], port: u16) -> Result<ServiceInfo, String> {
    let properties = [("node", node_id), ("v", PROTOCOL)];
    let info = ServiceInfo::new(
        SERVICE,
        node_id,
        &format!("{node_id}.local."),
        addresses,
        port,
        &properties[..],
    )
    .map_err(|e| e.to_string())?;
    Ok(if addresses.is_empty() {
        info.enable_addr_auto()
    } else {
        info
    })
}

/// The mDNS responder, shut down when the task ends.
struct Responder(ServiceDaemon);

impl Drop for Responder {
    fn drop(&mut self) {
        let _ = self.0.shutdown();
    }
}

/// Announces the node while it runs, re-announcing every `every` when its leaf's addresses
/// change.
pub(crate) async fn announce(
    node_id: String,
    port: u16,
    certificate: CertificateState,
    every: Duration,
) -> Result<(), String> {
    let responder = Responder(ServiceDaemon::new().map_err(|e| e.to_string())?);
    let mut announced: Option<Vec<IpAddr>> = None;
    loop {
        let addresses = certificate
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .addresses
            .clone();
        if announced.as_ref() != Some(&addresses) {
            let info = service(&node_id, &addresses, port)?;
            if announced.is_some() {
                let _ = responder.0.unregister(info.get_fullname());
            }
            responder.0.register(info).map_err(|e| e.to_string())?;
            tracing::info!(
                outcome = "announced",
                service = SERVICE,
                "mDNS announcement made"
            );
            announced = Some(addresses);
        }
        tokio::time::sleep(every).await;
    }
}

#[cfg(test)]
mod tests;

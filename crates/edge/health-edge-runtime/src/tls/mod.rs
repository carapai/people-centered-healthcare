//! The edge's LAN TLS (lan-security.md § Certificate hierarchy, § Addressing and discovery; #74).
//!
//! - **The leaf.** Once the facility intermediate is installed (`facility-edge provision`), the
//!   node issues its own server certificate under it: ECDSA P-256, serverAuth, naming
//!   `<node-id>.local`, the configured facility DNS name and the node's private LAN addresses,
//!   valid 397 days. Its key is new at each issue and kept, readable by the service alone, in
//!   `HEALTH_TLS_DIR`; the intermediate's key stays in the keystore and is unsealed only to
//!   issue.
//! - **Renewal.** Every `HEALTH_CERTIFICATE_SECS` the leaf is reissued if two-thirds of its life
//!   has passed or the node's addresses changed (an `address-changed` alert). The new leaf is
//!   served from the next handshake on, with no restart.
//! - **Before provisioning** there is nothing to serve: the node opens no LAN listener, with a
//!   `not-provisioned` alert, so nothing on the LAN ever talks to it in the clear. Its
//!   plain-HTTP console on the loopback address (`HEALTH_CONSOLE_BIND`) answers either way.

mod listener;

use std::collections::BTreeSet;
use std::fmt;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use health_pki::{Authority, Inspected, Tier, inspect, new_key};
use health_time::{SignedDuration, Timestamp};
use rustls::crypto::CryptoProvider;
use rustls::server::{ClientHello, ResolvesServerCert};
use rustls::sign::CertifiedKey;
use rustls_pki_types::pem::PemObject;
use rustls_pki_types::{CertificateDer, PrivateKeyDer};
use serde::Serialize;

use crate::config::EdgeConfig;

pub(crate) use listener::TlsListener;

/// The installed national root and chain, as `facility-edge provision install` writes them.
pub(crate) const ROOT: &str = "root.pem";
pub(crate) const CHAIN: &str = "facility-chain.pem";
/// The leaf and its key, in `HEALTH_TLS_DIR`.
const LEAF: &str = "leaf.pem";
const LEAF_KEY: &str = "leaf.key";

/// The composition root's access to the facility intermediate's sealed key, as PEM; `None`
/// before provisioning. Blocking: it may run the platform keystore's tools.
pub(crate) type IntermediateKey = Arc<dyn Fn() -> Result<Option<String>, String> + Send + Sync>;

/// The leaf, as the status page shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Certificate {
    /// `serving`, `not-provisioned`, or `invalid` (installed, but nothing can be served).
    pub state: &'static str,
    /// Its DNS names.
    pub names: Vec<String>,
    /// Its addresses.
    pub addresses: Vec<IpAddr>,
    /// When it expires.
    pub expires: Option<Timestamp>,
    /// When it is due for renewal.
    pub renews: Option<Timestamp>,
    /// When the facility intermediate expires: re-provision well before.
    pub intermediate_expires: Option<Timestamp>,
    /// Why the last issue or renewal failed, until one succeeds.
    pub problem: Option<String>,
    /// When the node's addresses last changed, so its leaf was reissued.
    pub address_changed: Option<Timestamp>,
}

impl Certificate {
    /// Before the first check, and on a node with no intermediate installed.
    pub(crate) fn unprovisioned() -> Self {
        Self {
            state: "not-provisioned",
            names: Vec::new(),
            addresses: Vec::new(),
            expires: None,
            renews: None,
            intermediate_expires: None,
            problem: None,
            address_changed: None,
        }
    }
}

/// The leaf's state, for the status page.
pub(crate) type CertificateState = Arc<Mutex<Certificate>>;

/// The leaf served at each handshake; renewal swaps it.
pub(crate) struct Resolver(RwLock<Arc<CertifiedKey>>);

impl fmt::Debug for Resolver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Resolver")
    }
}

impl ResolvesServerCert for Resolver {
    fn resolve(&self, _: ClientHello<'_>) -> Option<Arc<CertifiedKey>> {
        Some(Arc::clone(
            &self.0.read().unwrap_or_else(|e| e.into_inner()),
        ))
    }
}

impl Resolver {
    fn swap(&self, key: Arc<CertifiedKey>) {
        *self.0.write().unwrap_or_else(|e| e.into_inner()) = key;
    }

    /// The rustls server configuration serving this resolver's leaf, HTTP/1.1 only.
    pub(crate) fn server_config(self: &Arc<Self>) -> Result<rustls::ServerConfig, String> {
        let mut config = rustls::ServerConfig::builder_with_provider(provider())
            .with_safe_default_protocol_versions()
            .map_err(|e| e.to_string())?
            .with_no_client_auth()
            .with_cert_resolver(Arc::clone(self) as Arc<dyn ResolvesServerCert>);
        config.alpn_protocols = vec![b"http/1.1".to_vec()];
        Ok(config)
    }
}

fn provider() -> Arc<CryptoProvider> {
    Arc::new(rustls::crypto::ring::default_provider())
}

/// A leaf on disk, read back.
struct Leaf {
    pem: String,
    key_pem: String,
    read: Inspected,
}

/// Why a leaf is (re)issued.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reason {
    /// There is none, or it does not load.
    Missing,
    /// Two-thirds of its life has passed.
    Due,
    /// The configured names changed.
    Names,
    /// The node's addresses changed.
    Addresses,
}

/// The node's names, addresses and keys: what its leaf is issued from.
pub(crate) struct Identity {
    pki_dir: PathBuf,
    tls_dir: PathBuf,
    names: Vec<String>,
    /// The bound address: when it is a specific private one, the leaf names it alone.
    bind: IpAddr,
    key: Option<IntermediateKey>,
}

impl Identity {
    pub(crate) fn new(config: &EdgeConfig, key: Option<IntermediateKey>) -> Self {
        let mut names = vec![format!("{}.local", config.node_id)];
        names.extend(config.dns_name.iter().cloned());
        Self {
            pki_dir: config.pki_dir.clone(),
            tls_dir: config.tls_dir.clone(),
            names,
            bind: config.bind.ip(),
            key,
        }
    }

    /// The installed chain (the intermediate, then the issuing CA); `None` before provisioning.
    fn chain(&self) -> Option<String> {
        if !self.pki_dir.join(ROOT).is_file() {
            return None;
        }
        std::fs::read_to_string(self.pki_dir.join(CHAIN)).ok()
    }

    /// The private LAN addresses the leaf names: the bound one if it is specific and private,
    /// else every private address on the node's interfaces.
    fn addresses(&self) -> Vec<IpAddr> {
        if private(self.bind) {
            return vec![self.bind];
        }
        let found: BTreeSet<IpAddr> = if_addrs::get_if_addrs()
            .unwrap_or_default()
            .into_iter()
            .map(|i| i.ip())
            .filter(|ip| private(*ip))
            .collect();
        found.into_iter().collect()
    }

    fn stored(&self) -> Option<Leaf> {
        let pem = std::fs::read_to_string(self.tls_dir.join(LEAF)).ok()?;
        let key_pem = std::fs::read_to_string(self.tls_dir.join(LEAF_KEY)).ok()?;
        let read = inspect(&pem).ok()?;
        Some(Leaf { pem, key_pem, read })
    }

    fn due(&self, leaf: &Leaf, addresses: &[IpAddr], now: Timestamp) -> Option<Reason> {
        if leaf.read.dns_names != self.names {
            return Some(Reason::Names);
        }
        let named: BTreeSet<_> = leaf.read.addresses.iter().collect();
        if named != addresses.iter().collect() {
            return Some(Reason::Addresses);
        }
        (now >= renews(&leaf.read)).then_some(Reason::Due)
    }

    /// Issues a leaf under the installed intermediate and stores it.
    fn issue(&self, chain: &str, addresses: &[IpAddr], now: Timestamp) -> Result<Leaf, String> {
        let unseal = self
            .key
            .as_ref()
            .ok_or("no access to the facility intermediate's key")?;
        let key = unseal()?.ok_or("the facility intermediate's key is not in the keystore")?;
        let intermediate = Authority::from_pem(chain, &key).map_err(|e| e.to_string())?;
        if intermediate.tier() != Tier::Facility {
            return Err("the installed chain does not start with a facility intermediate".into());
        }
        let leaf_key = new_key().map_err(|e| e.to_string())?;
        let issued = intermediate
            .issue_server(&leaf_key, &self.names, addresses, now)
            .map_err(|e| e.to_string())?;
        let leaf = Leaf {
            pem: issued.pem(),
            key_pem: leaf_key.serialize_pem(),
            read: inspect(&issued.pem()).map_err(|e| e.to_string())?,
        };
        std::fs::create_dir_all(&self.tls_dir)
            .map_err(|e| format!("cannot create {}: {e}", self.tls_dir.display()))?;
        // The key first: a leaf whose key is missing or another's does not load, and is
        // issued again.
        write(&self.tls_dir.join(LEAF_KEY), &leaf.key_pem, true)?;
        write(&self.tls_dir.join(LEAF), &leaf.pem, false)?;
        Ok(leaf)
    }

    /// Makes sure a current leaf is stored, issuing one if needed; the leaf to serve, and
    /// whether it is new. `state` records what happened.
    fn refresh(
        &self,
        state: &CertificateState,
        now: Timestamp,
    ) -> Result<(Arc<CertifiedKey>, bool), String> {
        let Some(chain) = self.chain() else {
            *lock(state) = Certificate::unprovisioned();
            return Err("not provisioned".into());
        };
        let intermediate_expires = inspect(&chain).ok().map(|i| i.not_after);
        let addresses = self.addresses();
        let stored = self.stored().filter(|leaf| certified(leaf, &chain).is_ok());
        let reason = match &stored {
            None => Some(Reason::Missing),
            Some(leaf) => self.due(leaf, &addresses, now),
        };
        let mut problem = None;
        let (leaf, issued) = match reason {
            None => (stored.ok_or("no leaf")?, false),
            Some(reason) => match self.issue(&chain, &addresses, now) {
                Ok(leaf) => {
                    tracing::info!(
                        outcome = "certificate-issued",
                        ?reason,
                        "server certificate issued"
                    );
                    if reason == Reason::Addresses {
                        tracing::warn!(
                            alert = "address-changed",
                            "the node's LAN addresses changed; its certificate was reissued"
                        );
                        lock(state).address_changed = Some(now);
                    }
                    (leaf, true)
                }
                Err(error) => {
                    tracing::error!(alert = "certificate-renewal-failed", error.kind = %error, "cannot issue the server certificate");
                    match stored.filter(|leaf| leaf.read.not_after > now) {
                        // Keep serving the current leaf while it is valid.
                        Some(leaf) => {
                            problem = Some(error);
                            (leaf, false)
                        }
                        None => {
                            let mut state = lock(state);
                            state.state = "invalid";
                            state.problem = Some(error.clone());
                            state.intermediate_expires = intermediate_expires;
                            return Err(error);
                        }
                    }
                }
            },
        };
        let key = certified(&leaf, &chain)?;
        let mut state = lock(state);
        state.state = "serving";
        state.names = leaf.read.dns_names.clone();
        state.addresses = leaf.read.addresses.clone();
        state.expires = Some(leaf.read.not_after);
        state.renews = Some(renews(&leaf.read));
        state.intermediate_expires = intermediate_expires;
        state.problem = problem;
        Ok((key, issued))
    }
}

/// When two-thirds of `leaf`'s life has passed.
fn renews(leaf: &Inspected) -> Timestamp {
    let life = leaf.not_after.as_second() - leaf.not_before.as_second();
    leaf.not_before
        .checked_add(SignedDuration::from_secs(life * 2 / 3))
        .unwrap_or(leaf.not_after)
}

/// The leaf, then the installed chain, with the leaf's key: refused if the key is not the
/// leaf's.
fn certified(leaf: &Leaf, chain: &str) -> Result<Arc<CertifiedKey>, String> {
    let mut certificates =
        vec![CertificateDer::from_pem_slice(leaf.pem.as_bytes()).map_err(|e| e.to_string())?];
    for certificate in CertificateDer::pem_slice_iter(chain.as_bytes()) {
        certificates.push(certificate.map_err(|e| e.to_string())?);
    }
    let key = PrivateKeyDer::from_pem_slice(leaf.key_pem.as_bytes()).map_err(|e| e.to_string())?;
    CertifiedKey::from_der(certificates, key, &provider())
        .map(Arc::new)
        .map_err(|e| e.to_string())
}

fn private(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => ip.is_private(),
        IpAddr::V6(ip) => ip.segments()[0] & 0xfe00 == 0xfc00,
    }
}

fn lock(state: &CertificateState) -> std::sync::MutexGuard<'_, Certificate> {
    state.lock().unwrap_or_else(|e| e.into_inner())
}

/// Writes `contents` to `path` through a temporary file; `private` makes it readable by the
/// service alone.
fn write(path: &Path, contents: &str, private: bool) -> Result<(), String> {
    let temporary = path.with_extension("new");
    let _ = std::fs::remove_file(&temporary);
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    if private {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    #[cfg(not(unix))]
    let _ = private;
    let written = options.open(&temporary).and_then(|mut file| {
        use std::io::Write;
        file.write_all(contents.as_bytes())?;
        file.sync_all()
    });
    written
        .and_then(|()| std::fs::rename(&temporary, path))
        .map_err(|e| format!("cannot write {}: {e}", path.display()))
}

/// How the node serves.
pub(crate) enum Listen {
    /// HTTPS on the configured address, with this leaf.
    Https(Arc<Resolver>),
    /// No LAN listener: the loopback console alone.
    ConsoleOnly,
}

/// At start: the leaf to serve, issued or renewed if needed, or the console alone when there
/// is none. Blocking.
pub(crate) fn prepare(identity: &Identity, state: &CertificateState) -> Listen {
    match identity.refresh(state, Timestamp::now()) {
        Ok((key, _)) => Listen::Https(Arc::new(Resolver(RwLock::new(key)))),
        Err(_) if lock(state).state == "not-provisioned" => {
            tracing::warn!(
                alert = "not-provisioned",
                "no facility intermediate: serving the loopback console only"
            );
            Listen::ConsoleOnly
        }
        Err(_) => {
            tracing::error!(
                alert = "identity-invalid",
                "no server certificate can be served: serving the loopback console only"
            );
            Listen::ConsoleOnly
        }
    }
}

/// Every `every`, renews the leaf when it is due or the addresses changed, and serves the new
/// one from the next handshake.
pub(crate) async fn watch(
    identity: Arc<Identity>,
    resolver: Arc<Resolver>,
    state: CertificateState,
    every: Duration,
) -> Result<(), String> {
    loop {
        tokio::time::sleep(every).await;
        let (identity, state) = (Arc::clone(&identity), Arc::clone(&state));
        let refreshed =
            tokio::task::spawn_blocking(move || identity.refresh(&state, Timestamp::now()))
                .await
                .map_err(|e| format!("the certificate check panicked: {e}"))?;
        // A failure is in the state and the log; the current leaf is kept while it is valid.
        if let Ok((key, true)) = refreshed {
            resolver.swap(key);
        }
    }
}

#[cfg(test)]
mod tests;

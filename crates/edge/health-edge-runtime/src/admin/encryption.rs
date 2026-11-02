//! The disk-encryption indicator (ADR 0048 § Monitoring). The check itself is the composition
//! root's (`health-node-identity`, which a runtime library may not depend on); the runtime runs
//! it on a schedule and reports what it found.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use health_time::Timestamp;
use serde::Serialize;

use super::Alert;

/// What the disk-encryption check found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Encryption {
    /// When the host was read.
    pub checked: Timestamp,
    /// Whether a TPM 2.0 is present; `None` if it could not be read.
    pub tpm2: Option<bool>,
    /// Whether Secure Boot is on; `None` if it could not be read.
    pub secure_boot: Option<bool>,
    /// The system volume and the data directory's volume.
    pub volumes: Vec<EncryptedVolume>,
    /// How the machine falls short, as alerts; empty when it complies.
    pub findings: Vec<Alert>,
    /// The country-configuration grant exempting this facility, if any.
    pub exemption: Option<String>,
}

/// One volume's protection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EncryptedVolume {
    /// `system` or `data`.
    pub name: &'static str,
    /// `tpm` (compliant), `suspended`, `no-tpm-protector`, `not-sealed`, `unencrypted` or
    /// `unknown`.
    pub protection: &'static str,
    /// Why it is `unknown`.
    pub reason: Option<String>,
}

/// The check: blocking, as it runs the platform's tools or reads their result.
pub type EncryptionCheck = Arc<dyn Fn() -> Encryption + Send + Sync>;

/// The last check's result; `None` before the first.
pub(crate) type EncryptionState = Arc<Mutex<Option<Encryption>>>;

/// Runs `check` every `every`, keeping its result in `state`.
pub(crate) async fn watch(
    check: EncryptionCheck,
    state: EncryptionState,
    every: Duration,
) -> Result<(), String> {
    loop {
        let check = Arc::clone(&check);
        let found = tokio::task::spawn_blocking(move || check())
            .await
            .map_err(|e| format!("the encryption check panicked: {e}"))?;
        *state.lock().unwrap_or_else(|e| e.into_inner()) = Some(found);
        tokio::time::sleep(every).await;
    }
}

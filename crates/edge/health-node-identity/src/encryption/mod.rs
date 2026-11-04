//! The full-disk encryption check (ADR 0048; facility-edge.md § Hardware and power baseline).
//!
//! An edge may be provisioned only when its system volume and its data directory's volume are
//! encrypted with TPM-only unlock, on a machine with TPM 2.0 and Secure Boot:
//! - **Windows:** BitLocker, fully encrypted, protection on, with a `Tpm` protector.
//! - **Linux:** LUKS2 with a `systemd-tpm2` token sealed to PCR 7 (the Secure Boot state).
//!
//! [`probe`] reads the host into an [`EncryptionState`]; [`EncryptionState::findings`] judges
//! it; [`gate`] decides provisioning, with an optional [`Exemption`]. Anything the probe cannot
//! read is [`Protection::Unknown`], which counts against the machine: the check fails closed.
//!
//! The probes need administrator rights (`cryptsetup luksDump`, `Get-BitLockerVolume`). The
//! Linux service runs unprivileged, so systemd runs `facility-edge encryption probe` as root
//! before each start and the service reads its state file.

mod linux;
pub mod recovery;
mod windows;

use std::path::{Path, PathBuf};

use health_time::Timestamp;
use serde::{Deserialize, Serialize};

pub use linux::{parse_lsblk, parse_luks_dump, parse_secure_boot_var};
pub use windows::{BitLocker, parse_bitlocker};

/// What the probe found on this host.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EncryptionState {
    /// When it was read.
    pub checked: Timestamp,
    /// Whether a TPM 2.0 is present; `None` if it could not be read.
    pub tpm2: Option<bool>,
    /// Whether UEFI Secure Boot is on; `None` if it could not be read.
    pub secure_boot: Option<bool>,
    /// The system volume, then the data directory's volume.
    pub volumes: Vec<Volume>,
}

/// One volume the node depends on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Volume {
    /// What it holds.
    pub role: Role,
    /// The path that was checked.
    pub path: PathBuf,
    /// How it is protected.
    pub protection: Protection,
}

/// What a volume holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Role {
    /// The operating system.
    System,
    /// The node's data directory.
    Data,
}

impl Role {
    /// The name alerts use as their subject.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Data => "data",
        }
    }
}

/// How a volume is protected.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum Protection {
    /// Encrypted (BitLocker fully encrypted, or LUKS2).
    Encrypted {
        /// Whether a TPM protector unlocks it unattended.
        tpm_protector: bool,
        /// Whether protection is suspended (BitLocker; LUKS has no suspension).
        suspended: bool,
        /// Whether the TPM unlock is sealed to the Secure Boot state (PCR 7); `None` where
        /// the platform binds it itself (BitLocker).
        sealed_to_secure_boot: Option<bool>,
    },
    /// Not encrypted.
    Unencrypted,
    /// It could not be determined; counts as unprotected.
    Unknown {
        /// What went wrong, for the operator.
        reason: String,
    },
}

/// One way the machine falls short of ADR 0048.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Finding {
    /// No TPM 2.0.
    NoTpm2,
    /// The TPM could not be read.
    TpmUnknown,
    /// Secure Boot is off.
    SecureBootOff,
    /// Secure Boot could not be read.
    SecureBootUnknown,
    /// A volume is not encrypted.
    Unencrypted(Role),
    /// A volume is encrypted without a TPM protector, so it cannot restart unattended.
    NoTpmProtector(Role),
    /// A volume's protection is suspended.
    Suspended(Role),
    /// A volume's TPM unlock is not sealed to PCR 7.
    NotSealedToSecureBoot(Role),
    /// A volume could not be read.
    Unknown(Role),
}

impl Finding {
    /// The alert code.
    pub fn code(self) -> &'static str {
        match self {
            Self::NoTpm2 => "tpm-missing",
            Self::TpmUnknown => "tpm-unknown",
            Self::SecureBootOff => "secure-boot-off",
            Self::SecureBootUnknown => "secure-boot-unknown",
            Self::Unencrypted(_) => "disk-unencrypted",
            Self::NoTpmProtector(_) => "tpm-protector-missing",
            Self::Suspended(_) => "encryption-suspended",
            Self::NotSealedToSecureBoot(_) => "tpm-not-sealed-to-secure-boot",
            Self::Unknown(_) => "encryption-unknown",
        }
    }

    /// The volume it concerns, if any.
    pub fn subject(self) -> Option<&'static str> {
        match self {
            Self::Unencrypted(r)
            | Self::NoTpmProtector(r)
            | Self::Suspended(r)
            | Self::NotSealedToSecureBoot(r)
            | Self::Unknown(r) => Some(r.as_str()),
            _ => None,
        }
    }
}

impl EncryptionState {
    /// Every way the machine falls short; empty when it complies.
    pub fn findings(&self) -> Vec<Finding> {
        let mut findings = Vec::new();
        match self.tpm2 {
            Some(true) => {}
            Some(false) => findings.push(Finding::NoTpm2),
            None => findings.push(Finding::TpmUnknown),
        }
        match self.secure_boot {
            Some(true) => {}
            Some(false) => findings.push(Finding::SecureBootOff),
            None => findings.push(Finding::SecureBootUnknown),
        }
        for volume in &self.volumes {
            let role = volume.role;
            match &volume.protection {
                Protection::Encrypted {
                    tpm_protector,
                    suspended,
                    sealed_to_secure_boot,
                } => {
                    if !tpm_protector {
                        findings.push(Finding::NoTpmProtector(role));
                    } else if *sealed_to_secure_boot == Some(false) {
                        findings.push(Finding::NotSealedToSecureBoot(role));
                    }
                    if *suspended {
                        findings.push(Finding::Suspended(role));
                    }
                }
                Protection::Unencrypted => findings.push(Finding::Unencrypted(role)),
                Protection::Unknown { .. } => findings.push(Finding::Unknown(role)),
            }
        }
        findings.dedup();
        findings
    }
}

/// A country-configuration grant letting one facility's edge run without full-disk
/// encryption (ADR 0048 § Exemption). It is audited, and stays a standing alert.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Exemption {
    /// The grant's reference in country configuration.
    pub grant: String,
}

/// The provisioning decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "decision", rename_all = "kebab-case")]
pub enum Gate {
    /// The machine complies.
    Allowed,
    /// It falls short, but an exemption covers it.
    Exempt {
        /// The exemption's grant.
        grant: String,
        /// What it falls short in.
        findings: Vec<Finding>,
    },
    /// Provisioning is refused.
    Refused {
        /// Why.
        findings: Vec<Finding>,
    },
}

/// Whether provisioning may go ahead on a machine in `state`.
pub fn gate(state: &EncryptionState, exemption: Option<&Exemption>) -> Gate {
    let findings = state.findings();
    match (findings.is_empty(), exemption) {
        (true, _) => Gate::Allowed,
        (false, Some(e)) => Gate::Exempt {
            grant: e.grant.clone(),
            findings,
        },
        (false, None) => Gate::Refused { findings },
    }
}

/// Reads the host: the TPM, Secure Boot, and the volumes holding `system` and `data`.
/// Blocking: it runs the platform's tools.
pub fn probe(system: &Path, data: &Path, now: Timestamp) -> EncryptionState {
    let volumes = |protect: &dyn Fn(&Path) -> Protection| {
        [(Role::System, system), (Role::Data, data)]
            .into_iter()
            .map(|(role, path)| Volume {
                role,
                path: path.to_path_buf(),
                protection: protect(path),
            })
            .collect()
    };
    if cfg!(target_os = "linux") {
        EncryptionState {
            checked: now,
            tpm2: linux::tpm2(),
            secure_boot: linux::secure_boot(),
            volumes: volumes(&linux::protection),
        }
    } else if cfg!(windows) {
        windows::probe(&[(Role::System, system), (Role::Data, data)], now)
    } else {
        let reason = || Protection::Unknown {
            reason: "full-disk encryption is checked on Linux and Windows only".into(),
        };
        EncryptionState {
            checked: now,
            tpm2: None,
            secure_boot: None,
            volumes: volumes(&|_| reason()),
        }
    }
}

/// The system volume's root: `/`, or `%SystemDrive%\` on Windows.
pub fn system_root() -> PathBuf {
    if cfg!(windows) {
        let drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
        PathBuf::from(format!("{drive}\\"))
    } else {
        PathBuf::from("/")
    }
}

/// Runs `program` with `args`, returning its stdout if it succeeded.
pub(super) fn run(program: &str, args: &[&str]) -> Result<String, String> {
    let output = std::process::Command::new(program)
        .args(args)
        .output()
        .map_err(|e| format!("{program} could not run: {e}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!(
            "{program} failed ({}): {}",
            output.status,
            stderr.trim()
        ))
    }
}

//! `facility-edge encryption …` and the running node's disk-encryption check (ADR 0048), over
//! `health_node_identity::encryption`.
//!
//! - `encryption probe [--out FILE]`: read the host and print its state as JSON, or write it to
//!   `FILE` (replacing it). The Linux unit runs this as root before each start, because the
//!   service itself runs unprivileged.
//! - `encryption check`: the provisioning gate. It prints the decision, and fails unless the
//!   machine complies or `HEALTH_ENCRYPTION_EXEMPTION` exempts it.
//!
//! The running node checks every `HEALTH_ENCRYPTION_SECS`. It reads
//! `HEALTH_ENCRYPTION_STATE_FILE` when set, and otherwise probes the host itself.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use health_edge_runtime::EdgeConfig;
use health_edge_runtime::admin::{Alert, EncryptedVolume, Encryption, EncryptionCheck};
use health_node_identity::encryption::{
    EncryptionState, Exemption, Gate, Protection, Role, Volume, gate, probe, system_root,
};
use health_time::Timestamp;

const USAGE: &str = "usage: facility-edge encryption (probe [--out FILE] | check)";

/// Runs `args` (after `encryption`); the error is printed and becomes exit code 10.
pub(crate) fn run(config: &EdgeConfig, args: &[String]) -> Result<(), String> {
    match args {
        [cmd] if cmd == "probe" => say(&json(&read_host(config))),
        [cmd, flag, out] if cmd == "probe" && flag == "--out" => {
            let out = PathBuf::from(out);
            let temporary = out.with_extension("json.new");
            std::fs::write(&temporary, json(&read_host(config)))
                .and_then(|()| std::fs::rename(&temporary, &out))
                .map_err(|e| format!("cannot write {}: {e}", out.display()))
        }
        [cmd] if cmd == "check" => {
            let state = read_host(config);
            let decision = gate(&state, exemption(config).as_ref());
            say(&serde_json::json!({ "state": state, "gate": decision }).to_string())?;
            match decision {
                Gate::Allowed => Ok(()),
                Gate::Exempt { grant, .. } => {
                    eprintln!("facility-edge: exempt from full-disk encryption by {grant}");
                    Ok(())
                }
                Gate::Refused { findings } => Err(format!(
                    "provisioning refused: {}",
                    findings
                        .iter()
                        .map(|f| match f.subject() {
                            Some(subject) => format!("{} ({subject})", f.code()),
                            None => f.code().to_owned(),
                        })
                        .collect::<Vec<_>>()
                        .join(", ")
                )),
            }
        }
        _ => Err(USAGE.into()),
    }
}

/// The running node's check: the privileged probe's file when configured, else the host.
pub(crate) fn check(config: &EdgeConfig) -> EncryptionCheck {
    let config = config.clone();
    Arc::new(move || {
        let state = match &config.encryption_state_file {
            Some(file) => read_file(file, &config.data_dir),
            None => read_host(&config),
        };
        summarize(&state, exemption(&config))
    })
}

fn read_host(config: &EdgeConfig) -> EncryptionState {
    probe(&system_root(), &config.data_dir, Timestamp::now())
}

/// The probe's file; anything wrong with it is an unknown state, never a pass.
fn read_file(file: &Path, data_dir: &Path) -> EncryptionState {
    match std::fs::read_to_string(file)
        .map_err(|e| e.to_string())
        .and_then(|text| serde_json::from_str::<EncryptionState>(&text).map_err(|e| e.to_string()))
    {
        Ok(state) => state,
        Err(error) => {
            let reason = format!("the probe's file {} is unusable: {error}", file.display());
            EncryptionState {
                checked: Timestamp::now(),
                tpm2: None,
                secure_boot: None,
                volumes: [
                    (Role::System, system_root()),
                    (Role::Data, data_dir.to_path_buf()),
                ]
                .into_iter()
                .map(|(role, path)| Volume {
                    role,
                    path,
                    protection: Protection::Unknown {
                        reason: reason.clone(),
                    },
                })
                .collect(),
            }
        }
    }
}

fn exemption(config: &EdgeConfig) -> Option<Exemption> {
    config
        .encryption_exemption
        .clone()
        .map(|grant| Exemption { grant })
}

/// The state, judged, in the runtime's terms.
pub(crate) fn summarize(state: &EncryptionState, exemption: Option<Exemption>) -> Encryption {
    Encryption {
        checked: state.checked,
        tpm2: state.tpm2,
        secure_boot: state.secure_boot,
        volumes: state
            .volumes
            .iter()
            .map(|v| {
                let (protection, reason) = match &v.protection {
                    Protection::Encrypted {
                        tpm_protector: false,
                        ..
                    } => ("no-tpm-protector", None),
                    Protection::Encrypted {
                        suspended: true, ..
                    } => ("suspended", None),
                    Protection::Encrypted {
                        sealed_to_secure_boot: Some(false),
                        ..
                    } => ("not-sealed", None),
                    Protection::Encrypted { .. } => ("tpm", None),
                    Protection::Unencrypted => ("unencrypted", None),
                    Protection::Unknown { reason } => ("unknown", Some(reason.clone())),
                };
                EncryptedVolume {
                    name: v.role.as_str(),
                    protection,
                    reason,
                }
            })
            .collect(),
        findings: state
            .findings()
            .into_iter()
            .map(|f| Alert {
                code: f.code(),
                subject: f.subject(),
            })
            .collect(),
        exemption: exemption.map(|e| e.grant),
    }
}

fn json(state: &EncryptionState) -> String {
    serde_json::to_string_pretty(state).unwrap_or_default()
}

fn say(text: &str) -> Result<(), String> {
    use std::io::Write as _;
    writeln!(std::io::stdout(), "{text}").map_err(|e| e.to_string())
}

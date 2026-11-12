//! The desktop's full-disk encryption check (desktop-client.md § Storage and backup; ADR 0040).
//! SurrealDB has no encryption at rest, so the system volume and the data directory's must be
//! encrypted by the OS (BitLocker or LUKS). Unlike an edge (ADR 0048), no TPM or Secure Boot is
//! required. Whether a shortfall blocks the client or only warns is the country's choice: until
//! country configuration carries it, `HEALTH_DESKTOP_ENCRYPTION` is `block` (the default) or
//! `warn`.

use health_node_identity::encryption::{EncryptionState, Protection};
use serde::Serialize;

/// What a shortfall does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Policy {
    /// The client does not start.
    Block,
    /// The client starts, and shows the shortfall.
    Warn,
}

impl Policy {
    /// From `HEALTH_DESKTOP_ENCRYPTION`: `warn`, or anything else (and unset) is `block`.
    pub(crate) fn from_setting(setting: Option<&str>) -> Self {
        match setting.map(str::trim) {
            Some("warn") => Self::Warn,
            _ => Self::Block,
        }
    }
}

/// The check's outcome, as `GET /api/desktop/encryption` reports it to the app.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct Report {
    /// What a shortfall does here.
    pub(crate) policy: Policy,
    /// Whether every volume the client depends on is encrypted.
    pub(crate) encrypted: bool,
    /// The volumes that are not, or could not be read, as `system` or `data` with why.
    pub(crate) shortfalls: Vec<String>,
    /// What the probe found.
    pub(crate) state: EncryptionState,
}

impl Report {
    /// The outcome for `state` under `policy`.
    pub(crate) fn of(state: EncryptionState, policy: Policy) -> Self {
        let shortfalls: Vec<String> = state
            .volumes
            .iter()
            .filter_map(|volume| {
                let role = match volume.role {
                    health_node_identity::encryption::Role::System => "system",
                    health_node_identity::encryption::Role::Data => "data",
                };
                match &volume.protection {
                    Protection::Encrypted {
                        suspended: false, ..
                    } => None,
                    Protection::Encrypted {
                        suspended: true, ..
                    } => Some(format!("{role}: encryption is suspended")),
                    Protection::Unencrypted => Some(format!("{role}: not encrypted")),
                    Protection::Unknown { reason } => Some(format!("{role}: unknown ({reason})")),
                }
            })
            .collect();
        Self {
            policy,
            encrypted: shortfalls.is_empty(),
            shortfalls,
            state,
        }
    }

    /// Whether the client may start.
    pub(crate) fn may_start(&self) -> bool {
        self.encrypted || self.policy == Policy::Warn
    }
}

#[cfg(test)]
mod tests {
    use health_node_identity::encryption::{Role, Volume};

    use super::*;

    fn state(system: Protection, data: Protection) -> EncryptionState {
        EncryptionState {
            checked: health_time::Timestamp::now(),
            // A desktop needs neither: only the volumes count.
            tpm2: Some(false),
            secure_boot: None,
            volumes: vec![
                Volume {
                    role: Role::System,
                    path: "/".into(),
                    protection: system,
                },
                Volume {
                    role: Role::Data,
                    path: "/var/lib/health-desktop".into(),
                    protection: data,
                },
            ],
        }
    }

    fn encrypted() -> Protection {
        Protection::Encrypted {
            tpm_protector: false,
            suspended: false,
            sealed_to_secure_boot: None,
        }
    }

    #[test]
    fn encrypted_volumes_start_with_or_without_a_tpm() {
        let report = Report::of(state(encrypted(), encrypted()), Policy::Block);
        assert!(report.encrypted && report.may_start(), "{report:?}");
    }

    #[test]
    fn a_shortfall_blocks_by_default_and_only_warns_when_the_country_says_so() {
        let unencrypted = || state(encrypted(), Protection::Unencrypted);
        let blocked = Report::of(unencrypted(), Policy::from_setting(None));
        assert!(!blocked.may_start());
        assert_eq!(blocked.shortfalls, ["data: not encrypted"]);
        let warned = Report::of(unencrypted(), Policy::from_setting(Some("warn")));
        assert!(warned.may_start() && !warned.encrypted);
        // Anything but `warn` blocks.
        assert_eq!(Policy::from_setting(Some("off")), Policy::Block);
        let unknown = Protection::Unknown {
            reason: "checked on Linux and Windows only".into(),
        };
        let suspended = Protection::Encrypted {
            tpm_protector: true,
            suspended: true,
            sealed_to_secure_boot: Some(true),
        };
        let report = Report::of(state(suspended, unknown), Policy::Block);
        assert_eq!(
            report.shortfalls,
            [
                "system: encryption is suspended",
                "data: unknown (checked on Linux and Windows only)"
            ]
        );
    }
}

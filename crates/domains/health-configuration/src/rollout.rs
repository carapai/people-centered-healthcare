//! Staged software roll-out (deployment.md § Environments and rollout; ADRs 0041, 0045): which
//! platform release each node may install, set like package activations.
//!
//! A [`Rollout`] allows one release in a [`Scope`] from an effective time. Records are never
//! changed: a later one in the same scope replaces it, and holding a wave back is a new record of
//! the version it should stay on. Every node is sent every record and works out its own allowed
//! release along its [`Chain`] ([`allowed`]): the most specific scope wins, and within a scope the
//! latest issued record already in effect. Program scopes do not apply: software is per node.
//!
//! A [`RolloutRecord`] carries the release's [`Artifact`]s, each with its sha-256 and signature,
//! so a node fetches its own by digest from upstream and verifies it before staging. [`offer`]
//! says what a node running one version should do: a node never moves to an older release,
//! because migrations only go forward (facility-edge.md § Updates and migrations).

use std::fmt;

use health_time::Timestamp;
use serde::{Deserialize, Serialize};

use crate::activation::{Chain, Scope};
use crate::package::Version;

/// What an artifact is built for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Target {
    /// The Facility Edge Node's update package for Linux x64, signed with the release-signing key.
    #[serde(rename = "edge-linux-x64")]
    EdgeLinux,
    /// The edge's update package for Windows x64.
    #[serde(rename = "edge-windows-x64")]
    EdgeWindows,
    /// The desktop's NSIS installer for Windows x64, signed with the updater key.
    #[serde(rename = "desktop-windows-x64")]
    DesktopWindows,
    /// The desktop's AppImage for Linux x64, signed with the updater key.
    #[serde(rename = "desktop-linux-x64")]
    DesktopLinux,
}

impl Target {
    /// Every target, in order.
    pub const ALL: [Self; 4] = [
        Self::EdgeLinux,
        Self::EdgeWindows,
        Self::DesktopWindows,
        Self::DesktopLinux,
    ];

    /// Its name, as records carry it.
    pub fn name(self) -> &'static str {
        match self {
            Self::EdgeLinux => "edge-linux-x64",
            Self::EdgeWindows => "edge-windows-x64",
            Self::DesktopWindows => "desktop-windows-x64",
            Self::DesktopLinux => "desktop-linux-x64",
        }
    }
}

impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// One of a release's files, as a node fetches and checks it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Artifact {
    /// What it is for.
    pub target: Target,
    /// Its file name in the release.
    pub file: String,
    /// Its sha-256, lowercase hex: the blob a node fetches it by.
    pub sha256: String,
    /// Its size in bytes.
    pub size: u64,
    /// Its signature: hex `r || s` from the release-signing key for an edge package, the
    /// updater signature (minisign, base64) for a desktop artifact.
    pub signature: String,
}

/// A record allowing release `version` in `scope` from `effective_at`. Never changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rollout {
    /// Its id.
    pub id: String,
    /// The platform release allowed.
    pub version: Version,
    /// Where.
    pub scope: Scope,
    /// From when.
    pub effective_at: Timestamp,
    /// When it was issued: the later of two in one scope wins.
    pub issued_at: Timestamp,
    /// Who issued it.
    pub by: String,
    /// Why, e.g. the wave, or the reason for holding one back.
    #[serde(default)]
    pub reason: Option<String>,
}

/// A roll-out as it travels to nodes: the record, with the release's artifacts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RolloutRecord {
    /// The roll-out.
    pub rollout: Rollout,
    /// The release's files, one per target at most.
    pub artifacts: Vec<Artifact>,
}

impl RolloutRecord {
    /// The artifact for `target`, if the release has one.
    pub fn artifact(&self, target: Target) -> Option<&Artifact> {
        self.artifacts.iter().find(|a| a.target == target)
    }
}

/// Why a roll-out cannot be issued.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Refused {
    /// Software is per node, not per program.
    #[error("a roll-out applies nationally, to a district or to a facility, not to a program")]
    ProgramScope,
    /// The release names a target twice, or an artifact's digest is not sha-256 hex.
    #[error("the release's artifacts are malformed: {0}")]
    Artifacts(String),
}

/// Checks a roll-out before it is issued.
pub fn check(record: &RolloutRecord) -> Result<(), Refused> {
    if matches!(record.rollout.scope, Scope::Program { .. }) {
        return Err(Refused::ProgramScope);
    }
    for (i, artifact) in record.artifacts.iter().enumerate() {
        if record.artifacts[..i]
            .iter()
            .any(|a| a.target == artifact.target)
        {
            return Err(Refused::Artifacts(format!("{} twice", artifact.target)));
        }
        let hex = artifact.sha256.len() == 64
            && artifact
                .sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
        if !hex {
            return Err(Refused::Artifacts(format!(
                "{}'s sha-256 is not lowercase hex",
                artifact.target
            )));
        }
    }
    Ok(())
}

fn rank(scope: &Scope) -> u8 {
    match scope {
        Scope::National => 0,
        Scope::District { .. } => 1,
        Scope::Facility { .. } | Scope::Program { .. } => 2,
    }
}

/// The record that sets the release a node on `chain` may run at `now`: the most specific
/// scope's latest issued record in effect. `None` before any applies.
pub fn allowed<'a>(
    records: &'a [RolloutRecord],
    chain: &Chain,
    now: Timestamp,
) -> Option<&'a RolloutRecord> {
    records
        .iter()
        .filter(|r| {
            let rollout = &r.rollout;
            rollout.effective_at <= now
                && !matches!(rollout.scope, Scope::Program { .. })
                && chain.covers(&rollout.scope)
        })
        .max_by(|a, b| {
            let key = |r: &RolloutRecord| (rank(&r.rollout.scope), r.rollout.issued_at);
            key(a)
                .cmp(&key(b))
                .then_with(|| a.rollout.id.cmp(&b.rollout.id))
        })
}

/// What a node running `running` should do, given its allowed record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Offer<'a> {
    /// Fetch, verify and stage this artifact of the allowed, newer release.
    Update {
        /// The release.
        version: &'a Version,
        /// Its artifact for the node's target.
        artifact: &'a Artifact,
    },
    /// The node runs the allowed release, or nothing allows one yet.
    Current,
    /// The allowed release is older than the node's: it stays, since migrations only go
    /// forward. Reported, so the roll-out can be corrected.
    Held {
        /// The allowed release.
        allowed: &'a Version,
    },
    /// The allowed, newer release has no artifact for the node's target. Reported.
    Missing {
        /// The release.
        version: &'a Version,
    },
}

/// What a node running `running` for `target` should do under `record`.
pub fn offer<'a>(
    record: Option<&'a RolloutRecord>,
    target: Target,
    running: &Version,
) -> Offer<'a> {
    let Some(record) = record else {
        return Offer::Current;
    };
    let version = &record.rollout.version;
    if version == running {
        return Offer::Current;
    }
    if version < running {
        return Offer::Held { allowed: version };
    }
    match record.artifact(target) {
        Some(artifact) => Offer::Update { version, artifact },
        None => Offer::Missing { version },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(text: &str) -> Timestamp {
        text.parse().expect("timestamp")
    }

    fn version(text: &str) -> Version {
        text.parse().expect("version")
    }

    fn artifact(target: Target) -> Artifact {
        Artifact {
            target,
            file: format!("{target}.bin"),
            sha256: "ab".repeat(32),
            size: 10,
            signature: "sig".into(),
        }
    }

    fn record(id: &str, v: &str, scope: Scope, effective: &str, issued: &str) -> RolloutRecord {
        RolloutRecord {
            rollout: Rollout {
                id: id.into(),
                version: version(v),
                scope,
                effective_at: at(effective),
                issued_at: at(issued),
                by: "release-manager".into(),
                reason: None,
            },
            artifacts: Target::ALL.into_iter().map(artifact).collect(),
        }
    }

    fn chain(district: &str, facility: &str) -> Chain {
        Chain {
            district: Some(district.into()),
            facility: Some(facility.into()),
            ..Chain::default()
        }
    }

    fn district(name: &str) -> Scope {
        Scope::District {
            district: name.into(),
        }
    }

    fn facility(code: &str) -> Scope {
        Scope::Facility {
            facility: code.into(),
        }
    }

    #[test]
    fn a_wave_reaches_its_district_and_a_facility_can_be_held_back() {
        let records = vec![
            record(
                "a",
                "1.0.0",
                Scope::National,
                "2026-11-01T00:00:00Z",
                "2026-11-01T00:00:00Z",
            ),
            // The pilot district gets 1.1.0 from the 10th.
            record(
                "b",
                "1.1.0",
                district("kampala"),
                "2026-11-10T00:00:00Z",
                "2026-11-09T00:00:00Z",
            ),
            // One of its facilities is held on 1.0.0.
            record(
                "c",
                "1.0.0",
                facility("KAM013"),
                "2026-11-10T00:00:00Z",
                "2026-11-09T12:00:00Z",
            ),
        ];
        let now = at("2026-11-12T00:00:00Z");
        let version_for =
            |c: &Chain, now| allowed(&records, c, now).map(|r| r.rollout.version.to_string());
        assert_eq!(
            version_for(&chain("kampala", "KAM001"), now).as_deref(),
            Some("1.1.0")
        );
        assert_eq!(
            version_for(&chain("kampala", "KAM013"), now).as_deref(),
            Some("1.0.0")
        );
        assert_eq!(
            version_for(&chain("gulu", "GUL002"), now).as_deref(),
            Some("1.0.0")
        );
        // Not before it takes effect.
        let before = at("2026-11-09T18:00:00Z");
        assert_eq!(
            version_for(&chain("kampala", "KAM001"), before).as_deref(),
            Some("1.0.0")
        );
        // Nothing before the first record.
        assert_eq!(
            version_for(&chain("gulu", "GUL002"), at("2026-10-01T00:00:00Z")),
            None
        );
    }

    #[test]
    fn within_a_scope_the_latest_issued_record_wins() {
        let records = vec![
            record(
                "b",
                "1.2.0",
                Scope::National,
                "2026-11-01T00:00:00Z",
                "2026-11-02T00:00:00Z",
            ),
            // Issued later, effective earlier: the national wave is pulled back to 1.1.0.
            record(
                "a",
                "1.1.0",
                Scope::National,
                "2026-11-01T00:00:00Z",
                "2026-11-03T00:00:00Z",
            ),
        ];
        let winner = allowed(
            &records,
            &chain("gulu", "GUL002"),
            at("2026-11-04T00:00:00Z"),
        );
        assert_eq!(winner.map(|r| r.rollout.id.as_str()), Some("a"));
    }

    #[test]
    fn a_node_updates_forward_only() {
        let record = record(
            "a",
            "1.1.0",
            Scope::National,
            "2026-11-01T00:00:00Z",
            "2026-11-01T00:00:00Z",
        );
        assert_eq!(
            offer(Some(&record), Target::DesktopWindows, &version("1.0.0")),
            Offer::Update {
                version: &version("1.1.0"),
                artifact: &artifact(Target::DesktopWindows)
            }
        );
        assert_eq!(
            offer(Some(&record), Target::EdgeLinux, &version("1.1.0")),
            Offer::Current
        );
        assert_eq!(
            offer(Some(&record), Target::EdgeLinux, &version("1.2.0")),
            Offer::Held {
                allowed: &version("1.1.0")
            }
        );
        assert_eq!(
            offer(None, Target::EdgeLinux, &version("1.0.0")),
            Offer::Current
        );
        let mut no_windows = record.clone();
        no_windows
            .artifacts
            .retain(|a| a.target != Target::EdgeWindows);
        assert_eq!(
            offer(Some(&no_windows), Target::EdgeWindows, &version("1.0.0")),
            Offer::Missing {
                version: &version("1.1.0")
            }
        );
    }

    #[test]
    fn program_scopes_and_malformed_artifacts_are_refused() {
        let mut program = record(
            "a",
            "1.1.0",
            Scope::National,
            "2026-11-01T00:00:00Z",
            "2026-11-01T00:00:00Z",
        );
        program.rollout.scope = Scope::Program {
            facility: "KAM013".into(),
            program: "hiv".into(),
        };
        assert_eq!(check(&program), Err(Refused::ProgramScope));
        assert_eq!(
            allowed(
                std::slice::from_ref(&program),
                &Chain::default(),
                at("2026-11-02T00:00:00Z")
            ),
            None
        );

        let mut twice = record(
            "a",
            "1.1.0",
            Scope::National,
            "2026-11-01T00:00:00Z",
            "2026-11-01T00:00:00Z",
        );
        twice.artifacts.push(artifact(Target::EdgeLinux));
        assert!(matches!(check(&twice), Err(Refused::Artifacts(_))));
        let mut upper = record(
            "a",
            "1.1.0",
            Scope::National,
            "2026-11-01T00:00:00Z",
            "2026-11-01T00:00:00Z",
        );
        upper.artifacts[0].sha256 = "AB".repeat(32);
        assert!(matches!(check(&upper), Err(Refused::Artifacts(_))));
        assert_eq!(
            check(&record(
                "a",
                "1.1.0",
                district("gulu"),
                "2026-11-01T00:00:00Z",
                "2026-11-01T00:00:00Z"
            )),
            Ok(())
        );
    }

    #[test]
    fn records_read_back_as_they_travel() {
        let record = record(
            "a",
            "1.1.0",
            district("gulu"),
            "2026-11-01T00:00:00Z",
            "2026-11-01T00:00:00Z",
        );
        let json = serde_json::to_value(&record).expect("json");
        assert_eq!(json["rollout"]["scope"]["level"], "district");
        assert_eq!(json["artifacts"][2]["target"], "desktop-windows-x64");
        assert_eq!(json["rollout"]["effectiveAt"], "2026-11-01T00:00:00Z");
        let back: RolloutRecord = serde_json::from_value(json).expect("back");
        assert_eq!(back, record);
    }
}

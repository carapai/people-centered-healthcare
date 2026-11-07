use std::collections::BTreeSet;
use std::fmt;
use std::str::FromStr;

use health_identifiers::NodeId;
use serde::{Deserialize, Serialize};

use crate::PackageRef;

/// A protocol version. Majors must match; a minor adds only what an older peer may ignore.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ProtocolVersion {
    /// Breaking changes.
    pub major: u16,
    /// Compatible additions.
    pub minor: u16,
}

/// This crate's protocol: `sync/v1`.
pub const PROTOCOL: ProtocolVersion = ProtocolVersion { major: 1, minor: 0 };

/// A node's software release, `major.minor.patch`. A release is its `major.minor`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SoftwareVersion {
    /// Major.
    pub major: u32,
    /// Minor.
    pub minor: u32,
    /// Patch.
    pub patch: u32,
}

/// Text that is not `major.minor.patch`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("`{0}` is not a major.minor.patch version")]
pub struct SoftwareVersionError(pub String);

impl SoftwareVersion {
    /// Whether `self` is this release or the one before it (N-1), on the same major.
    pub fn supports(self, peer: SoftwareVersion) -> bool {
        self.major == peer.major && (peer.minor == self.minor || peer.minor + 1 == self.minor)
    }
}

impl FromStr for SoftwareVersion {
    type Err = SoftwareVersionError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let invalid = || SoftwareVersionError(text.to_owned());
        let mut parts = text
            .split('.')
            .map(|p| p.parse::<u32>().map_err(|_| invalid()));
        let version = Self {
            major: parts.next().ok_or_else(invalid)??,
            minor: parts.next().ok_or_else(invalid)??,
            patch: parts.next().ok_or_else(invalid)??,
        };
        if parts.next().is_some() {
            return Err(invalid());
        }
        Ok(version)
    }
}

impl fmt::Display for SoftwareVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl TryFrom<String> for SoftwareVersion {
    type Error = SoftwareVersionError;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        text.parse()
    }
}

impl From<SoftwareVersion> for String {
    fn from(version: SoftwareVersion) -> Self {
        version.to_string()
    }
}

/// A position in one node's change log: `(node_id, epoch, seq)`. Opaque to the peer that holds
/// it, which only sends it back.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Cursor {
    /// Whose change log.
    pub node: NodeId,
    /// The log's epoch; it changes when the node is restored or re-seeded.
    pub epoch: String,
    /// The last `seq` seen.
    pub seq: u64,
}

/// The downstream node's opening message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hello {
    /// Its protocol version.
    pub protocol: ProtocolVersion,
    /// Its software release.
    pub software: SoftwareVersion,
    /// Who it is; the TLS certificate must name the same node.
    pub node: NodeId,
    /// Its change log's current epoch.
    pub epoch: String,
    /// The metadata and configuration packages it has active.
    pub packages: Vec<PackageRef>,
    /// The permission snapshot it holds.
    pub snapshot_version: u64,
    /// Where it has read the upstream's log to, if anywhere.
    pub cursor: Option<Cursor>,
}

/// What the upstream supports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Compatibility {
    /// Its protocol.
    pub protocol: ProtocolVersion,
    /// Its software release.
    pub software: SoftwareVersion,
    /// Every package version it has approved.
    pub approved: BTreeSet<PackageRef>,
}

/// Why a peer is out of range.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum Incompatibility {
    /// The protocol's major differs.
    Protocol {
        /// The upstream's.
        ours: ProtocolVersion,
        /// The peer's.
        theirs: ProtocolVersion,
    },
    /// The peer's software is neither this release nor the one before.
    Software {
        /// The upstream's.
        ours: SoftwareVersion,
        /// The peer's.
        theirs: SoftwareVersion,
    },
    /// The peer has a package version the upstream has not approved.
    Package {
        /// The package.
        package: PackageRef,
    },
}

/// The upstream's judgement of a [`Hello`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "verdict", rename_all = "snake_case")]
pub enum Verdict {
    /// Push and pull.
    Compatible,
    /// The peer may push its outbox, which the upstream holds for review; it is sent no data
    /// until it upgrades.
    Quarantined {
        /// Every reason.
        reasons: Vec<Incompatibility>,
    },
    /// The session ends: the protocols cannot understand each other.
    Refused {
        /// Why.
        reason: Incompatibility,
    },
}

impl Compatibility {
    /// Judges a peer: a different protocol major is refused; software outside N-1 or an unapproved
    /// package is quarantined.
    pub fn judge(&self, hello: &Hello) -> Verdict {
        if hello.protocol.major != self.protocol.major {
            return Verdict::Refused {
                reason: Incompatibility::Protocol {
                    ours: self.protocol,
                    theirs: hello.protocol,
                },
            };
        }
        let mut reasons = Vec::new();
        if !self.software.supports(hello.software) {
            reasons.push(Incompatibility::Software {
                ours: self.software,
                theirs: hello.software,
            });
        }
        reasons.extend(
            hello
                .packages
                .iter()
                .filter(|p| !self.approved.contains(*p))
                .map(|p| Incompatibility::Package { package: p.clone() }),
        );
        if reasons.is_empty() {
            Verdict::Compatible
        } else {
            Verdict::Quarantined { reasons }
        }
    }
}

/// The upstream's answer to a [`Hello`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Welcome {
    /// The upstream: whose log the peer pulls, and the peer its outbox is queued for.
    pub node: NodeId,
    /// Whether the session goes ahead.
    pub verdict: Verdict,
    /// The version of the peer's Sync Scope definition.
    pub scope_version: u64,
    /// The upstream's permission snapshot; newer than the peer's means one comes down first.
    pub snapshot_version: u64,
    /// Where the upstream has read the peer's log to. `None` means it holds nothing for the
    /// peer's current epoch, so the peer pushes everything still in its outbox.
    pub expected: Option<Cursor>,
}

/// The checkpoint a side holds for a peer's log, if it is still valid: it belongs to that peer
/// and that peer's current epoch. A new epoch (restore, re-seed) discards it, so a `seq` that went
/// backwards is never misread.
pub fn expected_cursor(saved: Option<Cursor>, node: NodeId, epoch: &str) -> Option<Cursor> {
    saved.filter(|cursor| cursor.node == node && cursor.epoch == epoch)
}

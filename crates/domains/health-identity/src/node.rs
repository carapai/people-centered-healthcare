//! Nodes and their provisioning at central (identifiers.md; lan-security.md; ADRs 0048, 0058).
//!
//! An operator issues a **one-time provisioning code** for one facility. The installer enters it on
//! the new edge, which sends it to central with its certificate request. Central redeems the code
//! once, mints the node's id and short code, and has the issuing CA issue the facility
//! intermediate for it.
//!
//! - **Codes** are 8 Crockford base32 characters (40 bits), shown as `XXXX-XXXX`. Central keeps
//!   only their hash. A code expires 72 hours after it is issued, and redeems once.
//! - **Encryption** (ADR 0048): an edge must report full-disk encryption, unless the operator
//!   granted the facility an exemption when issuing the code, with a reason. An exempt node stands
//!   as an alert at central.
//! - **Short codes** are `E1`, `E2`, … for edges, unique within the facility and never reused: a
//!   decommissioned node keeps its record (identifiers.md § Never recycled).
//! - **Recovery keys** (ADR 0048): each encrypted volume's disk recovery key is escrowed at
//!   central, wrapped, never kept on site. An operator reads one out by phone to a facility admin
//!   whose identity they verified, and every read is recorded. A key that has been read is
//!   **used**, and stays an alert until the edge escrows its replacement. The edge signs each
//!   escrow; a signing time no later than the current key's is refused, so an old escrow cannot
//!   be replayed to bring back a key that was read out.
//! - **Decommissioning** (ADR 0048 § Decommissioning): the machine is crypto-erased, by clearing
//!   its TPM or destroying its key slots, and then reformatted. Central records how, by whom and
//!   why, with the node's revocation. A decommissioned node keeps its record and short code, and
//!   is never active again.

use std::fmt;

use health_time::{SignedDuration, Timestamp};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A node's id: a UUIDv7 minted by central at provisioning, never reused (identifiers.md). A
/// re-provisioned machine gets a new one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NodeId(pub Uuid);

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// The Crockford base32 alphabet: no I, L, O or U.
const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// Characters in a provisioning code.
pub const CODE_LENGTH: usize = 8;

/// The provisioning rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProvisioningPolicy {
    /// How long a code stays redeemable.
    pub lifetime: SignedDuration,
}

impl Default for ProvisioningPolicy {
    /// 72 hours: time to carry a code to a facility.
    fn default() -> Self {
        Self {
            lifetime: SignedDuration::from_hours(72),
        }
    }
}

/// How an edge's disks are protected.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "state")]
pub enum Encryption {
    /// Full-disk encryption with TPM-only unlock, as ADR 0048 requires.
    Compliant,
    /// Not encrypted, under an exemption granted for this facility.
    Exempt {
        /// Why it was granted.
        reason: String,
    },
}

/// Why a code was not redeemed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CodeRefused {
    /// Unknown, mistyped, or not a code at all.
    #[error("no such provisioning code")]
    Unknown,
    /// Past its lifetime.
    #[error("the provisioning code has expired")]
    Expired,
    /// Already redeemed.
    #[error("the provisioning code has been used")]
    Used,
    /// The edge is not encrypted and the facility has no exemption.
    #[error("the edge's disks are not encrypted, and the facility has no exemption")]
    NotEncrypted,
}

impl ProvisioningPolicy {
    /// The code for 5 random bytes, `XXXX-XXXX`.
    pub fn code(&self, draw: [u8; 5]) -> String {
        let bits = draw.iter().fold(0u64, |acc, b| (acc << 8) | u64::from(*b));
        let chars: String = (0..CODE_LENGTH)
            .rev()
            .map(|i| char::from(ALPHABET[((bits >> (i * 5)) & 31) as usize]))
            .collect();
        format!("{}-{}", &chars[..4], &chars[4..])
    }

    /// `input` as a code is stored: upper case, without separators, with the letters Crockford
    /// reads as digits (`I`, `L` as 1; `O` as 0) mapped. `None` if it cannot be a code.
    pub fn normalise(&self, input: &str) -> Option<String> {
        let code: String = input
            .chars()
            .filter(|c| !matches!(c, '-' | ' '))
            .map(|c| match c.to_ascii_uppercase() {
                'I' | 'L' => '1',
                'O' => '0',
                c => c,
            })
            .collect();
        (code.len() == CODE_LENGTH && code.bytes().all(|b| ALPHABET.contains(&b))).then_some(code)
    }

    /// A code for `facility`, issued by `issued_by` at `now`; `exemption` exempts its node from
    /// the encryption requirement, with the reason given.
    pub fn issue(
        &self,
        id: Uuid,
        facility: String,
        dns_names: Vec<String>,
        exemption: Option<String>,
        issued_by: String,
        now: Timestamp,
    ) -> ProvisioningCode {
        ProvisioningCode {
            id,
            facility,
            dns_names,
            exemption,
            issued_by,
            issued_at: now,
            expires: now.checked_add(self.lifetime).unwrap_or(now),
            redeemed: None,
        }
    }
}

/// A one-time provisioning code, as central keeps it (without the code itself).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvisioningCode {
    /// The code's id.
    pub id: Uuid,
    /// The facility the node will serve.
    pub facility: String,
    /// The facility's DNS names, for the intermediate's name constraints.
    pub dns_names: Vec<String>,
    /// The reason for an encryption exemption, if one was granted.
    pub exemption: Option<String>,
    /// The operator who issued it.
    pub issued_by: String,
    /// When.
    pub issued_at: Timestamp,
    /// When it stops being redeemable.
    pub expires: Timestamp,
    /// When it was redeemed, and for which node.
    pub redeemed: Option<(Timestamp, NodeId)>,
}

impl ProvisioningCode {
    /// Whether the code may be redeemed at `now` by an edge whose disks are `encrypted`: the
    /// node's encryption state if so.
    pub fn redeemable(&self, encrypted: bool, now: Timestamp) -> Result<Encryption, CodeRefused> {
        if self.redeemed.is_some() {
            return Err(CodeRefused::Used);
        }
        if now >= self.expires {
            return Err(CodeRefused::Expired);
        }
        match (encrypted, &self.exemption) {
            (true, _) => Ok(Encryption::Compliant),
            (false, Some(reason)) => Ok(Encryption::Exempt {
                reason: reason.clone(),
            }),
            (false, None) => Err(CodeRefused::NotEncrypted),
        }
    }
}

/// What kind of node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NodeKind {
    /// A Facility Edge Node: short codes `E1`, `E2`, …
    Edge,
}

impl NodeKind {
    fn prefix(self) -> char {
        match self {
            Self::Edge => 'E',
        }
    }

    /// The next short code for this kind at a facility whose nodes have `existing` short codes:
    /// one past the highest, so none is ever reused.
    pub fn next_short_code<'a>(self, existing: impl IntoIterator<Item = &'a str>) -> String {
        let prefix = self.prefix();
        let highest = existing
            .into_iter()
            .filter_map(|code| code.strip_prefix(prefix))
            .filter_map(|n| n.parse::<u32>().ok())
            .max()
            .unwrap_or(0);
        format!("{prefix}{}", highest + 1)
    }
}

/// Whether a node is in service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NodeStatus {
    /// Provisioned and trusted.
    Active,
    /// Crypto-erased and revoked (ADR 0048 § Decommissioning).
    Decommissioned,
}

/// A provisioned node, in central's registry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Node {
    /// Its id.
    pub id: NodeId,
    /// Its facility.
    pub facility: String,
    /// What it is.
    pub kind: NodeKind,
    /// Its short code, unique within the facility.
    pub short_code: String,
    /// Whether it is in service.
    pub status: NodeStatus,
    /// Its disks' protection.
    pub encryption: Encryption,
    /// The serial of its facility intermediate, to revoke it by.
    pub intermediate_serial: String,
    /// The facility intermediate's public key (SEC1, base64url): what the node's signed
    /// requests to central, such as escrowing a recovery key, verify against.
    #[serde(default)]
    pub intermediate_key: String,
    /// The code it was provisioned with.
    pub code: Uuid,
    /// When.
    pub provisioned_at: Timestamp,
    /// Its decommissioning, once it is decommissioned.
    #[serde(default)]
    pub decommissioned: Option<Decommission>,
}

/// How a decommissioned machine's disks were made unreadable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Erasure {
    /// The TPM was cleared, so its sealed volume keys are gone.
    TpmCleared,
    /// The encrypted volumes' key slots were destroyed.
    KeySlotsDestroyed,
}

impl Erasure {
    /// Its name: `tpm-cleared` or `key-slots-destroyed`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TpmCleared => "tpm-cleared",
            Self::KeySlotsDestroyed => "key-slots-destroyed",
        }
    }
}

/// A node's decommissioning.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Decommission {
    /// When.
    pub at: Timestamp,
    /// The central operations person who recorded it.
    pub by: String,
    /// How its disks were erased.
    pub erasure: Erasure,
    /// Why.
    pub reason: String,
}

/// Why a node was not decommissioned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DecommissionRefused {
    /// It already is.
    #[error("the node is already decommissioned")]
    Already,
    /// No one said who, or why.
    #[error("a decommission needs who records it and why")]
    Incomplete,
}

impl Node {
    /// Decommissions the node at `now`, by `by`, its disks erased by `erasure`, for `reason`.
    pub fn decommission(
        &mut self,
        by: &str,
        erasure: Erasure,
        reason: &str,
        now: Timestamp,
    ) -> Result<(), DecommissionRefused> {
        if self.status == NodeStatus::Decommissioned {
            return Err(DecommissionRefused::Already);
        }
        if by.trim().is_empty() || reason.trim().is_empty() {
            return Err(DecommissionRefused::Incomplete);
        }
        self.status = NodeStatus::Decommissioned;
        self.decommissioned = Some(Decommission {
            at: now,
            by: by.to_owned(),
            erasure,
            reason: reason.to_owned(),
        });
        Ok(())
    }
}

/// A volume whose recovery key is escrowed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RecoveryVolume {
    /// The operating system's volume.
    System,
    /// The volume holding the node's data directory, when it is another one.
    Data,
}

impl RecoveryVolume {
    /// Its name: `system` or `data`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Data => "data",
        }
    }
}

/// One read-out of a recovery key, by phone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Retrieval {
    /// When.
    pub at: Timestamp,
    /// The central operations person who read it.
    pub by: String,
    /// How the facility admin's identity was verified against the device registry.
    pub verified: String,
}

/// An escrowed recovery key, without the key: the wrapped key is the store's.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryKey {
    /// The node.
    pub node: NodeId,
    /// Which volume.
    pub volume: RecoveryVolume,
    /// When the node signed the escrow.
    pub signed_at: Timestamp,
    /// When central stored it.
    pub escrowed_at: Timestamp,
    /// Every read-out.
    pub retrievals: Vec<Retrieval>,
}

/// Why an escrow was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum EscrowRefused {
    /// Signed no later than the key it would replace: a replay, or out of order.
    #[error("the escrow is older than the key it would replace")]
    Stale,
    /// Signed too far from central's clock.
    #[error("the escrow's signing time is too far from central's clock")]
    Clock,
}

/// How far an escrow's signing time may be from central's clock.
pub const ESCROW_SKEW: SignedDuration = SignedDuration::from_mins(5);

impl RecoveryKey {
    /// Whether it has been read out, so the edge must rotate it.
    pub fn used(&self) -> bool {
        !self.retrievals.is_empty()
    }

    /// The key a node escrows for `volume`, signed at `signed_at`, replacing `current`, at `now`.
    pub fn escrow(
        current: Option<&Self>,
        node: NodeId,
        volume: RecoveryVolume,
        signed_at: Timestamp,
        now: Timestamp,
    ) -> Result<Self, EscrowRefused> {
        let early = now.checked_sub(ESCROW_SKEW).unwrap_or(now);
        let late = now.checked_add(ESCROW_SKEW).unwrap_or(now);
        if signed_at < early || signed_at > late {
            return Err(EscrowRefused::Clock);
        }
        if current.is_some_and(|key| signed_at <= key.signed_at) {
            return Err(EscrowRefused::Stale);
        }
        Ok(Self {
            node,
            volume,
            signed_at,
            escrowed_at: now,
            retrievals: Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(text: &str) -> Timestamp {
        text.parse().expect("timestamp")
    }

    #[test]
    fn codes_are_crockford_base32_and_read_forgivingly() {
        let policy = ProvisioningPolicy::default();
        assert_eq!(policy.code([0; 5]), "0000-0000");
        assert_eq!(policy.code([0xff; 5]), "ZZZZ-ZZZZ");
        let code = policy.code([0x12, 0x34, 0x56, 0x78, 0x9a]);
        assert_eq!(code, "28T5-CY4T");
        assert_eq!(policy.normalise(&code).as_deref(), Some("28T5CY4T"));
        assert_eq!(policy.normalise("28t5 cy4t").as_deref(), Some("28T5CY4T"));
        // Read as Crockford does: I and L are 1, O is 0.
        assert_eq!(policy.normalise("oooo-iiil").as_deref(), Some("00001111"));
        for wrong in ["28T5-CY4", "28T5-CY4TT", "28T5-CY4U", ""] {
            assert_eq!(policy.normalise(wrong), None, "{wrong}");
        }
    }

    #[test]
    fn a_code_redeems_once_before_it_expires_and_only_for_an_encrypted_edge_unless_exempt() {
        let policy = ProvisioningPolicy::default();
        let now = at("2026-11-03T09:00:00Z");
        let code = policy.issue(
            Uuid::nil(),
            "ug-kla-mulago".into(),
            vec!["mulago.health.go.ug".into()],
            None,
            "ops:auma".into(),
            now,
        );
        assert_eq!(code.expires, at("2026-11-06T09:00:00Z"));
        assert_eq!(code.redeemable(true, now), Ok(Encryption::Compliant));
        assert_eq!(code.redeemable(false, now), Err(CodeRefused::NotEncrypted));
        assert_eq!(
            code.redeemable(true, at("2026-11-06T09:00:00Z")),
            Err(CodeRefused::Expired)
        );
        let mut used = code.clone();
        used.redeemed = Some((now, NodeId(Uuid::max())));
        assert_eq!(used.redeemable(true, now), Err(CodeRefused::Used));

        let mut exempt = code;
        exempt.exemption = Some("TPM-less PC pending replacement".into());
        assert_eq!(
            exempt.redeemable(false, now),
            Ok(Encryption::Exempt {
                reason: "TPM-less PC pending replacement".into()
            })
        );
        assert_eq!(exempt.redeemable(true, now), Ok(Encryption::Compliant));
    }

    #[test]
    fn an_escrow_must_be_fresh_and_newer_than_the_key_it_replaces() {
        let now = at("2026-11-04T09:00:00Z");
        let node = NodeId(Uuid::nil());
        let first = RecoveryKey::escrow(None, node, RecoveryVolume::System, now, now)
            .expect("first escrow");
        assert!(!first.used());
        // A replay of the same escrow, or an older one, cannot bring back a key.
        assert_eq!(
            RecoveryKey::escrow(Some(&first), node, RecoveryVolume::System, now, now),
            Err(EscrowRefused::Stale)
        );
        let later = at("2026-11-04T09:01:00Z");
        let rotated = RecoveryKey::escrow(Some(&first), node, RecoveryVolume::System, later, later)
            .expect("rotated");
        assert_eq!(rotated.signed_at, later);
        for skewed in ["2026-11-04T08:54:00Z", "2026-11-04T09:06:00Z"] {
            assert_eq!(
                RecoveryKey::escrow(None, node, RecoveryVolume::Data, at(skewed), now),
                Err(EscrowRefused::Clock),
                "{skewed}"
            );
        }
        let mut read = first;
        read.retrievals.push(Retrieval {
            at: now,
            by: "ops:okello".into(),
            verified: "called back on the registry number".into(),
        });
        assert!(read.used());
        assert_eq!(RecoveryVolume::Data.as_str(), "data");
    }

    #[test]
    fn a_node_is_decommissioned_once_with_who_how_and_why() {
        let now = at("2026-11-04T09:00:00Z");
        let mut node = Node {
            id: NodeId(Uuid::nil()),
            facility: "ug-kla-mulago".into(),
            kind: NodeKind::Edge,
            short_code: "E1".into(),
            status: NodeStatus::Active,
            encryption: Encryption::Compliant,
            intermediate_serial: "01".into(),
            intermediate_key: String::new(),
            code: Uuid::nil(),
            provisioned_at: now,
            decommissioned: None,
        };
        assert_eq!(
            node.decommission(" ", Erasure::TpmCleared, "replaced", now),
            Err(DecommissionRefused::Incomplete)
        );
        assert_eq!(node.status, NodeStatus::Active);
        node.decommission("ops:okello", Erasure::TpmCleared, "hardware replaced", now)
            .expect("decommissioned");
        assert_eq!(node.status, NodeStatus::Decommissioned);
        assert_eq!(
            node.decommissioned
                .as_ref()
                .map(|d| (d.erasure, d.by.as_str())),
            Some((Erasure::TpmCleared, "ops:okello"))
        );
        assert_eq!(
            node.decommission("ops:okello", Erasure::KeySlotsDestroyed, "again", now),
            Err(DecommissionRefused::Already)
        );
        assert_eq!(Erasure::KeySlotsDestroyed.as_str(), "key-slots-destroyed");
    }

    #[test]
    fn short_codes_count_up_and_are_never_reused() {
        assert_eq!(NodeKind::Edge.next_short_code([]), "E1");
        assert_eq!(NodeKind::Edge.next_short_code(["E1", "E2"]), "E3");
        // A decommissioned E2 keeps its code: the next is E4, not E2.
        assert_eq!(NodeKind::Edge.next_short_code(["E1", "E3", "D07"]), "E4");
        assert_eq!(NodeKind::Edge.next_short_code(["E9", "E10"]), "E11");
    }
}

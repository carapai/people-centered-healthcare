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
    /// The code it was provisioned with.
    pub code: Uuid,
    /// When.
    pub provisioned_at: Timestamp,
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
    fn short_codes_count_up_and_are_never_reused() {
        assert_eq!(NodeKind::Edge.next_short_code([]), "E1");
        assert_eq!(NodeKind::Edge.next_short_code(["E1", "E2"]), "E3");
        // A decommissioned E2 keeps its code: the next is E4, not E2.
        assert_eq!(NodeKind::Edge.next_short_code(["E1", "E3", "D07"]), "E4");
        assert_eq!(NodeKind::Edge.next_short_code(["E9", "E10"]), "E11");
    }
}

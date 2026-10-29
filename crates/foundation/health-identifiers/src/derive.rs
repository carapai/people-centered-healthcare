use std::fmt;

use uuid::Uuid;

use crate::ResourceId;

/// The fixed namespace for every derived id (ADR 0013). Changing it would fork every derived id
/// across the deployment, so it never changes.
pub const PLATFORM_NAMESPACE: Uuid = Uuid::from_u128(0x83664680_8814_416c_a4bb_2cc772956887);

/// The name a derived id is computed from (identifiers.md § Deterministic ids for derived records).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivationName(String);

impl DerivationName {
    /// A resource extracted from a QuestionnaireResponse:
    /// `{response id}|{mapping canonical, no version}|{target linkId path}#{repeat key}|{type}`.
    ///
    /// `repeat_key` is the stable `QuestionnaireResponse.item.id` of the repeat instance (or the
    /// repeat index for imported responses without item ids); `None` for a non-repeating target.
    /// Any `|version` on the mapping canonical is dropped, so re-extracting with a newer mapping
    /// produces a new version of the same resource.
    pub fn extraction(
        response: ResourceId,
        mapping_canonical: &str,
        link_path: &str,
        repeat_key: Option<&str>,
        target_type: &str,
    ) -> Self {
        let mapping = unversioned(mapping_canonical);
        let path = match repeat_key {
            Some(key) => format!("{link_path}#{key}"),
            None => link_path.to_owned(),
        };
        Self(format!("{response}|{mapping}|{path}|{target_type}"))
    }

    /// Any other derivation: `{source record id or op_id}|{derivation rule canonical, no version}`.
    pub fn rule(source: impl fmt::Display, rule_canonical: &str) -> Self {
        Self(format!("{source}|{}", unversioned(rule_canonical)))
    }

    /// The name as hashed.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DerivationName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// `UUIDv5(PLATFORM_NAMESPACE, name)`: the same on every node and every platform.
pub fn derive_id(name: &DerivationName) -> ResourceId {
    let uuid = Uuid::new_v5(&PLATFORM_NAMESPACE, name.0.as_bytes());
    // A v5 UUID always satisfies ResourceId's V7OrV5 rule.
    ResourceId::from_uuid(uuid).unwrap_or_else(|_| unreachable!("UUIDv5 is a valid ResourceId"))
}

fn unversioned(canonical: &str) -> &str {
    canonical.split_once('|').map_or(canonical, |(url, _)| url)
}

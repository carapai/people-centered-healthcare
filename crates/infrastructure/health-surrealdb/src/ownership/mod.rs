//! Which Owner each table and record belongs to (resource-ownership.md, ADR 0004).
//!
//! Every resource type has one head table, a default Owner and optional category partitions.
//! Rule 2 of the ownership matrix splits some types by category: a laboratory Observation
//! belongs to laboratory, an imaging one to imaging, and every other one to clinical-data.

mod generated;

use serde_json::Value;

/// A category coding that moves matching records to another Owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Partition {
    /// The coding system.
    pub system: &'static str,
    /// The code.
    pub code: &'static str,
    /// The Owner of records carrying this category.
    pub owner: &'static str,
}

/// The ownership of one resource type and its tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceOwnership {
    /// The FHIR resource type.
    pub resource_type: &'static str,
    /// Its head table; the history table is `{table}_history`.
    pub table: &'static str,
    /// The Owner of records no partition matches.
    pub owner: &'static str,
    /// Category partitions, disjoint by `system|code`.
    pub partitions: &'static [Partition],
}

/// The Owner of the unit of work's own tables.
pub const UNIT_OF_WORK: &str = "unit-of-work";

/// Infrastructure tables, written only through the unit of work (persistence.md). The search
/// index tables hold entries derived from each Owner's records, written in that Owner's
/// transaction; no module writes them directly.
pub const INFRASTRUCTURE_TABLES: &[&str] = &[
    "_migration",
    "guard",
    "counter",
    "outbox",
    "idx_token",
    "idx_string",
    "idx_date",
    "idx_reference",
    "idx_quantity",
    "idx_number",
    "idx_uri",
    "idx_composite",
    "idx_missing",
    "reindex_job",
    "condition_lock",
    "idx_ref_in",
    "idx_block",
    "block_count",
];

/// Tables a module owns that hold no FHIR resources: `(table, owner)`.
pub const MODULE_TABLES: &[(&str, &str)] = &[
    // The Inbound Dead Letter and quarantine queue (ADR 0051).
    ("inbound", "integration"),
    // The client registry's review queue.
    ("registry_review", "client-registry"),
    // The authorization server's state (health-auth).
    ("auth_account", "identity"),
    ("auth_code", "identity"),
    ("auth_session", "identity"),
    ("auth_retired_refresh", "identity"),
    ("auth_jti", "identity"),
    ("auth_snapshot", "identity"),
    ("auth_clock_floor", "identity"),
    ("auth_phone_account", "identity"),
    ("auth_otp", "identity"),
    ("caregiver_grant", "consent"),
    ("break_glass", "identity"),
    // Stock (health-inventory).
    ("stock_balance", "inventory"),
    ("stock_movement", "inventory"),
];

/// Every owned resource type, sorted by type.
pub fn resources() -> &'static [ResourceOwnership] {
    generated::RESOURCES
}

/// The ownership entry for a resource type, if the platform stores it.
pub fn resource(resource_type: &str) -> Option<&'static ResourceOwnership> {
    generated::RESOURCES
        .iter()
        .find(|r| r.resource_type == resource_type)
}

/// The Owner of one record: the first partition whose category coding it carries, otherwise
/// the type's default Owner. `None` for a type the platform does not store.
pub fn owner_of(resource_type: &str, record: &Value) -> Option<&'static str> {
    let entry = resource(resource_type)?;
    let codings = record
        .get("category")
        .into_iter()
        .flat_map(|c| {
            c.as_array()
                .map_or_else(|| std::slice::from_ref(c), Vec::as_slice)
        })
        .filter_map(|concept| concept.get("coding")?.as_array())
        .flatten();
    for coding in codings {
        let (system, code) = (
            coding.get("system").and_then(Value::as_str),
            coding.get("code").and_then(Value::as_str),
        );
        if let Some(p) = entry
            .partitions
            .iter()
            .find(|p| Some(p.system) == system && Some(p.code) == code)
        {
            return Some(p.owner);
        }
    }
    Some(entry.owner)
}

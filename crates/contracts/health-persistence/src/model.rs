use std::fmt;
use std::str::FromStr;

use health_identifiers::{OpId, ResourceId};
use health_time::Timestamp;

use crate::RawJson;

/// A FHIR resource type name, e.g. `Observation`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ResourceType(String);

/// A resource type name that is not `[A-Z][A-Za-z]*`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("`{0}` is not a resource type name")]
pub struct ResourceTypeError(pub String);

impl ResourceType {
    /// The name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for ResourceType {
    type Err = ResourceTypeError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let mut chars = text.chars();
        let valid = chars.next().is_some_and(|c| c.is_ascii_uppercase())
            && chars.all(|c| c.is_ascii_alphabetic());
        if valid {
            Ok(Self(text.to_owned()))
        } else {
            Err(ResourceTypeError(text.to_owned()))
        }
    }
}

impl fmt::Display for ResourceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The version of a stored resource (persistence.md § Tables per resource type).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VersionMeta {
    /// `meta.versionId`: the `op_id` of the write that produced this version (ADR 0018).
    pub version_id: OpId,
    /// Monotonic per resource, assigned by the Authority Node; starts at 1.
    pub version_seq: u64,
    /// `meta.lastUpdated`.
    pub last_updated: Timestamp,
    /// Whether this version is a tombstone.
    pub deleted: bool,
}

/// A resource head as stored: the stamped JSON and its version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredResource {
    /// The resource type.
    pub resource_type: ResourceType,
    /// The resource id.
    pub id: ResourceId,
    /// Its current version.
    pub version: VersionMeta,
    /// The stored bytes, served as they are. Tombstones keep their last body.
    pub json: RawJson,
}

/// What a write expects of the current head.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Precondition {
    /// The resource must not exist yet (create, including update-as-create with a client UUID).
    MustNotExist,
    /// The head's `versionId` must equal this one (`If-Match`), required on our clients' updates.
    IfMatch(OpId),
    /// No expectation; only for flows that resolve conflicts by other means, such as sync apply.
    None,
}

/// One resource version to write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteRequest {
    /// The resource type.
    pub resource_type: ResourceType,
    /// The resource id, client-generated (ADR 0013).
    pub id: ResourceId,
    /// Validated input JSON; the store stamps `id` and `meta` (see [`crate::stamp_meta`]).
    pub json: RawJson,
    /// The write's operation id, which becomes `meta.versionId`.
    pub op_id: OpId,
    /// What the head must look like.
    pub precondition: Precondition,
}

/// A key that at most one record may hold, claimed in the same transaction as the record, e.g.
/// `mrn|KLA01|KLA01-E1-42` or `open-encounter|{patient}|{service point}`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GuardKey(pub String);

/// A node-local counter, e.g. `mrn|KLA01|E1`; its value advances by
/// [`health_identifiers::SequenceState::next`].
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CounterKey(pub String);

/// A row appended to the transactional outbox, read after commit by in-process subscribers and
/// the sync uploader.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutboxEntry {
    /// The operation this entry belongs to.
    pub op_id: OpId,
    /// What it carries.
    pub payload: OutboxPayload,
}

/// The kinds of outbox row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutboxPayload {
    /// A resource version written in this transaction, to be synced upstream.
    ResourceVersion {
        /// The resource type.
        resource_type: ResourceType,
        /// The resource id.
        id: ResourceId,
        /// The version written.
        version_id: OpId,
    },
    /// A domain event returned by a domain function, as named JSON.
    DomainEvent {
        /// The event name, e.g. `clinical-records.encounter-completed.v1`.
        name: String,
        /// The event body.
        body: RawJson,
    },
}

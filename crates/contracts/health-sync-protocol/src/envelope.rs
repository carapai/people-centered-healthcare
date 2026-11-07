use health_identifiers::{DeviceId, NodeId, OpId, ResourceId};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{ContentHash, Hlc};

/// What an item carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// A FHIR resource version.
    Version,
    /// A ledger operation (stock, charges, payments), always appended.
    Op,
}

/// A metadata or configuration package version an item was validated against.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PackageRef {
    /// The package name.
    pub name: String,
    /// Its version.
    pub version: String,
}

/// An attachment an item references (ADR 0021).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BlobRef {
    /// The sha-256 of its bytes.
    pub hash: ContentHash,
    /// Its size in bytes.
    pub size: u64,
    /// Its media type.
    pub content_type: String,
}

/// One item on the wire, in either direction (synchronization.md § Envelope).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Item {
    /// A version or an operation.
    pub kind: Kind,
    /// The idempotency key: `meta.versionId` for a version (ADR 0018).
    pub op_id: OpId,
    /// The transaction it was committed in. A group of items with one `tx_id` is applied
    /// all-or-nothing.
    pub tx_id: OpId,
    /// The resource type for a version; the ledger for an operation.
    pub resource_type: String,
    /// The resource id for a version; the ledger's scope for an operation.
    pub resource_id: String,
    /// The version the writer saw; absent on create and on every operation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_version_id: Option<OpId>,
    /// The node that first committed it.
    pub origin_node: NodeId,
    /// The device it was written on, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_id: Option<DeviceId>,
    /// Who wrote it, as a Practitioner or Device reference.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// For ordering and audit only.
    pub hlc: Hlc,
    /// The hash of [`Item::body`].
    pub content_hash: ContentHash,
    /// The package versions it was validated against.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub packages: Vec<PackageRef>,
    /// The permission snapshot in force when it was written (ADR 0011).
    pub snapshot_version: u64,
    /// A provisional Patient or an unconfirmed payment.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub provisional: bool,
    /// A tombstone.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub deleted: bool,
    /// The attachments it references.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blobs: Vec<BlobRef>,
    /// The resource or the operation.
    pub body: Value,
}

/// Why an item is malformed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EnvelopeError {
    /// `content_hash` is not the hash of `body`.
    #[error("item {0}: content_hash does not match its body")]
    HashMismatch(OpId),
    /// A version whose `resource_type` is not a FHIR type name.
    #[error("item {0}: `{1}` is not a resource type")]
    ResourceType(OpId, String),
    /// A version whose `resource_id` is not a resource id.
    #[error("item {0}: `{1}` is not a resource id")]
    ResourceId(OpId, String),
    /// An operation with a `parent_version_id`.
    #[error("item {0}: an operation has no parent version")]
    OpWithParent(OpId),
    /// An operation without a ledger or scope.
    #[error("item {0}: an operation names its ledger and scope")]
    OpWithoutLedger(OpId),
}

impl Item {
    /// Whether the item is well-formed: its hash matches its body, and its subject fits its kind.
    pub fn check(&self) -> Result<(), EnvelopeError> {
        if ContentHash::of(&self.body) != self.content_hash {
            return Err(EnvelopeError::HashMismatch(self.op_id));
        }
        match self.kind {
            Kind::Version => {
                let mut chars = self.resource_type.chars();
                let type_name = chars.next().is_some_and(|c| c.is_ascii_uppercase())
                    && chars.all(|c| c.is_ascii_alphabetic());
                if !type_name {
                    return Err(EnvelopeError::ResourceType(
                        self.op_id,
                        self.resource_type.clone(),
                    ));
                }
                if self.resource_id.parse::<ResourceId>().is_err() {
                    return Err(EnvelopeError::ResourceId(
                        self.op_id,
                        self.resource_id.clone(),
                    ));
                }
            }
            Kind::Op => {
                if self.parent_version_id.is_some() {
                    return Err(EnvelopeError::OpWithParent(self.op_id));
                }
                if self.resource_type.is_empty() || self.resource_id.is_empty() {
                    return Err(EnvelopeError::OpWithoutLedger(self.op_id));
                }
            }
        }
        Ok(())
    }
}

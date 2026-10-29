use std::fmt;

use health_identifiers::{OpId, ResourceId};
use health_time::Timestamp;
use serde_json::{Map, Value};

use crate::ResourceType;

/// Resource JSON exactly as stored and served. It is built from input that has passed tier 1
/// validation on the raw JSON (ADR 0051), and is never produced by re-serializing a typed model.
#[derive(Clone, PartialEq, Eq)]
pub struct RawJson(Box<str>);

/// Why text cannot be stamped as a stored resource.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RawJsonError {
    /// Not JSON.
    #[error("not JSON: {0}")]
    Syntax(String),
    /// Not a JSON object with the expected `resourceType`.
    #[error("expected a {expected} resource object")]
    WrongShape {
        /// The resource type the write declared.
        expected: String,
    },
}

impl RawJson {
    /// Wraps JSON text that has already passed tier 1 validation.
    pub fn from_validated(text: impl Into<Box<str>>) -> Self {
        Self(text.into())
    }

    /// The stored text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for RawJson {
    // Resource bodies can hold clinical data, so they are never printed in debug output.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RawJson({} bytes)", self.0.len())
    }
}

/// Sets the server-owned fields on validated input: `id`, `meta.versionId` (the write's `op_id`,
/// ADR 0018) and `meta.lastUpdated`. Everything else, including key order and the exact text of
/// every number, is kept as received.
pub fn stamp_meta(
    input: &RawJson,
    resource_type: &ResourceType,
    id: ResourceId,
    version_id: OpId,
    last_updated: Timestamp,
) -> Result<RawJson, RawJsonError> {
    let mut value: Value =
        serde_json::from_str(input.as_str()).map_err(|e| RawJsonError::Syntax(e.to_string()))?;
    let wrong_shape = || RawJsonError::WrongShape {
        expected: resource_type.to_string(),
    };
    let object = value.as_object_mut().ok_or_else(wrong_shape)?;
    if object.get("resourceType").and_then(Value::as_str) != Some(resource_type.as_str()) {
        return Err(wrong_shape());
    }
    object.insert("id".to_owned(), Value::String(id.to_string()));
    let meta = object
        .entry("meta")
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .ok_or_else(wrong_shape)?;
    meta.insert(
        "versionId".to_owned(),
        Value::String(version_id.to_string()),
    );
    meta.insert(
        "lastUpdated".to_owned(),
        Value::String(fhir_instant(last_updated)),
    );
    let text = serde_json::to_string(&value).map_err(|e| RawJsonError::Syntax(e.to_string()))?;
    Ok(RawJson::from_validated(text))
}

/// A FHIR `instant`: UTC with millisecond precision, e.g. `2026-10-29T08:00:00.000Z`.
fn fhir_instant(at: Timestamp) -> String {
    format!("{at:.3}")
}

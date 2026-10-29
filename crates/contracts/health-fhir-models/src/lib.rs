//! The FHIR R5 model facade (ADRs 0015, 0050; fhir-r5-model.md § Resource model).
//!
//! - **Typed models.** The adopted crate is `fhir-r5 =4.2.3`. Its types are re-exported here
//!   under `health_fhir_models::{resources, types, codes, coded, validate}`, and no other crate
//!   may depend on `fhir-r5` (the `facade-bypass` architecture check). Swapping the crate then
//!   touches only this one.
//! - **One place builds `Resource`.** The serde code for the whole [`Resource`] enum is compiled
//!   in whichever crate instantiates it, at a cost of minutes in a release build. Parse and
//!   serialize it through [`parse_resource`] and [`to_json`], which are not generic, so it is
//!   compiled here once. Only rarely changing crates should name `Resource` in other ways.
//! - **Never the stored form.** Typed parsing is strict (a missing required element is an
//!   error) and drops unknown elements and an `_id` extension. So tier 1 validation runs on the
//!   raw JSON first, and the stored resource is the validated input, not a re-serialized model
//!   (ADR 0051).

pub use fhir_r5::resources::Resource;
pub use fhir_r5::{coded, codes, resources, types, validate};

use serde_json::Value;

/// Why JSON could not be read or written as a resource.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ModelError {
    /// The text is not a resource the model accepts: invalid JSON, an unknown
    /// `resourceType`, a missing required element or a wrongly typed value.
    #[error("not a valid R5 resource: {0}")]
    Parse(String),
    /// The model could not be written as JSON.
    #[error("cannot serialize the resource: {0}")]
    Serialize(String),
}

/// Parses R5 JSON into the typed model, with `resourceType` selecting the variant.
pub fn parse_resource(json: &str) -> Result<Resource, ModelError> {
    serde_json::from_str(json).map_err(|e| ModelError::Parse(e.to_string()))
}

/// Writes a resource as R5 JSON, with `resourceType`. Decimals keep their text (`1.50` stays
/// `1.50`) and `integer64` values are strings.
pub fn to_json(resource: &Resource) -> Result<String, ModelError> {
    serde_json::to_string(resource).map_err(|e| ModelError::Serialize(e.to_string()))
}

/// The official R5 examples the model crate cannot parse although they are valid R5.
///
/// ADR 0050 recorded this example as invalid, "missing the required
/// `QuestionnaireResponse.questionnaire`". In fact it carries `_questionnaire`, an
/// extension-only primitive, which R5 allows for a required primitive and tier 1 accepts.
/// `fhir-r5` treats the element as missing, so this is a gap of the crate, worth reporting
/// upstream. Such resources are stored and served as they are, and only typed domain
/// operations on them fail (ADR 0051).
pub const KNOWN_UNPARSEABLE_EXAMPLES: &[&str] =
    &["questionnaireresponse-example-f201-lifelines.json"];

/// The outcome of parsing and re-serializing JSON.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoundTrip {
    /// The output is the same JSON: same members, values and number text. Member order may
    /// differ, as it does not matter in FHIR JSON.
    Exact,
    /// The output differs, first at this JSON pointer.
    Differs {
        /// Where the first difference is, e.g. `/component/0/valueQuantity/value`.
        pointer: String,
    },
}

/// Parses `json`, writes it back, and compares the two.
pub fn round_trip(json: &str) -> Result<RoundTrip, ModelError> {
    let resource = parse_resource(json)?;
    let written = to_json(&resource)?;
    let before: Value = serde_json::from_str(json).map_err(|e| ModelError::Parse(e.to_string()))?;
    let after: Value =
        serde_json::from_str(&written).map_err(|e| ModelError::Serialize(e.to_string()))?;
    Ok(match first_difference(&before, &after, String::new()) {
        None => RoundTrip::Exact,
        Some(pointer) => RoundTrip::Differs { pointer },
    })
}

fn first_difference(a: &Value, b: &Value, at: String) -> Option<String> {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            for key in x.keys().chain(y.keys().filter(|k| !x.contains_key(*k))) {
                let path = format!("{at}/{}", key.replace('~', "~0").replace('/', "~1"));
                match (x.get(key), y.get(key)) {
                    (Some(l), Some(r)) => {
                        if let Some(found) = first_difference(l, r, path) {
                            return Some(found);
                        }
                    }
                    _ => return Some(path),
                }
            }
            None
        }
        (Value::Array(x), Value::Array(y)) => {
            if x.len() != y.len() {
                return Some(at);
            }
            x.iter()
                .zip(y)
                .enumerate()
                .find_map(|(i, (l, r))| first_difference(l, r, format!("{at}/{i}")))
        }
        // Numbers compare by their text (`arbitrary_precision`), so 1.50 differs from 1.5.
        _ => (a != b).then_some(at),
    }
}

//! JSON Schema validation of a package's `configuration/` (configuration.md § Package format).
//!
//! The schemas belong to the software, not the package: a schema a package brought with it
//! would let its author pass anything. Each configuration file names the schema it is written
//! to with a top-level `$schema` URI. The release knows those URIs ([`Schemas`]), as it knows
//! the `formatVersion`s it reads, and refuses a file that names none, names one it does not
//! know, or does not validate.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::Error;
use crate::archive::Files;

/// Where configuration lives in a package.
const FOLDER: &str = "configuration/";

/// The `$schema` of a Form Layout: a Questionnaire's presentation (configuration.md).
pub const FORM_LAYOUT: &str =
    "https://carapai.github.io/people-centered-healthcare/fhir/schema/form-layout/1";

const FORM_LAYOUT_SCHEMA: &str = include_str!("../schemas/form-layout-1.json");

/// The configuration schemas a software release reads, by `$schema` URI.
#[derive(Default)]
pub struct Schemas {
    validators: BTreeMap<String, jsonschema::Validator>,
}

impl std::fmt::Debug for Schemas {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_set().entries(self.validators.keys()).finish()
    }
}

/// The package format versions this release reads (configuration.md § Distribution and
/// activation on nodes): like the schemas, they belong to the release, never to a package.
pub const FORMAT_VERSIONS: &[u32] = &[1];

impl Schemas {
    /// The schemas built into this release, each arriving with the feature that reads its
    /// configuration: Form Layouts (#92, [`FORM_LAYOUT`]). Configuration naming any other schema
    /// (override points, settings) is refused until its feature arrives.
    pub fn platform() -> Self {
        let mut schemas = Self::default();
        for (uri, text) in [(FORM_LAYOUT, FORM_LAYOUT_SCHEMA)] {
            // Built into the release and checked by its tests, so it always compiles.
            if let Ok(schema) = serde_json::from_str::<Value>(text)
                && let Ok(validator) = jsonschema::validator_for(&schema)
            {
                schemas.validators.insert(uri.to_owned(), validator);
            }
        }
        schemas
    }

    /// With `schema` for files naming `uri`. Remote `$ref`s are not resolved, so a schema must be
    /// whole.
    pub fn with(mut self, uri: impl Into<String>, schema: &Value) -> Result<Self, Error> {
        let uri = uri.into();
        let validator = jsonschema::validator_for(schema).map_err(|e| Error::Configuration {
            path: uri.clone(),
            detail: format!("not a usable schema: {e}"),
        })?;
        self.validators.insert(uri, validator);
        Ok(self)
    }

    /// The URIs known.
    pub fn uris(&self) -> impl Iterator<Item = &str> {
        self.validators.keys().map(String::as_str)
    }

    /// Checks every file under `configuration/` in `files`.
    pub(crate) fn check(&self, files: &Files) -> Result<(), Error> {
        for (path, bytes) in files.range(FOLDER.to_owned()..) {
            if !path.starts_with(FOLDER) {
                break;
            }
            let refused = |detail: String| Error::Configuration {
                path: path.clone(),
                detail,
            };
            if !path.ends_with(".json") {
                return Err(refused("configuration must be JSON".into()));
            }
            let value: Value = serde_json::from_slice(bytes).map_err(|e| refused(e.to_string()))?;
            let uri = value
                .get("$schema")
                .and_then(Value::as_str)
                .ok_or_else(|| refused("names no $schema".into()))?;
            let validator = self
                .validators
                .get(uri)
                .ok_or_else(|| refused(format!("{uri} is not a schema this release reads")))?;
            let errors: Vec<String> = validator
                .iter_errors(&value)
                .map(|e| {
                    let at = e.instance_path().to_string();
                    if at.is_empty() {
                        e.to_string()
                    } else {
                        format!("{at}: {e}")
                    }
                })
                .collect();
            if !errors.is_empty() {
                return Err(refused(errors.join("; ")));
            }
        }
        Ok(())
    }
}

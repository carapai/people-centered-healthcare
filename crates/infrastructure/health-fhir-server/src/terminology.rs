//! The terminology operations over HTTP (#90; terminology.md § Runtime service), served from the
//! node's own terminology with no external server:
//!
//! | Route | Answer |
//! |---|---|
//! | `ValueSet/$expand` (`url`, `valueSetVersion`, `filter`, `offset`, `count`, `activeOnly`) | a ValueSet with its `expansion` |
//! | `ValueSet/$validate-code` (`url`, `system`, `code`, `display`, or `coding`) | Parameters: `result`, `display`, `message` |
//! | `CodeSystem/$validate-code` (`url`, `version`, `code`, `display`, or `coding`) | the same |
//! | `CodeSystem/$lookup` (`system`, `version`, `code`, or `coding`) | Parameters: `name`, `version`, `display`, `definition`, `designation`, `property` (with `parent` and `child`) |
//! | `CodeSystem/$subsumes` (`system`, `version`, `codeA`, `codeB`) | Parameters: `outcome` |
//! | `ConceptMap/$translate` (`url`, `sourceCode` and `sourceSystem`, or `sourceCoding`, and `targetSystem`) | Parameters: `result`, `match` (`relationship`, `concept`, `originMap`) |
//!
//! Each takes its parameters from the query string (GET) or a Parameters body (POST). What the
//! node does not hold is 404 (`not-found`), and what cannot be answered offline, such as a
//! filter on a code system held in part, is 422 (`not-supported`): never an empty answer.

use std::sync::{Arc, RwLock};

use axum::body::Bytes;
use axum::extract::{RawQuery, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use health_terminology::{
    Error, ExpandRequest, Relationship, Subsumption, Terminology, Validation,
};
use serde_json::{Value, json};

use crate::http::fhir_json;
use crate::{FhirError, FhirServer, pairs};

/// The terminology a node serves. Activation (#89) replaces it when a package with terminology
/// becomes active; readers keep the version they started with.
#[derive(Debug, Clone, Default)]
pub struct ActiveTerminology(Arc<RwLock<Arc<Terminology>>>);

impl ActiveTerminology {
    /// Serving `terminology`.
    pub fn new(terminology: Terminology) -> Self {
        Self(Arc::new(RwLock::new(Arc::new(terminology))))
    }

    /// What is served now.
    pub fn current(&self) -> Arc<Terminology> {
        self.0.read().map_or_else(
            |poisoned| poisoned.into_inner().clone(),
            |guard| guard.clone(),
        )
    }

    /// Serves `terminology` from now on.
    pub fn replace(&self, terminology: Terminology) {
        let next = Arc::new(terminology);
        match self.0.write() {
            Ok(mut guard) => *guard = next,
            Err(poisoned) => *poisoned.into_inner() = next,
        }
    }
}

impl From<Error> for FhirError {
    fn from(error: Error) -> Self {
        match error {
            Error::UnknownCodeSystem(_)
            | Error::UnknownValueSet(_)
            | Error::UnknownConceptMap(_)
            | Error::UnknownCode { .. } => FhirError::not_found(error.to_string()),
            Error::Incomplete(_) | Error::UnsupportedFilter { .. } | Error::Cycle(_) => {
                FhirError::new(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "not-supported",
                    error.to_string(),
                )
            }
        }
    }
}

/// An operation's parameters, by name, from the query string or a Parameters body. A
/// `Coding` parameter gives `<name>.system`, `<name>.code` and `<name>.display`.
struct Inputs(Vec<(String, String)>);

impl Inputs {
    fn read(query: Option<&str>, headers: &HeaderMap, body: &Bytes) -> Result<Self, FhirError> {
        let mut found = pairs(query);
        if !body.is_empty() {
            crate::http::require_json_body(headers)?;
            let value: Value = serde_json::from_slice(body)
                .map_err(|e| FhirError::bad_request(format!("the body is not JSON: {e}")))?;
            if value["resourceType"] != "Parameters" {
                return Err(FhirError::bad_request(
                    "the body is not a Parameters resource",
                ));
            }
            for parameter in value["parameter"].as_array().into_iter().flatten() {
                let Some(name) = parameter["name"].as_str() else {
                    continue;
                };
                if let Some(coding) = parameter.get("valueCoding") {
                    for part in ["system", "code", "display", "version"] {
                        if let Some(text) = coding[part].as_str() {
                            found.push((format!("{name}.{part}"), text.to_owned()));
                        }
                    }
                    continue;
                }
                let value = parameter
                    .as_object()
                    .into_iter()
                    .flatten()
                    .find(|(key, _)| key.starts_with("value"))
                    .map(|(_, v)| match v {
                        Value::String(text) => text.clone(),
                        other => other.to_string(),
                    });
                if let Some(value) = value {
                    found.push((name.to_owned(), value));
                }
            }
        }
        Ok(Self(found))
    }

    fn get(&self, name: &str) -> Option<&str> {
        self.0
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    fn require(&self, name: &str, operation: &str) -> Result<&str, FhirError> {
        self.get(name)
            .ok_or_else(|| FhirError::bad_request(format!("{operation} needs {name}")))
    }

    /// `code` and `system`, or the same from a Coding parameter `coding`.
    fn coded<'a>(
        &'a self,
        code: &str,
        system: &str,
        coding: &str,
    ) -> (Option<&'a str>, Option<&'a str>, Option<&'a str>) {
        let from_coding = |part: &str| self.get(&format!("{coding}.{part}"));
        (
            self.get(code).or_else(|| from_coding("code")),
            self.get(system).or_else(|| from_coding("system")),
            self.get("display").or_else(|| from_coding("display")),
        )
    }

    fn number(&self, name: &str) -> Result<Option<usize>, FhirError> {
        self.get(name)
            .map(|text| {
                text.parse()
                    .map_err(|_| FhirError::bad_request(format!("{name} must be a whole number")))
            })
            .transpose()
    }
}

fn terminology(server: &FhirServer) -> Result<Arc<Terminology>, FhirError> {
    server
        .terminology
        .as_ref()
        .map(ActiveTerminology::current)
        .ok_or_else(|| FhirError::not_found("this node serves no terminology"))
}

fn parameters(parameter: Vec<Value>) -> Response {
    fhir_json(
        StatusCode::OK,
        json!({ "resourceType": "Parameters", "parameter": parameter }).to_string(),
    )
}

/// `ValueSet/$expand`.
pub(crate) async fn expand(
    State(server): State<FhirServer>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, FhirError> {
    let inputs = Inputs::read(query.as_deref(), &headers, &body)?;
    let url = inputs.require("url", "$expand")?;
    let reference = match inputs.get("valueSetVersion") {
        Some(version) => format!("{url}|{version}"),
        None => url.to_owned(),
    };
    let request = ExpandRequest {
        filter: inputs.get("filter").map(str::to_owned),
        active_only: inputs.get("activeOnly") == Some("true"),
        offset: inputs.number("offset")?.unwrap_or(0),
        count: inputs.number("count")?,
    };
    let expansion = terminology(&server)?.expand(&reference, &request)?;
    let (url, version) = expansion
        .value_set
        .split_once('|')
        .map_or((expansion.value_set.as_str(), None), |(u, v)| (u, Some(v)));
    let contains: Vec<Value> = expansion
        .contains
        .iter()
        .map(|c| {
            let mut entry = json!({ "system": c.system, "code": c.code });
            if let Some(version) = &c.version {
                entry["version"] = json!(version);
            }
            if let Some(display) = &c.display {
                entry["display"] = json!(display);
            }
            if c.inactive {
                entry["inactive"] = json!(true);
            }
            entry
        })
        .collect();
    let mut value_set = json!({
        "resourceType": "ValueSet",
        "url": url,
        "status": "active",
        "expansion": {
            "timestamp": health_time::Timestamp::now().to_string(),
            "total": expansion.total,
            "offset": request.offset,
            "parameter": [{ "name": "pinned", "valueBoolean": expansion.pinned }],
            "contains": contains,
        },
    });
    if let Some(version) = version {
        value_set["version"] = json!(version);
    }
    Ok(fhir_json(StatusCode::OK, value_set.to_string()))
}

fn validation(found: Validation, code: &str) -> Response {
    let (result, display, message) = match found {
        Validation::Valid { display } => (true, display, None),
        Validation::WrongDisplay { expected } => (
            false,
            expected.clone(),
            Some(format!(
                "the display given is not {code}'s display, {}",
                expected.as_deref().unwrap_or("(none)")
            )),
        ),
        Validation::Invalid => (false, None, Some(format!("{code} is not valid here"))),
    };
    let mut out = vec![json!({ "name": "result", "valueBoolean": result })];
    if let Some(display) = display {
        out.push(json!({ "name": "display", "valueString": display }));
    }
    if let Some(message) = message {
        out.push(json!({ "name": "message", "valueString": message }));
    }
    parameters(out)
}

/// `ValueSet/$validate-code`.
pub(crate) async fn validate_value_set(
    State(server): State<FhirServer>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, FhirError> {
    let inputs = Inputs::read(query.as_deref(), &headers, &body)?;
    let url = inputs.require("url", "$validate-code")?;
    let reference = match inputs.get("valueSetVersion") {
        Some(version) => format!("{url}|{version}"),
        None => url.to_owned(),
    };
    let (code, system, display) = inputs.coded("code", "system", "coding");
    let (Some(code), Some(system)) = (code, system) else {
        return Err(FhirError::bad_request(
            "$validate-code needs a code and its system, or a coding",
        ));
    };
    let found = terminology(&server)?.validate_code(&reference, system, code, display)?;
    Ok(validation(found, code))
}

/// `CodeSystem/$validate-code`.
pub(crate) async fn validate_code_system(
    State(server): State<FhirServer>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, FhirError> {
    let inputs = Inputs::read(query.as_deref(), &headers, &body)?;
    let (code, system, display) = inputs.coded("code", "url", "coding");
    let (Some(code), Some(system)) = (code, system) else {
        return Err(FhirError::bad_request(
            "$validate-code needs a code and the code system's url, or a coding",
        ));
    };
    let found = terminology(&server)?.validate_in_code_system(
        system,
        inputs.get("version"),
        code,
        display,
    )?;
    Ok(validation(found, code))
}

/// `CodeSystem/$lookup`.
pub(crate) async fn lookup(
    State(server): State<FhirServer>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, FhirError> {
    let inputs = Inputs::read(query.as_deref(), &headers, &body)?;
    let (code, system, _) = inputs.coded("code", "system", "coding");
    let (Some(code), Some(system)) = (code, system) else {
        return Err(FhirError::bad_request(
            "$lookup needs a code and its system, or a coding",
        ));
    };
    let found = terminology(&server)?.lookup(system, inputs.get("version"), code)?;
    let mut out = vec![json!({ "name": "name", "valueString": found.system })];
    if let Some(version) = &found.version {
        out.push(json!({ "name": "version", "valueString": version }));
    }
    if let Some(display) = &found.concept.display {
        out.push(json!({ "name": "display", "valueString": display }));
    }
    if let Some(definition) = &found.concept.definition {
        out.push(json!({ "name": "definition", "valueString": definition }));
    }
    for designation in &found.concept.designations {
        let mut part = vec![json!({ "name": "value", "valueString": designation.value })];
        if let Some(language) = &designation.language {
            part.push(json!({ "name": "language", "valueCode": language }));
        }
        if let Some(code) = &designation.use_code {
            part.push(json!({ "name": "use", "valueCoding": { "code": code } }));
        }
        out.push(json!({ "name": "designation", "part": part }));
    }
    let property = |code: &str, value: Value| json!({ "name": "property", "part": [{ "name": "code", "valueCode": code }, value] });
    for p in &found.concept.properties {
        let value = match &p.value {
            health_terminology::PropertyValue::Code(text) => {
                json!({ "name": "value", "valueCode": text })
            }
            health_terminology::PropertyValue::Boolean(flag) => {
                json!({ "name": "value", "valueBoolean": flag })
            }
            health_terminology::PropertyValue::String(text)
            | health_terminology::PropertyValue::Other(text) => {
                json!({ "name": "value", "valueString": text })
            }
        };
        out.push(property(&p.code, value));
    }
    for parent in &found.parents {
        out.push(property(
            "parent",
            json!({ "name": "value", "valueCode": parent }),
        ));
    }
    for child in &found.children {
        out.push(property(
            "child",
            json!({ "name": "value", "valueCode": child }),
        ));
    }
    Ok(parameters(out))
}

/// `CodeSystem/$subsumes`.
pub(crate) async fn subsumes(
    State(server): State<FhirServer>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, FhirError> {
    let inputs = Inputs::read(query.as_deref(), &headers, &body)?;
    let system = inputs
        .get("system")
        .or_else(|| inputs.get("codingA.system"))
        .ok_or_else(|| FhirError::bad_request("$subsumes needs the system"))?;
    let a = inputs
        .get("codeA")
        .or_else(|| inputs.get("codingA.code"))
        .ok_or_else(|| FhirError::bad_request("$subsumes needs codeA"))?;
    let b = inputs
        .get("codeB")
        .or_else(|| inputs.get("codingB.code"))
        .ok_or_else(|| FhirError::bad_request("$subsumes needs codeB"))?;
    let outcome = match terminology(&server)?.subsumes(system, inputs.get("version"), a, b)? {
        Subsumption::Equivalent => "equivalent",
        Subsumption::Subsumes => "subsumes",
        Subsumption::SubsumedBy => "subsumed-by",
        Subsumption::NotSubsumed => "not-subsumed",
    };
    Ok(parameters(vec![
        json!({ "name": "outcome", "valueCode": outcome }),
    ]))
}

/// `ConceptMap/$translate`.
pub(crate) async fn translate(
    State(server): State<FhirServer>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, FhirError> {
    let inputs = Inputs::read(query.as_deref(), &headers, &body)?;
    let (code, system, _) = inputs.coded("sourceCode", "sourceSystem", "sourceCoding");
    let (Some(code), Some(system)) = (code, system) else {
        return Err(FhirError::bad_request(
            "$translate needs sourceCode and sourceSystem, or a sourceCoding",
        ));
    };
    let found = terminology(&server)?.translate(
        inputs.get("url"),
        system,
        code,
        inputs.get("targetSystem"),
    )?;
    let mapped = found
        .iter()
        .any(|t| t.target.relationship != Relationship::NotRelatedTo);
    let mut out = vec![json!({ "name": "result", "valueBoolean": mapped })];
    for translation in &found {
        let relationship = match translation.target.relationship {
            Relationship::Equivalent => "equivalent",
            Relationship::RelatedTo => "related-to",
            Relationship::SourceIsNarrowerThanTarget => "source-is-narrower-than-target",
            Relationship::SourceIsBroaderThanTarget => "source-is-broader-than-target",
            Relationship::NotRelatedTo => "not-related-to",
        };
        let mut part = vec![
            json!({ "name": "relationship", "valueCode": relationship }),
            json!({ "name": "originMap", "valueCanonical": translation.concept_map }),
        ];
        if let Some(target) = &translation.target.code {
            let mut coding = json!({ "system": translation.system, "code": target });
            if let Some(display) = &translation.target.display {
                coding["display"] = json!(display);
            }
            part.push(json!({ "name": "concept", "valueCoding": coding }));
        }
        out.push(json!({ "name": "match", "part": part }));
    }
    Ok(parameters(out))
}

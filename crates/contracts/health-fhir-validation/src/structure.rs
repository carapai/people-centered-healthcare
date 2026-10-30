//! Tier 1: structural validation of raw R5 JSON (ADR 0051; fhir-r5-model.md § Validation).
//!
//! It walks the JSON against the element table generated from the core StructureDefinitions
//! (`data/r5-elements.json`, `tools/fhir-element-table`) and reports, with FHIRPath locations:
//!
//! - a missing or unknown `resourceType`, and abstract types;
//! - **unknown elements** (anything not in the definition and not an extension), which are
//!   always errors;
//! - cardinality: arrays where `max > 1`, single values where `max = 1`, `max` counts, required
//!   elements;
//! - primitive formats: JSON type, the core `regex`, and integer ranges;
//! - choice exclusivity: at most one `value[x]`-style key per choice element;
//! - empty objects, arrays and strings, and `null` outside primitive arrays paired with `_name`.

use std::collections::{BTreeMap, HashMap};
use std::sync::OnceLock;

use regex::Regex;
use serde_json::{Map, Value};

/// What kind of problem an issue is, as an OperationOutcome `issue.code`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssueCode {
    /// Not valid JSON, or an unknown element or resource type, or the wrong JSON shape.
    Structure,
    /// A required element is missing.
    Required,
    /// A primitive value is malformed.
    Value,
    /// A core invariant does not hold (tier 2).
    Invariant,
    /// The resource could not be validated, e.g. a decimal beyond 28 significant digits that
    /// FHIRPath cannot evaluate exactly; it fails closed.
    Processing,
}

impl IssueCode {
    /// The OperationOutcome code.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Structure => "structure",
            Self::Required => "required",
            Self::Value => "value",
            Self::Invariant => "invariant",
            Self::Processing => "processing",
        }
    }
}

/// One validation error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    /// The kind of problem.
    pub code: IssueCode,
    /// Where, as a FHIRPath expression, e.g. `Patient.name[0].given[1]`.
    pub location: String,
    /// What is wrong.
    pub message: String,
}

pub(crate) struct Element {
    pub(crate) min: u64,
    /// `None` for `*`.
    pub(crate) max: Option<u64>,
    pub(crate) types: Vec<String>,
    pub(crate) content_reference: Option<String>,
}

/// An invariant a type defines at one of its element paths ("" for the type itself).
pub(crate) struct Constraint {
    pub(crate) key: String,
    pub(crate) severity: String,
    pub(crate) expression: String,
    pub(crate) human: String,
}

pub(crate) struct TypeDef {
    pub(crate) kind: String,
    pub(crate) is_abstract: bool,
    pub(crate) base: Option<String>,
    pub(crate) elements: HashMap<String, Element>,
    /// Parent path ("" for the root) to the paths of its direct children.
    pub(crate) children: HashMap<String, Vec<String>>,
    /// Element path ("" for the root) to the invariants defined there.
    pub(crate) constraints: HashMap<String, Vec<Constraint>>,
}

pub(crate) struct Primitive {
    json: String,
    regex: Option<Regex>,
}

pub(crate) struct Table {
    pub(crate) types: HashMap<String, TypeDef>,
    pub(crate) primitives: HashMap<String, Primitive>,
}

impl Table {
    /// `type_name` and its ancestors, nearest first.
    pub(crate) fn lineage<'a>(&'a self, type_name: &'a str) -> impl Iterator<Item = &'a str> + 'a {
        std::iter::successors(Some(type_name), |t| {
            self.types.get(*t).and_then(|d| d.base.as_deref())
        })
    }
}

const TABLE_JSON: &str = include_str!("../data/r5-elements.json");

pub(crate) fn table() -> &'static Table {
    static TABLE: OnceLock<Table> = OnceLock::new();
    TABLE.get_or_init(|| {
        // The table is generated and checked in CI, so a malformed table is a build defect.
        #[allow(clippy::expect_used)]
        let raw: Value =
            serde_json::from_str(TABLE_JSON).expect("the generated element table is valid JSON");
        let mut types = HashMap::new();
        for (name, def) in raw["types"].as_object().into_iter().flatten() {
            let mut elements = HashMap::new();
            let mut children: HashMap<String, Vec<String>> = HashMap::new();
            for (path, entry) in def["elements"].as_object().into_iter().flatten() {
                let entry = entry.as_array().map(Vec::as_slice).unwrap_or_default();
                let max = entry.get(1).and_then(Value::as_str).unwrap_or("*");
                elements.insert(
                    path.clone(),
                    Element {
                        min: entry.first().and_then(Value::as_u64).unwrap_or(0),
                        max: max.parse().ok(),
                        types: entry
                            .get(2)
                            .and_then(Value::as_array)
                            .map(|t| {
                                t.iter()
                                    .filter_map(Value::as_str)
                                    .map(str::to_owned)
                                    .collect()
                            })
                            .unwrap_or_default(),
                        content_reference: entry.get(3).and_then(Value::as_str).map(str::to_owned),
                    },
                );
                let parent = path.rsplit_once('.').map_or("", |(p, _)| p).to_owned();
                children.entry(parent).or_default().push(path.clone());
            }
            let mut constraints: HashMap<String, Vec<Constraint>> = HashMap::new();
            for c in def["constraints"].as_array().into_iter().flatten() {
                let text = |key: &str| c[key].as_str().unwrap_or_default().to_owned();
                constraints
                    .entry(text("path"))
                    .or_default()
                    .push(Constraint {
                        key: text("key"),
                        severity: text("severity"),
                        expression: text("expression"),
                        human: text("human"),
                    });
            }
            types.insert(
                name.clone(),
                TypeDef {
                    kind: def["kind"].as_str().unwrap_or_default().to_owned(),
                    is_abstract: def["abstract"].as_bool().unwrap_or(false),
                    base: def["base"].as_str().map(str::to_owned),
                    elements,
                    children,
                    constraints,
                },
            );
        }
        let primitives = raw["primitives"]
            .as_object()
            .into_iter()
            .flatten()
            .map(|(name, def)| {
                let regex = def["regex"]
                    .as_str()
                    .and_then(|r| Regex::new(&format!("^(?:{r})$")).ok());
                (
                    name.clone(),
                    Primitive {
                        json: def["json"].as_str().unwrap_or("string").to_owned(),
                        regex,
                    },
                )
            })
            .collect();
        Table { types, primitives }
    })
}

/// The resource types tier 1 knows (concrete ones only).
pub fn resource_types() -> impl Iterator<Item = &'static str> {
    table()
        .types
        .iter()
        .filter(|(_, t)| t.kind == "resource" && !t.is_abstract)
        .map(|(name, _)| name.as_str())
}

/// Validates resource JSON text. An empty result means it passed tier 1.
pub fn validate_structure(json: &str) -> Vec<Issue> {
    match serde_json::from_str::<Value>(json) {
        Ok(value) => validate_value(&value),
        Err(error) => vec![Issue {
            code: IssueCode::Structure,
            location: String::new(),
            message: format!("not valid JSON: {error}"),
        }],
    }
}

/// Validates a parsed resource.
pub fn validate_value(value: &Value) -> Vec<Issue> {
    let mut walker = Walker {
        table: table(),
        issues: Vec::new(),
    };
    walker.resource(value, None);
    walker.issues
}

struct Walker {
    table: &'static Table,
    issues: Vec<Issue>,
}

fn join(base: &str, name: &str) -> String {
    if base.is_empty() {
        name.to_owned()
    } else {
        format!("{base}.{name}")
    }
}

pub(crate) fn is_primitive(type_code: &str) -> bool {
    type_code.starts_with(|c: char| c.is_ascii_lowercase())
}

impl Walker {
    fn issue(&mut self, code: IssueCode, location: &str, message: impl Into<String>) {
        self.issues.push(Issue {
            code,
            location: location.to_owned(),
            message: message.into(),
        });
    }

    /// A resource at `location` (`None` for the root, whose location is its type).
    fn resource(&mut self, value: &Value, location: Option<&str>) {
        let Some(object) = value.as_object() else {
            self.issue(
                IssueCode::Structure,
                location.unwrap_or(""),
                "a resource must be a JSON object",
            );
            return;
        };
        let Some(type_name) = object.get("resourceType").and_then(Value::as_str) else {
            self.issue(
                IssueCode::Structure,
                location.unwrap_or(""),
                "resourceType is missing",
            );
            return;
        };
        let known = self
            .table
            .types
            .get(type_name)
            .is_some_and(|t| t.kind == "resource" && !t.is_abstract);
        if !known {
            self.issue(
                IssueCode::Structure,
                location.unwrap_or(""),
                format!("{type_name} is not an R5 resource type"),
            );
            return;
        }
        let here = location.map_or_else(|| type_name.to_owned(), str::to_owned);
        self.object(type_name, "", object, &here, true);
    }

    /// The members of an object of `type_name`, at element path `base` within it.
    fn object(
        &mut self,
        type_name: &str,
        base: &str,
        object: &Map<String, Value>,
        location: &str,
        is_resource: bool,
    ) {
        let table = self.table;
        let Some(def) = table.types.get(type_name) else {
            self.issue(
                IssueCode::Structure,
                location,
                format!("no definition for {type_name}"),
            );
            return;
        };
        if object.is_empty() {
            self.issue(
                IssueCode::Structure,
                location,
                "empty objects are not allowed",
            );
            return;
        }
        // Element path → the keys that set it (a choice element counts each type once).
        let mut seen: BTreeMap<&str, Vec<String>> = BTreeMap::new();
        for (key, value) in object {
            if is_resource && key == "resourceType" {
                continue;
            }
            let (name, underscore) = key
                .strip_prefix('_')
                .map_or((key.as_str(), false), |n| (n, true));
            let member = format!("{location}.{name}");
            let Some((path, element, chosen)) = resolve(def, base, name) else {
                self.issue(
                    IssueCode::Structure,
                    &member,
                    format!("{name} is not an element of {}", join(type_name, base)),
                );
                continue;
            };
            seen.entry(path).or_default().push(chosen.clone());
            if underscore {
                if !is_primitive(&chosen) {
                    self.issue(
                        IssueCode::Structure,
                        &format!("{location}.{key}"),
                        "only primitive elements have _ properties",
                    );
                    continue;
                }
                let paired = object.get(name);
                self.primitive_extension(
                    value,
                    paired,
                    element.max != Some(1),
                    &format!("{location}.{key}"),
                );
                continue;
            }
            let paired = object.get(&format!("_{name}"));
            self.member(type_name, path, element, &chosen, value, paired, &member);
        }
        for (path, keys) in &seen {
            let mut distinct = keys.clone();
            distinct.sort();
            distinct.dedup();
            if distinct.len() > 1 {
                self.issue(
                    IssueCode::Structure,
                    location,
                    format!("{path} has more than one type: {}", distinct.join(", ")),
                );
            }
        }
        for child in def.children.get(base).into_iter().flatten() {
            let Some(element) = def.elements.get(child) else {
                continue;
            };
            if element.min > 0 && !seen.contains_key(child.as_str()) {
                let name = child.rsplit('.').next().unwrap_or(child);
                self.issue(
                    IssueCode::Required,
                    &format!("{location}.{name}"),
                    format!("{name} is required"),
                );
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn member(
        &mut self,
        type_name: &str,
        path: &str,
        element: &Element,
        chosen: &str,
        value: &Value,
        paired: Option<&Value>,
        location: &str,
    ) {
        let repeating = element.max != Some(1);
        match value {
            Value::Array(items) if repeating => {
                if items.is_empty() {
                    self.issue(
                        IssueCode::Structure,
                        location,
                        "empty arrays are not allowed",
                    );
                }
                if let Some(max) = element.max
                    && items.len() as u64 > max
                {
                    self.issue(
                        IssueCode::Structure,
                        location,
                        format!("at most {max} allowed"),
                    );
                }
                let paired_items = paired.and_then(Value::as_array);
                for (i, item) in items.iter().enumerate() {
                    let here = format!("{location}[{i}]");
                    if item.is_null() {
                        let has_extension = paired_items
                            .and_then(|p| p.get(i))
                            .is_some_and(|p| !p.is_null());
                        if !(is_primitive(chosen) && has_extension) {
                            self.issue(
                                IssueCode::Structure,
                                &here,
                                "null is only allowed in a primitive array paired with extensions",
                            );
                        }
                        continue;
                    }
                    self.typed(type_name, path, element, chosen, item, &here);
                }
            }
            Value::Array(_) => self.issue(
                IssueCode::Structure,
                location,
                "a single value is expected, not an array",
            ),
            _ if repeating => self.issue(IssueCode::Structure, location, "an array is expected"),
            Value::Null => self.issue(IssueCode::Structure, location, "null is not allowed"),
            _ => self.typed(type_name, path, element, chosen, value, location),
        }
    }

    /// One value of an element, of type `chosen`.
    fn typed(
        &mut self,
        type_name: &str,
        path: &str,
        element: &Element,
        chosen: &str,
        value: &Value,
        location: &str,
    ) {
        if is_primitive(chosen) {
            return self.primitive(chosen, value, location);
        }
        // ele-1: an element needs a value or children other than its id (found by the HL7 Java
        // validator differential, #166). Resources are not elements.
        if !matches!(chosen, "Resource" | "DomainResource")
            && value
                .as_object()
                .is_some_and(|o| !o.is_empty() && o.keys().all(|k| k == "id"))
        {
            self.issue(
                IssueCode::Structure,
                location,
                "an element needs children other than its id (ele-1)",
            );
            return;
        }
        if let Some(target) = &element.content_reference {
            return self.complex(type_name, target, value, location);
        }
        match chosen {
            "Resource" | "DomainResource" => self.resource(value, Some(location)),
            "BackboneElement" | "Element"
                if self
                    .table
                    .types
                    .get(type_name)
                    .is_some_and(|d| d.children.contains_key(path)) =>
            {
                self.complex(type_name, path, value, location);
            }
            other => self.complex(other, "", value, location),
        }
    }

    fn complex(&mut self, type_name: &str, base: &str, value: &Value, location: &str) {
        match value.as_object() {
            Some(object) => self.object(type_name, base, object, location, false),
            None => self.issue(IssueCode::Structure, location, "an object is expected"),
        }
    }

    fn primitive(&mut self, type_code: &str, value: &Value, location: &str) {
        let Some(primitive) = self.table.primitives.get(type_code) else {
            self.issue(
                IssueCode::Structure,
                location,
                format!("unknown primitive type {type_code}"),
            );
            return;
        };
        let text = match (primitive.json.as_str(), value) {
            ("string", Value::String(s)) => {
                if s.is_empty() {
                    self.issue(IssueCode::Value, location, "empty strings are not allowed");
                    return;
                }
                s.clone()
            }
            ("number", Value::Number(n)) => n.to_string(),
            ("boolean", Value::Bool(b)) => b.to_string(),
            (json, _) => {
                self.issue(
                    IssueCode::Structure,
                    location,
                    format!("a {type_code} must be a JSON {json}"),
                );
                return;
            }
        };
        if matches!(type_code, "integer" | "positiveInt" | "unsignedInt") {
            let range = match type_code {
                "positiveInt" => 1..=i64::from(i32::MAX),
                "unsignedInt" => 0..=i64::from(i32::MAX),
                _ => i64::from(i32::MIN)..=i64::from(i32::MAX),
            };
            if !text.parse::<i64>().is_ok_and(|n| range.contains(&n)) {
                self.issue(
                    IssueCode::Value,
                    location,
                    format!("{text} is not a valid {type_code}"),
                );
                return;
            }
        }
        if let Some(regex) = &primitive.regex
            && !regex.is_match(&text)
        {
            self.issue(
                IssueCode::Value,
                location,
                format!("not a valid {type_code}"),
            );
        }
    }

    /// `_name`: an Element (`id`, `extension`) per value, or an array of them aligned with the
    /// primitive array.
    fn primitive_extension(
        &mut self,
        value: &Value,
        paired: Option<&Value>,
        repeating: bool,
        location: &str,
    ) {
        match (value, repeating) {
            (Value::Array(items), true) => {
                let paired_len = paired.and_then(Value::as_array).map(Vec::len);
                if paired_len.is_some_and(|len| len != items.len()) {
                    self.issue(
                        IssueCode::Structure,
                        location,
                        "must have as many entries as the primitive array",
                    );
                }
                for (i, item) in items.iter().enumerate() {
                    let here = format!("{location}[{i}]");
                    match item {
                        Value::Null => {
                            let has_value =
                                paired.and_then(|p| p.get(i)).is_some_and(|p| !p.is_null());
                            if !has_value {
                                self.issue(
                                    IssueCode::Structure,
                                    &here,
                                    "a value or an extension is required",
                                );
                            }
                        }
                        other => self.complex("Element", "", other, &here),
                    }
                }
            }
            (Value::Array(_), false) => self.issue(
                IssueCode::Structure,
                location,
                "a single object is expected",
            ),
            (_, true) => self.issue(IssueCode::Structure, location, "an array is expected"),
            (other, false) => self.complex("Element", "", other, location),
        }
    }
}

/// The element a JSON key names under `base`: a direct child, or a choice `[x]` element with
/// the type its suffix names.
pub(crate) fn resolve<'a>(
    def: &'a TypeDef,
    base: &str,
    name: &str,
) -> Option<(&'a str, &'a Element, String)> {
    let direct = join(base, name);
    if let Some((path, element)) = def.elements.get_key_value(&direct)
        && !path.ends_with("[x]")
    {
        let chosen = element
            .types
            .first()
            .cloned()
            .unwrap_or_else(|| "BackboneElement".to_owned());
        return Some((path.as_str(), element, chosen));
    }
    for child in def.children.get(base).into_iter().flatten() {
        let Some(stem) = child.strip_suffix("[x]") else {
            continue;
        };
        let prefix = stem.rsplit('.').next().unwrap_or(stem);
        let Some(suffix) = name.strip_prefix(prefix) else {
            continue;
        };
        let element = def.elements.get(child)?;
        if let Some(chosen) = element.types.iter().find(|t| {
            let mut chars = t.chars();
            chars.next().is_some_and(|c| {
                let capitalized: String = c.to_uppercase().chain(chars).collect();
                capitalized == suffix
            })
        }) {
            return Some((child.as_str(), element, chosen.clone()));
        }
    }
    None
}

/// A minimal instance of `resource_type` that passes tiers 1 and 2: every required element at
/// its minimum with placeholder values, plus what the type's core invariants need
/// (a short per-type list). For tests and tooling that need a valid resource of any type;
/// never for clinical data.
pub fn skeleton(resource_type: &str) -> Option<Value> {
    let table = table();
    let def = table.types.get(resource_type)?;
    if def.kind != "resource" || def.is_abstract {
        return None;
    }
    let mut object = required_members(table, resource_type, "", 0);
    object.insert(
        "resourceType".into(),
        Value::String(resource_type.to_owned()),
    );
    if let Some(Value::Object(extra)) = invariant_members(resource_type) {
        object.extend(extra);
    }
    Some(Value::Object(object))
}

/// Members a type's skeleton needs to hold its core invariants, beyond its required elements.
fn invariant_members(resource_type: &str) -> Option<Value> {
    let placeholder = || serde_json::json!({ "extension": [{ "url": "http://example.org/x", "valueString": "x" }] });
    Some(match resource_type {
        // app-3 (no dates unless proposed), app-1 (a participant type or actor)
        "Appointment" => serde_json::json!({
            "status": "proposed",
            "participant": [{ "status": "accepted", "type": [{ "text": "x" }] }],
        }),
        // apr-1: a participant type or actor
        "AppointmentResponse" => serde_json::json!({ "participantType": [{ "text": "x" }] }),
        // cpb-1 (rest, messaging or document), cpb-2 (a description, software or implementation)
        "CapabilityStatement" => serde_json::json!({
            "document": [{ "mode": "producer", "profile": "http://example.org/x" }],
            "software": { "name": "x" },
        }),
        // imr-1: a vaccine code or target disease
        "ImmunizationRecommendation" => serde_json::json!({
            "recommendation": [{ "targetDisease": [{ "text": "x" }], "forecastStatus": { "text": "x" } }],
        }),
        // ipn-1, org-1: an identifier or a name
        "InsurancePlan" | "Organization" => serde_json::json!({ "name": "x" }),
        // lnk-1: at least two items
        "Linkage" => serde_json::json!({
            "item": [
                { "type": "source", "resource": placeholder() },
                { "type": "alternate", "resource": placeholder() },
            ],
        }),
        // sdf-6 (a snapshot or a differential), sdf-14/17 (unique element ids), sdf-8a
        // (paths start with the type)
        "StructureDefinition" => serde_json::json!({
            "type": "Patient",
            "differential": { "element": [{ "id": "Patient", "path": "Patient" }] },
        }),
        // tcp-2: a description, software or implementation
        "TerminologyCapabilities" => serde_json::json!({ "software": { "name": "x" } }),
        _ => return None,
    })
}

fn placeholder(type_code: &str) -> Value {
    match type_code {
        "boolean" => Value::Bool(true),
        "integer" | "positiveInt" | "unsignedInt" | "decimal" => Value::from(1),
        "date" => Value::from("2026-01-01"),
        "dateTime" | "instant" => Value::from("2026-01-01T00:00:00Z"),
        "time" => Value::from("08:00:00"),
        "uri" | "url" | "canonical" => Value::from("http://example.org/x"),
        "oid" => Value::from("urn:oid:1.2.3"),
        "uuid" => Value::from("urn:uuid:3c3b8f5e-4b4f-4c41-9b6a-6f6f6f6f6f6f"),
        "base64Binary" => Value::from("eA=="),
        "integer64" => Value::from("1"),
        "xhtml" => Value::from("<div xmlns=\"http://www.w3.org/1999/xhtml\">x</div>"),
        _ => Value::from("x"),
    }
}

/// An element that is only an extension: valid for any datatype, and a Reference without a
/// `reference` never fails reference integrity.
fn extension_only() -> Value {
    serde_json::json!({ "extension": [{ "url": "http://example.org/x", "valueString": "x" }] })
}

fn required_members(
    table: &Table,
    type_name: &str,
    base: &str,
    depth: usize,
) -> Map<String, Value> {
    let mut members = Map::new();
    let Some(def) = table.types.get(type_name) else {
        return members;
    };
    for child in def.children.get(base).into_iter().flatten() {
        let Some(element) = def.elements.get(child) else {
            continue;
        };
        if element.min == 0 {
            continue;
        }
        let name = child.rsplit('.').next().unwrap_or(child);
        let (key, type_code) = match name.strip_suffix("[x]") {
            Some(stem) => {
                let first = element.types.first().map_or("string", String::as_str);
                let mut chars = first.chars();
                let suffix: String = chars
                    .next()
                    .map(|c| c.to_uppercase().chain(chars).collect())
                    .unwrap_or_default();
                (format!("{stem}{suffix}"), first.to_owned())
            }
            None => (
                name.to_owned(),
                element.types.first().cloned().unwrap_or_default(),
            ),
        };
        let value = if is_primitive(&type_code) {
            placeholder(&type_code)
        } else if depth > 4 {
            extension_only()
        } else {
            let inline = element.content_reference.clone().or_else(|| {
                (matches!(type_code.as_str(), "BackboneElement" | "Element")
                    && def.children.contains_key(child.as_str()))
                .then(|| child.clone())
            });
            let nested = match &inline {
                Some(path) => required_members(table, type_name, path, depth + 1),
                None => required_members(table, &type_code, "", depth + 1),
            };
            if nested.is_empty() {
                extension_only()
            } else {
                Value::Object(nested)
            }
        };
        let repeating = element.max != Some(1);
        members.insert(
            key,
            if repeating {
                Value::Array(vec![value])
            } else {
                value
            },
        );
    }
    members
}

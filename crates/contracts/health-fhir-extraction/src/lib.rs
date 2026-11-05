//! SDC template extraction (questionnaire-extraction.md; ADR 0022, 0023; #93): the resources a
//! completed QuestionnaireResponse produces under an Extraction Mapping, as a **plan**. Nothing is
//! written here: the `CompleteQuestionnaireResponse` use case hands each planned resource to its
//! Owner's use case, in one transaction.
//!
//! - **The mapping** ([`Mapping::from_library`]) is an `ExtractionMappingLibrary`: contained
//!   resource templates, each with the platform's `extraction-template` extension (mode,
//!   retraction, the Owner's use case).
//! - **Templates** carry SDC's `templateExtractContext` and `templateExtractValue`, holding
//!   FHIRPath:
//!   - a `templateExtractContext` on the template itself gives one resource per result, each
//!     with that result as its context; without one, the template gives one resource with the
//!     response as its context;
//!   - a `templateExtractContext` on an element repeats it once per result, each with that
//!     result as its context, and removes it if there is none;
//!   - a `templateExtractValue` sets a primitive (on its `_element`) or a complex element (in its
//!     `extension`) to the results; an empty result removes it, and several results repeat it
//!     where the element repeats (anything else is refused);
//!   - an element left empty is removed.
//!
//!   Relative paths start at the context, and `%resource` is the response. The response is the
//!   only input: there is no `now()`, query or `launchContext`, so extraction is a pure function
//!   of the response and the mapping, and the authority can re-derive it.
//! - **Ids** are derived (identifiers.md): `{response}|{mapping canonical}|{linkId path}#{repeat
//!   key}|{type}`. The repeat key is the item's `id`, else its index among repeats. A template
//!   whose context is not a response item is keyed by its own id (and the result's index).
//! - **Decimals keep their source scale** (ADR 0052): `1.50` is written as `1.50`.
//! - Every resource is tagged with its mode ([`EXTRACTION_TAG_SYSTEM`]), and an Observation or
//!   DocumentReference is `derivedFrom` the response.

use std::collections::BTreeSet;
use std::future::Future;
use std::pin::Pin;

use health_fhir_validation::{FhirPathEngine, FhirPathItem};
use health_identifiers::{DerivationName, ResourceId, derive_id};
use serde_json::{Map, Value, json};

pub mod gates;

/// SDC's `templateExtractContext` extension.
pub const TEMPLATE_EXTRACT_CONTEXT: &str =
    "http://hl7.org/fhir/uv/sdc/StructureDefinition/sdc-questionnaire-templateExtractContext";
/// SDC's `templateExtractValue` extension.
pub const TEMPLATE_EXTRACT_VALUE: &str =
    "http://hl7.org/fhir/uv/sdc/StructureDefinition/sdc-questionnaire-templateExtractValue";
/// The platform's `extraction-template` extension: a template's mode, retraction and Owner.
pub const EXTRACTION_TEMPLATE: &str = "https://carapai.github.io/people-centered-healthcare/fhir/StructureDefinition/extraction-template";
/// The Questionnaire canonical (no version) a mapping applies to.
pub const TARGET_QUESTIONNAIRE: &str = "https://carapai.github.io/people-centered-healthcare/fhir/StructureDefinition/extraction-target-questionnaire";
/// The semver range of Questionnaire versions a mapping applies to.
pub const TARGET_VERSIONS: &str = "https://carapai.github.io/people-centered-healthcare/fhir/StructureDefinition/extraction-target-versions";
/// The tag system on every extracted resource; the code is its mode.
pub const EXTRACTION_TAG_SYSTEM: &str = "urn:health:extraction";

/// How a template governs what it produces (ADR 0023).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// The response stays the source of truth: re-extraction versions it, and it is retracted
    /// when it disappears from the output.
    Managed,
    /// Created once; its own workflow owns it from then on.
    Seed,
}

impl Mode {
    /// The platform code.
    pub fn code(self) -> &'static str {
        match self {
            Self::Managed => "managed",
            Self::Seed => "seed",
        }
    }
}

/// One contained template of a mapping.
#[derive(Debug, Clone, PartialEq)]
pub struct Template {
    /// Its contained id.
    pub id: String,
    /// The resource type it produces.
    pub resource_type: String,
    /// Its mode.
    pub mode: Mode,
    /// The status a managed target takes when it disappears from the output.
    pub retraction: Option<String>,
    /// The Owner's use case that accepts what it produces.
    pub owner_use_case: String,
    /// Its own `templateExtractContext`, if any.
    context: Option<String>,
    /// The template, without its governing extensions.
    body: Value,
}

/// An Extraction Mapping.
#[derive(Debug, Clone, PartialEq)]
pub struct Mapping {
    /// Its canonical URL.
    pub url: String,
    /// Its version.
    pub version: String,
    /// The Questionnaire canonical it applies to, without a version.
    pub questionnaire: String,
    /// The semver range of Questionnaire versions it applies to.
    pub versions: String,
    /// Its templates, in order.
    pub templates: Vec<Template>,
}

/// Why a Library is not a usable Extraction Mapping.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MappingError {
    /// Not a Library of type `extraction-mapping`.
    #[error("not an Extraction Mapping Library")]
    NotAMapping,
    /// A required element is missing.
    #[error("the mapping has no {0}")]
    Missing(&'static str),
    /// A template is unusable.
    #[error("template {template}: {reason}")]
    Template {
        /// The template's id.
        template: String,
        /// Why.
        reason: String,
    },
}

/// Why a response could not be extracted. Completion fails as a whole (ADR 0022).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ExtractionError {
    /// The response has no id the derived ids can be named from.
    #[error("the response has no resource id")]
    NoResponseId,
    /// An expression could not be evaluated.
    #[error("template {template}: `{expression}` failed: {reason}")]
    Expression {
        /// The template.
        template: String,
        /// The expression.
        expression: String,
        /// Why.
        reason: String,
    },
    /// Results that do not fit the element they are written to.
    #[error("template {template}: {path}: {reason}")]
    Shape {
        /// The template.
        template: String,
        /// The element.
        path: String,
        /// Why.
        reason: String,
    },
    /// Two planned resources would have the same id.
    #[error("two planned resources share the id {0}")]
    Collision(String),
}

/// One resource the response produces, for its Owner's use case.
#[derive(Debug, Clone, PartialEq)]
pub struct Planned {
    /// The template that produced it.
    pub template: String,
    /// Its type.
    pub resource_type: String,
    /// Its derived id.
    pub id: ResourceId,
    /// The resource, with its id, tag and `derivedFrom`.
    pub resource: Value,
    /// Its mode.
    pub mode: Mode,
    /// The status it takes if it later disappears (managed only).
    pub retraction: Option<String>,
    /// The Owner's use case that accepts it.
    pub owner_use_case: String,
}

fn extensions(value: &Value) -> impl Iterator<Item = &Value> {
    value
        .get("extension")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
}

fn extension<'a>(value: &'a Value, url: &str) -> Option<&'a Value> {
    extensions(value).find(|e| e.get("url").and_then(Value::as_str) == Some(url))
}

/// The FHIRPath of an SDC extension (`valueString`, or `valueExpression.expression`).
fn expression_of(value: &Value, url: &str) -> Option<String> {
    let found = extension(value, url)?;
    found
        .get("valueString")
        .or_else(|| found.pointer("/valueExpression/expression"))
        .and_then(Value::as_str)
        .map(str::to_owned)
}

const GOVERNING: [&str; 3] = [
    TEMPLATE_EXTRACT_CONTEXT,
    TEMPLATE_EXTRACT_VALUE,
    EXTRACTION_TEMPLATE,
];

/// `value` without the extraction extensions; `extension` goes if nothing else is left in it.
fn strip(value: &Value) -> Value {
    let Some(object) = value.as_object() else {
        return value.clone();
    };
    let mut out = object.clone();
    if let Some(list) = object.get("extension").and_then(Value::as_array) {
        let kept: Vec<Value> = list
            .iter()
            .filter(|e| {
                !e.get("url")
                    .and_then(Value::as_str)
                    .is_some_and(|u| GOVERNING.contains(&u))
            })
            .cloned()
            .collect();
        if kept.is_empty() {
            out.remove("extension");
        } else {
            out.insert("extension".into(), Value::Array(kept));
        }
    }
    Value::Object(out)
}

fn sub_extension<'a>(extension: &'a Value, url: &str) -> Option<&'a Value> {
    extensions(extension).find(|e| e.get("url").and_then(Value::as_str) == Some(url))
}

impl Template {
    fn parse(contained: &Value) -> Result<Self, MappingError> {
        let id = contained
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let refused = |reason: &str| MappingError::Template {
            template: id.clone(),
            reason: reason.to_owned(),
        };
        if id.is_empty() {
            return Err(refused("a contained template needs an id"));
        }
        let resource_type = contained
            .get("resourceType")
            .and_then(Value::as_str)
            .ok_or_else(|| refused("no resourceType"))?
            .to_owned();
        let governing = extension(contained, EXTRACTION_TEMPLATE)
            .ok_or_else(|| refused("no extraction-template extension"))?;
        let mode = match sub_extension(governing, "mode")
            .and_then(|e| e.get("valueCode"))
            .and_then(Value::as_str)
        {
            Some("managed") => Mode::Managed,
            Some("seed") => Mode::Seed,
            _ => return Err(refused("its mode is managed or seed")),
        };
        let retraction = sub_extension(governing, "retraction")
            .and_then(|e| e.get("valueCode"))
            .and_then(Value::as_str)
            .map(str::to_owned);
        if mode == Mode::Managed && retraction.is_none() {
            return Err(refused("a managed template names its retraction status"));
        }
        let owner_use_case = sub_extension(governing, "ownerUseCase")
            .and_then(|e| e.get("valueString"))
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| refused("no ownerUseCase"))?
            .to_owned();
        let context = expression_of(contained, TEMPLATE_EXTRACT_CONTEXT);
        let mut body = strip(contained);
        if let Some(object) = body.as_object_mut() {
            object.remove("id");
        }
        Ok(Self {
            id,
            resource_type,
            mode,
            retraction,
            owner_use_case,
            context,
            body,
        })
    }
}

impl Mapping {
    /// The mapping an `ExtractionMappingLibrary` holds.
    pub fn from_library(library: &Value) -> Result<Self, MappingError> {
        let is_mapping = library.get("resourceType").and_then(Value::as_str) == Some("Library")
            && library
                .pointer("/type/coding")
                .and_then(Value::as_array)
                .is_some_and(|c| {
                    c.iter().any(|c| {
                        c.get("code").and_then(Value::as_str) == Some("extraction-mapping")
                    })
                });
        if !is_mapping {
            return Err(MappingError::NotAMapping);
        }
        let text = |path: &str, what: &'static str| {
            library
                .pointer(path)
                .and_then(Value::as_str)
                .map(str::to_owned)
                .ok_or(MappingError::Missing(what))
        };
        let url = text("/url", "url")?;
        let version = text("/version", "version")?;
        let questionnaire = extension(library, TARGET_QUESTIONNAIRE)
            .and_then(|e| e.get("valueUri"))
            .and_then(Value::as_str)
            .ok_or(MappingError::Missing("target Questionnaire"))?
            .to_owned();
        let versions = extension(library, TARGET_VERSIONS)
            .and_then(|e| e.get("valueString"))
            .and_then(Value::as_str)
            .ok_or(MappingError::Missing("target versions"))?
            .to_owned();
        let templates = library
            .get("contained")
            .and_then(Value::as_array)
            .filter(|c| !c.is_empty())
            .ok_or(MappingError::Missing("templates"))?
            .iter()
            .map(Template::parse)
            .collect::<Result<Vec<_>, _>>()?;
        let mut ids = BTreeSet::new();
        for template in &templates {
            if !ids.insert(&template.id) {
                return Err(MappingError::Template {
                    template: template.id.clone(),
                    reason: "two templates share this id".into(),
                });
            }
        }
        Ok(Self {
            url,
            version,
            questionnaire,
            versions,
            templates,
        })
    }

    /// `url|version`, as the response records it.
    pub fn canonical(&self) -> String {
        format!("{}|{}", self.url, self.version)
    }

    /// Every FHIRPath expression of the mapping, for the publication checks.
    pub fn expressions(&self) -> Vec<String> {
        self.expressions_by_template()
            .into_iter()
            .map(|e| e.expression)
            .collect()
    }

    /// Every FHIRPath expression of the mapping, with its template and kind.
    pub fn expressions_by_template(&self) -> Vec<Expression> {
        fn walk(template: &str, value: &Value, out: &mut Vec<Expression>) {
            match value {
                Value::Object(object) => {
                    for (url, kind) in [
                        (TEMPLATE_EXTRACT_CONTEXT, ExpressionKind::Context),
                        (TEMPLATE_EXTRACT_VALUE, ExpressionKind::Value),
                    ] {
                        if let Some(found) = expression_of(value, url) {
                            out.push(Expression {
                                template: template.to_owned(),
                                kind,
                                expression: found,
                            });
                        }
                    }
                    object.values().for_each(|v| walk(template, v, out));
                }
                Value::Array(list) => list.iter().for_each(|v| walk(template, v, out)),
                _ => {}
            }
        }
        let mut out = Vec::new();
        for template in &self.templates {
            if let Some(context) = &template.context {
                out.push(Expression {
                    template: template.id.clone(),
                    kind: ExpressionKind::Context,
                    expression: context.clone(),
                });
            }
            walk(&template.id, &template.body, &mut out);
        }
        out
    }
}

/// What an expression of a mapping does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpressionKind {
    /// A `templateExtractContext`: what a resource or element repeats over.
    Context,
    /// A `templateExtractValue`: the value written.
    Value,
}

/// An expression of a mapping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expression {
    /// Its template.
    pub template: String,
    /// What it does.
    pub kind: ExpressionKind,
    /// The FHIRPath.
    pub expression: String,
}

/// A response item: its linkId path and repeat key.
struct Located {
    item: Value,
    path: String,
    key: Option<String>,
}

fn locate(items: Option<&Value>, prefix: &str, out: &mut Vec<Located>) {
    let Some(items) = items.and_then(Value::as_array) else {
        return;
    };
    for (position, item) in items.iter().enumerate() {
        let link = item.get("linkId").and_then(Value::as_str).unwrap_or("");
        let path = if prefix.is_empty() {
            link.to_owned()
        } else {
            format!("{prefix}/{link}")
        };
        let repeats = items
            .iter()
            .filter(|i| i.get("linkId").and_then(Value::as_str) == Some(link))
            .count()
            > 1;
        let index = items[..position]
            .iter()
            .filter(|i| i.get("linkId").and_then(Value::as_str) == Some(link))
            .count();
        let key = item
            .get("id")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .or_else(|| repeats.then(|| index.to_string()));
        out.push(Located {
            item: item.clone(),
            path: path.clone(),
            key,
        });
        locate(item.get("item"), &path, out);
        for answer in item
            .get("answer")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            locate(answer.get("item"), &path, out);
        }
    }
}

type Filled<'a> = Pin<Box<dyn Future<Output = Result<Vec<Value>, ExtractionError>> + Send + 'a>>;

struct Run<'a> {
    engine: &'a dyn FhirPathEngine,
    response: &'a Value,
    template: &'a Template,
}

/// One FHIRPath result as JSON.
fn to_json(item: &FhirPathItem) -> Value {
    match item {
        FhirPathItem::Boolean(b) => Value::Bool(*b),
        FhirPathItem::Integer(i) => json!(i),
        FhirPathItem::Decimal(text) => decimal(text),
        FhirPathItem::String(s) | FhirPathItem::Temporal(s) => Value::String(s.clone()),
        FhirPathItem::Quantity { value, unit } => {
            let mut quantity = Map::new();
            quantity.insert("value".into(), decimal(value));
            if let Some(unit) = unit {
                quantity.insert("unit".into(), Value::String(unit.clone()));
            }
            Value::Object(quantity)
        }
        FhirPathItem::Element(value) => value.clone(),
    }
}

/// A decimal as an exact JSON number, in its scale.
fn decimal(text: &str) -> Value {
    serde_json::from_str::<serde_json::Number>(text)
        .map_or_else(|_| Value::String(text.to_owned()), Value::Number)
}

impl Run<'_> {
    async fn evaluate(
        &self,
        context: &str,
        expression: &str,
    ) -> Result<Vec<FhirPathItem>, ExtractionError> {
        let composed = vec![format!("({context}).select({expression})")];
        let mut results = self.engine.evaluate_many(self.response, &composed).await;
        results
            .pop()
            .unwrap_or_else(|| Ok(Vec::new()))
            .map_err(|failure| ExtractionError::Expression {
                template: self.template.id.clone(),
                expression: expression.to_owned(),
                reason: failure.0,
            })
    }

    fn shape(&self, path: &str, reason: &str) -> ExtractionError {
        ExtractionError::Shape {
            template: self.template.id.clone(),
            path: path.to_owned(),
            reason: reason.to_owned(),
        }
    }

    /// The copies of the element `value` at `path` in `context`: none, one, or one per result.
    fn element<'b>(&'b self, value: &'b Value, context: String, path: String) -> Filled<'b> {
        Box::pin(async move {
            match value {
                Value::Object(_) => {
                    if let Some(expression) = expression_of(value, TEMPLATE_EXTRACT_CONTEXT) {
                        let count = self.evaluate(&context, &expression).await?.len();
                        let mut out = Vec::new();
                        for index in 0..count {
                            let inner = format!("(({context}).select({expression}))[{index}]");
                            out.extend(self.object(value, inner, &path).await?);
                        }
                        return Ok(out);
                    }
                    if let Some(expression) = expression_of(value, TEMPLATE_EXTRACT_VALUE) {
                        return Ok(self
                            .evaluate(&context, &expression)
                            .await?
                            .iter()
                            .map(to_json)
                            .collect());
                    }
                    self.object(value, context, &path).await
                }
                Value::Array(list) => {
                    let mut out = Vec::new();
                    for (index, entry) in list.iter().enumerate() {
                        out.extend(
                            self.element(entry, context.clone(), format!("{path}[{index}]"))
                                .await?,
                        );
                    }
                    Ok(out)
                }
                other => Ok(vec![other.clone()]),
            }
        })
    }

    /// The object `value` filled in `context`; nothing if it is left empty.
    async fn object(
        &self,
        value: &Value,
        context: String,
        path: &str,
    ) -> Result<Vec<Value>, ExtractionError> {
        let source = strip(value);
        let Some(fields) = source.as_object() else {
            return Ok(vec![source]);
        };
        // Each element: its own key, or only its `_key` when SDC leaves the placeholder out.
        let names: Vec<&str> = fields
            .keys()
            .filter_map(|k| match k.strip_prefix('_') {
                Some(name) if !fields.contains_key(name) => Some(name),
                Some(_) => None,
                None => Some(k.as_str()),
            })
            .collect();
        let mut out = Map::new();
        for key in names {
            let field = fields.get(key).unwrap_or(&Value::Null);
            let key = key.to_owned();
            let here = format!("{path}.{key}");
            let shadow = fields.get(&format!("_{key}"));
            let produced = match shadow.and_then(|s| expression_of(s, TEMPLATE_EXTRACT_VALUE)) {
                Some(expression) => {
                    let values: Vec<Value> = self
                        .evaluate(&context, &expression)
                        .await?
                        .iter()
                        .map(to_json)
                        .collect();
                    if values.iter().any(|v| v.is_object() || v.is_array()) {
                        return Err(self.shape(&here, "a primitive set to a complex value"));
                    }
                    values
                }
                None if field.is_null() => Vec::new(),
                None => self.element(field, context.clone(), here.clone()).await?,
            };
            if let Some(kept) = shadow
                .map(strip)
                .filter(|s| s.as_object().is_some_and(|o| !o.is_empty()))
            {
                out.insert(format!("_{key}"), kept);
            }
            match (field.is_array(), produced.len()) {
                (_, 0) => {
                    out.remove(&format!("_{key}"));
                }
                (true, _) => {
                    out.insert(key.clone(), Value::Array(produced));
                }
                (false, 1) => {
                    out.insert(
                        key.clone(),
                        produced.into_iter().next().unwrap_or(Value::Null),
                    );
                }
                (false, _) => {
                    return Err(
                        self.shape(&here, "several values for an element that does not repeat")
                    );
                }
            }
        }
        Ok(if out.is_empty() {
            Vec::new()
        } else {
            vec![Value::Object(out)]
        })
    }
}

/// The resources `response` produces under `mapping`, in template order.
pub async fn extract(
    engine: &dyn FhirPathEngine,
    response: &Value,
    mapping: &Mapping,
) -> Result<Vec<Planned>, ExtractionError> {
    let response_id: ResourceId = response
        .get("id")
        .and_then(Value::as_str)
        .and_then(|id| id.parse().ok())
        .ok_or(ExtractionError::NoResponseId)?;
    let mut located = Vec::new();
    locate(response.get("item"), "", &mut located);
    let mut planned = Vec::new();
    let mut ids = BTreeSet::new();
    for template in &mapping.templates {
        let run = Run {
            engine,
            response,
            template,
        };
        // Each context, with the linkId path and repeat key its resource is named by.
        let mut contexts = Vec::new();
        match &template.context {
            None => contexts.push(("%resource".to_owned(), template.id.clone(), None)),
            Some(expression) => {
                let results = run.evaluate("%resource", expression).await?;
                let mut used = BTreeSet::new();
                for (index, result) in results.iter().enumerate() {
                    let context = format!("(%resource.select({expression}))[{index}]");
                    let found = match result {
                        FhirPathItem::Element(value) => located
                            .iter()
                            .enumerate()
                            .find(|(at, l)| !used.contains(at) && l.item == *value),
                        _ => None,
                    };
                    match found {
                        Some((at, l)) => {
                            used.insert(at);
                            contexts.push((context, l.path.clone(), l.key.clone()));
                        }
                        None => {
                            contexts.push((context, template.id.clone(), Some(index.to_string())))
                        }
                    }
                }
            }
        }
        for (context, path, key) in contexts {
            let Some(mut resource) = run
                .object(&template.body, context, &template.resource_type)
                .await?
                .pop()
            else {
                continue;
            };
            let id = derive_id(&DerivationName::extraction(
                response_id,
                &mapping.url,
                &path,
                key.as_deref(),
                &template.resource_type,
            ));
            if !ids.insert(id.to_string()) {
                return Err(ExtractionError::Collision(id.to_string()));
            }
            finish(&mut resource, template, id, response_id);
            planned.push(Planned {
                template: template.id.clone(),
                resource_type: template.resource_type.clone(),
                id,
                resource,
                mode: template.mode,
                retraction: template.retraction.clone(),
                owner_use_case: template.owner_use_case.clone(),
            });
        }
    }
    Ok(planned)
}

/// Sets the resource's type, id, mode tag and, where it has one, `derivedFrom`.
fn finish(resource: &mut Value, template: &Template, id: ResourceId, response: ResourceId) {
    let Some(object) = resource.as_object_mut() else {
        return;
    };
    object.insert(
        "resourceType".into(),
        Value::String(template.resource_type.clone()),
    );
    object.insert("id".into(), Value::String(id.to_string()));
    let meta = object
        .entry("meta")
        .or_insert_with(|| Value::Object(Map::new()));
    if let Some(meta) = meta.as_object_mut() {
        let tags = meta
            .entry("tag")
            .or_insert_with(|| Value::Array(Vec::new()));
        if let Some(tags) = tags.as_array_mut() {
            tags.push(json!({ "system": EXTRACTION_TAG_SYSTEM, "code": template.mode.code() }));
        }
    }
    if matches!(
        template.resource_type.as_str(),
        "Observation" | "DocumentReference"
    ) {
        let derived = object
            .entry("derivedFrom")
            .or_insert_with(|| Value::Array(Vec::new()));
        if let Some(derived) = derived.as_array_mut() {
            derived.push(json!({ "reference": format!("QuestionnaireResponse/{response}") }));
        }
    }
}

/// The plan as SDC's `$extract` returns it: a transaction Bundle of `PUT`s to the derived ids,
/// for the dry run and for fixtures.
pub fn bundle(plan: &[Planned]) -> Value {
    let entries: Vec<Value> = plan
        .iter()
        .map(|p| {
            json!({
                "fullUrl": format!("urn:uuid:{}", p.id),
                "resource": p.resource,
                "request": { "method": "PUT", "url": format!("{}/{}", p.resource_type, p.id) }
            })
        })
        .collect();
    let mut bundle = json!({ "resourceType": "Bundle", "type": "transaction" });
    // FHIR has no empty arrays: a plan that writes nothing is a Bundle without entries.
    if !entries.is_empty() {
        bundle["entry"] = Value::Array(entries);
    }
    bundle
}

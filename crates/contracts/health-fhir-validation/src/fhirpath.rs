//! FHIRPath behind the `FhirPathEngine` trait (fhir-r5-model.md § FHIRPath), with the
//! decimal-exact `octofhir-fhirpath` fork as the default implementation (ADR 0052).
//!
//! The engine's model provider is [`TableModelProvider`], built from the same generated element
//! table as tier 1. It does not use `octofhir-fhirschema`, whose package manager would add
//! about 120 crates (an HTTP client, SQLite, a CLI) to every node.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use async_trait::async_trait;
use octofhir_fhirpath::core::model_provider::{
    ElementInfo, FhirVersion, ModelError, ModelProvider, TypeInfo,
};
use octofhir_fhirpath::core::trace::{SharedTraceProvider, TraceProvider};
use octofhir_fhirpath::core::value::utils::json_to_fhirpath_value;
use octofhir_fhirpath::evaluator::EvaluationContext;
use octofhir_fhirpath::{Collection, FhirPathValue, create_function_registry};
use serde_json::Value;

use crate::structure::{Table, is_primitive, table};

/// One item of a FHIRPath result.
#[derive(Debug, Clone, PartialEq)]
pub enum FhirPathItem {
    /// A Boolean.
    Boolean(bool),
    /// An Integer.
    Integer(i64),
    /// A Decimal, as exact text in its scale (`1.50`).
    Decimal(String),
    /// A String, or a FHIR primitive that maps to one (`code`, `uri`, …).
    String(String),
    /// A Date, DateTime or Time, as written.
    Temporal(String),
    /// A Quantity: the exact value and its unit.
    Quantity {
        /// The value, as exact text.
        value: String,
        /// The unit code or text.
        unit: Option<String>,
    },
    /// An element or resource, as JSON.
    Element(Value),
}

/// An expression that could not be evaluated: a syntax error, an unsupported function, or a
/// decimal beyond 28 significant digits (which fails closed, ADR 0052).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("FHIRPath evaluation failed: {0}")]
pub struct FhirPathFailure(pub String);

/// The results of evaluating several expressions against one resource, in order.
pub type Results = Vec<Result<Vec<FhirPathItem>, FhirPathFailure>>;

/// A FHIRPath engine. Expressions are evaluated with `resource` as the root and `%resource`.
pub trait FhirPathEngine: Send + Sync {
    /// Evaluates each of `expressions` against `resource`.
    fn evaluate_many<'a>(
        &'a self,
        resource: &'a Value,
        expressions: &'a [String],
    ) -> Pin<Box<dyn Future<Output = Results> + Send + 'a>>;
}

/// The default engine: the vendored, decimal-exact `octofhir-fhirpath`, built on first use.
pub struct OctofhirEngine {
    engine: tokio::sync::OnceCell<Arc<octofhir_fhirpath::FhirPathEngine>>,
    provider: Arc<TableModelProvider>,
}

impl Default for OctofhirEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl OctofhirEngine {
    /// An engine over the R5 element table. Construction is cheap; the engine is built on the
    /// first evaluation.
    pub fn new() -> Self {
        Self {
            engine: tokio::sync::OnceCell::new(),
            provider: Arc::new(TableModelProvider { table: table() }),
        }
    }

    async fn engine(&self) -> Result<Arc<octofhir_fhirpath::FhirPathEngine>, FhirPathFailure> {
        self.engine
            .get_or_try_init(|| async {
                octofhir_fhirpath::FhirPathEngine::new(
                    Arc::new(create_function_registry()),
                    self.provider.clone(),
                )
                .await
                .map(Arc::new)
                .map_err(|e| FhirPathFailure(e.to_string()))
            })
            .await
            .cloned()
    }
}

fn item(value: &FhirPathValue) -> FhirPathItem {
    match value {
        FhirPathValue::Boolean(b, ..) => FhirPathItem::Boolean(*b),
        FhirPathValue::Integer(i, ..) => FhirPathItem::Integer(*i),
        FhirPathValue::Decimal(d, ..) => FhirPathItem::Decimal(d.to_string()),
        FhirPathValue::String(s, ..) => FhirPathItem::String(s.clone()),
        FhirPathValue::Date(..) | FhirPathValue::DateTime(..) | FhirPathValue::Time(..) => {
            match value.to_json_value() {
                Value::String(s) => FhirPathItem::Temporal(s),
                other => FhirPathItem::Temporal(other.to_string()),
            }
        }
        FhirPathValue::Quantity { value, unit, .. } => FhirPathItem::Quantity {
            value: value.to_string(),
            unit: unit.clone(),
        },
        other => FhirPathItem::Element(other.to_json_value()),
    }
}

impl FhirPathEngine for OctofhirEngine {
    fn evaluate_many<'a>(
        &'a self,
        resource: &'a Value,
        expressions: &'a [String],
    ) -> Pin<Box<dyn Future<Output = Results> + Send + 'a>> {
        Box::pin(async move {
            let fail_all = |failure: FhirPathFailure| -> Results {
                expressions.iter().map(|_| Err(failure.clone())).collect()
            };
            let engine = match self.engine().await {
                Ok(engine) => engine,
                Err(failure) => return fail_all(failure),
            };
            let provider = self.provider.clone();
            let resource = resource.clone();
            let owned: Vec<String> = expressions.to_vec();
            // The engine is third-party code on the write path: run it in its own task, so a
            // panic inside it fails the evaluation closed instead of dropping the request.
            let task = tokio::spawn(async move {
                let context = EvaluationContext::new(
                    Collection::single(json_to_fhirpath_value(resource)),
                    provider,
                    None,
                    None,
                    Some(Arc::new(Silent) as SharedTraceProvider),
                );
                let mut results = Vec::with_capacity(owned.len());
                for expression in &owned {
                    results.push(
                        engine
                            .evaluate(expression, &context)
                            .await
                            .map(|r| r.value.iter().map(item).collect())
                            .map_err(|e| FhirPathFailure(e.to_string())),
                    );
                }
                results
            });
            match task.await {
                Ok(results) => results,
                Err(error) => fail_all(FhirPathFailure(format!(
                    "the FHIRPath engine panicked: {error}"
                ))),
            }
        })
    }
}

/// Discards `trace()` output. Core invariants call `trace()` (`dom-3`, `ref-1`), and the
/// engine's default writes the traced values (ids, references) to stderr, which the node's
/// collector would ship as logs (ADR 0047).
struct Silent;

impl TraceProvider for Silent {
    fn trace(&self, _name: &str, _index: usize, _message: &str) {}

    fn trace_simple(&self, _name: &str, _message: &str) {}

    fn collect_traces(&self) -> Vec<String> {
        Vec::new()
    }

    fn clear_traces(&self) {}
}

/// FHIR primitive types and the FHIRPath System types they map to.
const SYSTEM_TYPES: &[(&str, &str)] = &[
    ("boolean", "Boolean"),
    ("integer", "Integer"),
    ("integer64", "Integer"),
    ("positiveInt", "Integer"),
    ("unsignedInt", "Integer"),
    ("decimal", "Decimal"),
    ("date", "Date"),
    ("dateTime", "DateTime"),
    ("instant", "DateTime"),
    ("time", "Time"),
];

const QUANTITY_TYPES: &[&str] = &["Quantity", "Age", "Count", "Distance", "Duration", "Money"];

fn system_type(fhir_type: &str) -> &'static str {
    SYSTEM_TYPES
        .iter()
        .find(|(fhir, _)| *fhir == fhir_type)
        .map_or("String", |(_, system)| system)
}

/// The FHIRPath model of R5, from the generated element table. Backbone elements are named
/// by path (`Patient.contact`), as the engine expects.
#[derive(Debug)]
pub struct TableModelProvider {
    table: &'static Table,
}

impl std::fmt::Debug for Table {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Table({} types)", self.types.len())
    }
}

impl TableModelProvider {
    fn info(&self, fhir_type: &str, singleton: bool) -> TypeInfo {
        let type_name = if is_primitive(fhir_type) {
            system_type(fhir_type).to_owned()
        } else if QUANTITY_TYPES.contains(&fhir_type) {
            "Quantity".to_owned()
        } else {
            "Any".to_owned()
        };
        TypeInfo {
            type_name,
            singleton: Some(singleton),
            is_empty: Some(false),
            namespace: Some("FHIR".to_owned()),
            name: Some(fhir_type.to_owned()),
        }
    }

    /// A type name, or `Type.path` for a backbone element, as (type, element path).
    fn split<'a>(&self, name: &'a str) -> (&'a str, &'a str) {
        name.split_once('.').unwrap_or((name, ""))
    }

    fn derives(&self, derived: &str, base: &str) -> bool {
        self.table.lineage(derived).any(|t| t == base)
    }
}

#[async_trait]
impl ModelProvider for TableModelProvider {
    async fn get_type(&self, type_name: &str) -> Result<Option<TypeInfo>, ModelError> {
        if self.table.types.contains_key(type_name) || self.table.primitives.contains_key(type_name)
        {
            return Ok(Some(self.info(type_name, true)));
        }
        let system = SYSTEM_TYPES.iter().any(|(_, s)| *s == type_name)
            || matches!(type_name, "String" | "Quantity" | "Any");
        Ok(system.then(|| TypeInfo::system_type(type_name.to_owned(), true)))
    }

    async fn get_element_type(
        &self,
        parent_type: &TypeInfo,
        property_name: &str,
    ) -> Result<Option<TypeInfo>, ModelError> {
        let Some(name) = &parent_type.name else {
            return Ok(None);
        };
        let (type_name, base) = self.split(name);
        let Some(def) = self.table.types.get(type_name) else {
            return Ok(None);
        };
        let Some((path, element, chosen)) = crate::structure::resolve(def, base, property_name)
        else {
            return Ok(None);
        };
        let singleton = element.max == Some(1);
        let inline = element.content_reference.clone().or_else(|| {
            (matches!(chosen.as_str(), "BackboneElement" | "Element")
                && def.children.contains_key(path))
            .then(|| path.to_owned())
        });
        Ok(Some(match inline {
            Some(path) => TypeInfo {
                type_name: "Any".to_owned(),
                singleton: Some(singleton),
                is_empty: Some(false),
                namespace: Some("FHIR".to_owned()),
                name: Some(format!("{type_name}.{path}")),
            },
            None => self.info(&chosen, singleton),
        }))
    }

    fn of_type(&self, type_info: &TypeInfo, target_type: &str) -> Option<TypeInfo> {
        let target = target_type.rsplit('.').next().unwrap_or(target_type);
        let matches = type_info.type_name == target
            || type_info
                .name
                .as_deref()
                .is_some_and(|n| n == target || self.derives(n, target));
        matches.then(|| type_info.clone())
    }

    fn get_element_names(&self, parent_type: &TypeInfo) -> Vec<String> {
        let Some(name) = &parent_type.name else {
            return Vec::new();
        };
        let (type_name, base) = self.split(name);
        self.table
            .types
            .get(type_name)
            .and_then(|d| d.children.get(base))
            .map(|children| {
                children
                    .iter()
                    .map(|c| {
                        c.rsplit('.')
                            .next()
                            .unwrap_or(c)
                            .trim_end_matches("[x]")
                            .to_owned()
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    async fn get_children_type(
        &self,
        parent_type: &TypeInfo,
    ) -> Result<Option<TypeInfo>, ModelError> {
        Ok((!parent_type.singleton.unwrap_or(true)).then(|| TypeInfo {
            singleton: Some(true),
            ..parent_type.clone()
        }))
    }

    async fn get_elements(&self, type_name: &str) -> Result<Vec<ElementInfo>, ModelError> {
        let Some(def) = self.table.types.get(type_name) else {
            return Ok(Vec::new());
        };
        Ok(def
            .children
            .get("")
            .into_iter()
            .flatten()
            .filter_map(|path| {
                let element = def.elements.get(path)?;
                Some(ElementInfo {
                    name: path.trim_end_matches("[x]").to_owned(),
                    element_type: element
                        .types
                        .first()
                        .cloned()
                        .unwrap_or_else(|| "BackboneElement".to_owned()),
                    documentation: None,
                })
            })
            .collect())
    }

    async fn get_resource_types(&self) -> Result<Vec<String>, ModelError> {
        Ok(self
            .table
            .types
            .iter()
            .filter(|(_, d)| d.kind == "resource")
            .map(|(n, _)| n.clone())
            .collect())
    }

    async fn get_complex_types(&self) -> Result<Vec<String>, ModelError> {
        Ok(self
            .table
            .types
            .iter()
            .filter(|(_, d)| d.kind == "complex-type")
            .map(|(n, _)| n.clone())
            .collect())
    }

    async fn get_primitive_types(&self) -> Result<Vec<String>, ModelError> {
        Ok(self.table.primitives.keys().cloned().collect())
    }

    async fn get_fhir_version(&self) -> Result<FhirVersion, ModelError> {
        Ok(FhirVersion::R5)
    }

    fn is_type_derived_from(&self, derived_type: &str, base_type: &str) -> bool {
        self.derives(derived_type, base_type)
    }
}

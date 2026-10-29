//! The SearchParameter catalog and the per-node registry of supported parameters
//! (fhir-r5-model.md § Search parameters and indexed projections).

use std::collections::BTreeMap;
use std::sync::Arc;

use jiff::tz::Offset;
use serde_json::Value;

use crate::path::SearchPath;

/// A SearchParameter's `type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ParamType {
    /// `number`.
    Number,
    /// `date`.
    Date,
    /// `string`.
    String,
    /// `token`.
    Token,
    /// `reference`.
    Reference,
    /// `composite`.
    Composite,
    /// `quantity`.
    Quantity,
    /// `uri`.
    Uri,
}

impl ParamType {
    fn parse(code: &str) -> Option<Self> {
        Some(match code {
            "number" => Self::Number,
            "date" => Self::Date,
            "string" => Self::String,
            "token" => Self::Token,
            "reference" => Self::Reference,
            "composite" => Self::Composite,
            "quantity" => Self::Quantity,
            "uri" => Self::Uri,
            _ => return None,
        })
    }
}

/// One component of a composite parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Component {
    /// The component parameter's canonical URL.
    pub definition: String,
    /// Its type, from the component parameter.
    pub param_type: ParamType,
    /// Its expression, relative to each element the composite expression selects.
    pub path: SearchPath,
}

/// A compiled SearchParameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchParameter {
    /// Its canonical URL.
    pub url: String,
    /// The code used in queries, e.g. `birthdate`.
    pub code: String,
    /// The resource types it applies to; `Resource` and `DomainResource` mean every type.
    pub base: Vec<String>,
    /// Its type.
    pub param_type: ParamType,
    /// Its compiled expression.
    pub path: SearchPath,
    /// For a reference parameter, the target types it allows.
    pub targets: Vec<String>,
    /// For a composite parameter, its components in order.
    pub components: Vec<Component>,
}

impl SearchParameter {
    fn applies_to(&self, resource_type: &str) -> bool {
        self.base
            .iter()
            .any(|b| b == resource_type || b == "Resource" || b == "DomainResource")
    }
}

/// A SearchParameter that could not be compiled, with the reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rejected {
    /// Its URL, or `code` when it has none.
    pub url: String,
    /// Why it cannot be supported.
    pub reason: String,
}

/// Every SearchParameter that compiled, from the core package and configuration packages.
#[derive(Debug, Clone, Default)]
pub struct Catalog {
    params: BTreeMap<String, Arc<SearchParameter>>,
    rejected: Vec<Rejected>,
}

impl Catalog {
    /// Compiles SearchParameter resources. Parameters of type `special`, expressions outside
    /// the search subset and composites whose components do not resolve are rejected.
    pub fn from_definitions<'a>(definitions: impl IntoIterator<Item = &'a Value>) -> Self {
        let mut catalog = Self::default();
        let mut composites = Vec::new();
        for definition in definitions {
            let text = |key: &str| definition.get(key).and_then(Value::as_str);
            let url = text("url").or(text("code")).unwrap_or_default().to_owned();
            let reject = |catalog: &mut Self, reason: String| {
                catalog.rejected.push(Rejected {
                    url: url.clone(),
                    reason,
                });
            };
            let Some(code) = text("code") else {
                reject(&mut catalog, "no code".into());
                continue;
            };
            let Some(param_type) = text("type").and_then(ParamType::parse) else {
                reject(
                    &mut catalog,
                    format!("type {:?} is not supported", text("type")),
                );
                continue;
            };
            let Some(expression) = text("expression") else {
                reject(&mut catalog, "no expression".into());
                continue;
            };
            let path = match SearchPath::parse(expression) {
                Ok(path) => path,
                Err(error) => {
                    reject(&mut catalog, error.to_string());
                    continue;
                }
            };
            let strings = |key: &str| -> Vec<String> {
                definition
                    .get(key)
                    .and_then(Value::as_array)
                    .map(|items| {
                        items
                            .iter()
                            .filter_map(Value::as_str)
                            .map(str::to_owned)
                            .collect()
                    })
                    .unwrap_or_default()
            };
            let param = SearchParameter {
                url: url.clone(),
                code: code.to_owned(),
                base: strings("base"),
                param_type,
                path,
                targets: strings("target"),
                components: Vec::new(),
            };
            if let Some(existing) = param
                .base
                .iter()
                .find_map(|b| catalog.find(b, code).filter(|p| p.base.contains(b)))
            {
                let reason = format!("duplicates {} for {code}", existing.url);
                reject(&mut catalog, reason);
                continue;
            }
            if param_type == ParamType::Composite {
                composites.push((param, definition));
            } else {
                catalog.params.insert(url, Arc::new(param));
            }
        }
        for (mut param, definition) in composites {
            match catalog.components(definition) {
                Ok(components) => {
                    param.components = components;
                    catalog.params.insert(param.url.clone(), Arc::new(param));
                }
                Err(reason) => catalog.rejected.push(Rejected {
                    url: param.url,
                    reason,
                }),
            }
        }
        catalog
    }

    fn components(&self, definition: &Value) -> Result<Vec<Component>, String> {
        let items = definition
            .get("component")
            .and_then(Value::as_array)
            .filter(|items| !items.is_empty())
            .ok_or("a composite without components")?;
        items
            .iter()
            .map(|item| {
                let url = item
                    .get("definition")
                    .and_then(Value::as_str)
                    .ok_or("a component without a definition")?;
                let target = self
                    .params
                    .get(url)
                    .ok_or_else(|| format!("component {url} is not in the catalog"))?;
                let expression = item
                    .get("expression")
                    .and_then(Value::as_str)
                    .ok_or("a component without an expression")?;
                Ok(Component {
                    definition: url.to_owned(),
                    param_type: target.param_type,
                    path: SearchPath::parse(expression).map_err(|e| e.to_string())?,
                })
            })
            .collect()
    }

    /// The parameter `code` on `resource_type`, if one compiled.
    pub fn find(&self, resource_type: &str, code: &str) -> Option<&Arc<SearchParameter>> {
        self.params
            .values()
            .find(|p| p.code == code && p.applies_to(resource_type))
    }

    /// The parameters that could not be compiled.
    pub fn rejected(&self) -> &[Rejected] {
        &self.rejected
    }

    /// How many parameters compiled.
    pub fn len(&self) -> usize {
        self.params.len()
    }

    /// Whether nothing compiled.
    pub fn is_empty(&self) -> bool {
        self.params.is_empty()
    }
}

/// Whether a supported parameter can be searched yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamState {
    /// Indexed for every resource; searchable.
    Active,
    /// Indexed on every new write while the reindex job backfills older resources; a search
    /// that uses it is refused until the job completes.
    Reindexing,
}

/// A supported parameter in the registry.
#[derive(Debug, Clone)]
pub struct Supported {
    /// The parameter.
    pub param: Arc<SearchParameter>,
    /// Whether it is searchable yet.
    pub state: ParamState,
}

/// The parameters this node indexes and accepts, per resource type. Only parameters declared
/// supported in the CapabilityStatement are added.
#[derive(Debug, Clone)]
pub struct Registry {
    supported: BTreeMap<(String, String), Supported>,
    local_offset: Offset,
}

/// A parameter the catalog does not have for a resource type.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("no compiled SearchParameter `{code}` applies to {resource_type}")]
pub struct NotInCatalog {
    /// The resource type.
    pub resource_type: String,
    /// The parameter code.
    pub code: String,
}

impl Registry {
    /// An empty registry. Dates without an offset are read at `local_offset`, the node's
    /// configured offset (`+03:00` in Uganda).
    pub fn new(local_offset: Offset) -> Self {
        Self {
            supported: BTreeMap::new(),
            local_offset,
        }
    }

    /// Supports `code` on `resource_type` from `catalog`, in `state`.
    pub fn support(
        &mut self,
        catalog: &Catalog,
        resource_type: &str,
        code: &str,
        state: ParamState,
    ) -> Result<(), NotInCatalog> {
        let param = catalog
            .find(resource_type, code)
            .ok_or_else(|| NotInCatalog {
                resource_type: resource_type.to_owned(),
                code: code.to_owned(),
            })?;
        self.supported.insert(
            (resource_type.to_owned(), code.to_owned()),
            Supported {
                param: Arc::clone(param),
                state,
            },
        );
        Ok(())
    }

    /// Makes a reindexed parameter searchable. Returns whether it was supported.
    pub fn activate(&mut self, resource_type: &str, code: &str) -> bool {
        self.supported
            .get_mut(&(resource_type.to_owned(), code.to_owned()))
            .map(|s| s.state = ParamState::Active)
            .is_some()
    }

    /// The supported parameter `code` on `resource_type`.
    pub fn get(&self, resource_type: &str, code: &str) -> Option<&Supported> {
        self.supported
            .get(&(resource_type.to_owned(), code.to_owned()))
    }

    /// Every parameter indexed on writes of `resource_type`, active or reindexing.
    pub fn indexed(&self, resource_type: &str) -> impl Iterator<Item = &Supported> {
        self.supported
            .range((resource_type.to_owned(), String::new())..)
            .take_while(move |((t, _), _)| t == resource_type)
            .map(|(_, s)| s)
    }

    /// The offset dates without one are read at.
    pub fn local_offset(&self) -> Offset {
        self.local_offset
    }
}

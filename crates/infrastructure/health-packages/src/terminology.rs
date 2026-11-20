//! A package's terminology, read into `health_terminology`'s model (#90; terminology.md): its
//! CodeSystems, ValueSets and ConceptMaps.
//!
//! - **A ValueSet's `expansion`** in the package is its pinned expansion. The terminology service
//!   uses it as shipped, so a binding to a large external code system needs only the codes it
//!   names.
//! - **Unknown filter operators** and malformed resources are refused when the package is read,
//!   never skipped, so nothing is served from a terminology read in part.

use health_terminology::{
    CodeSystem, Concept, ConceptMap, Contains, Content, Designation, Filter, FilterOp, Group,
    Listed, Mapping, Property, PropertyValue, Relationship, Rule, Target, Terminology, ValueSet,
};
use serde_json::Value;

use crate::{Error, Files};

fn refused(path: &str, detail: impl Into<String>) -> Error {
    Error::Terminology {
        path: path.to_owned(),
        detail: detail.into(),
    }
}

fn text(value: &Value, name: &str) -> Option<String> {
    value.get(name).and_then(Value::as_str).map(str::to_owned)
}

fn array<'a>(value: &'a Value, name: &str) -> impl Iterator<Item = &'a Value> {
    value
        .get(name)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
}

/// A canonical without its `|version`.
fn unversioned(canonical: &str) -> String {
    canonical
        .split_once('|')
        .map_or(canonical, |(url, _)| url)
        .to_owned()
}

fn property(value: &Value) -> Option<Property> {
    let code = text(value, "code")?;
    let found = if let Some(code) = text(value, "valueCode") {
        PropertyValue::Code(code)
    } else if let Some(text) = text(value, "valueString") {
        PropertyValue::String(text)
    } else if let Some(flag) = value.get("valueBoolean").and_then(Value::as_bool) {
        PropertyValue::Boolean(flag)
    } else if let Some(coding) = value.get("valueCoding") {
        PropertyValue::Code(text(coding, "code")?)
    } else {
        let other = ["valueInteger", "valueDecimal", "valueDateTime"]
            .iter()
            .find_map(|name| value.get(*name))?;
        PropertyValue::Other(match other {
            Value::String(text) => text.clone(),
            other => other.to_string(),
        })
    };
    Some(Property { code, value: found })
}

fn concept(path: &str, value: &Value) -> Result<Concept, Error> {
    let code = text(value, "code").ok_or_else(|| refused(path, "a concept has no code"))?;
    Ok(Concept {
        display: text(value, "display"),
        definition: text(value, "definition"),
        designations: array(value, "designation")
            .filter_map(|d| {
                Some(Designation {
                    language: text(d, "language"),
                    use_code: d.get("use").and_then(|u| text(u, "code")),
                    value: text(d, "value")?,
                })
            })
            .collect(),
        properties: array(value, "property").filter_map(property).collect(),
        children: array(value, "concept")
            .map(|child| concept(path, child))
            .collect::<Result<_, _>>()?,
        code,
    })
}

/// A CodeSystem resource. A code system that does not say is taken as case-sensitive, the
/// stricter reading.
pub fn code_system(path: &str, value: &Value) -> Result<CodeSystem, Error> {
    let url = text(value, "url").ok_or_else(|| refused(path, "a CodeSystem has no url"))?;
    let content = match text(value, "content").as_deref() {
        Some("complete") => Content::Complete,
        Some("fragment") => Content::Fragment,
        Some("not-present") => Content::NotPresent,
        Some("example") => Content::Example,
        Some("supplement") => Content::Supplement,
        other => return Err(refused(path, format!("unknown content {other:?}"))),
    };
    Ok(CodeSystem {
        url,
        version: text(value, "version"),
        case_sensitive: value
            .get("caseSensitive")
            .and_then(Value::as_bool)
            .unwrap_or(true),
        content,
        concepts: array(value, "concept")
            .map(|c| concept(path, c))
            .collect::<Result<_, _>>()?,
    })
}

fn op(path: &str, code: &str) -> Result<FilterOp, Error> {
    Ok(match code {
        "=" => FilterOp::Equals,
        "is-a" => FilterOp::IsA,
        "descendent-of" => FilterOp::DescendentOf,
        "is-not-a" => FilterOp::IsNotA,
        "generalizes" => FilterOp::Generalizes,
        "child-of" => FilterOp::ChildOf,
        "in" => FilterOp::In,
        "not-in" => FilterOp::NotIn,
        "exists" => FilterOp::Exists,
        "regex" => FilterOp::Regex,
        other => {
            return Err(refused(
                path,
                format!("the filter operator {other} is not supported"),
            ));
        }
    })
}

fn rule(path: &str, value: &Value) -> Result<Rule, Error> {
    Ok(Rule {
        system: text(value, "system"),
        version: text(value, "version"),
        concepts: array(value, "concept")
            .map(|c| {
                Ok(Listed {
                    code: text(c, "code").ok_or_else(|| refused(path, "a listed code is empty"))?,
                    display: text(c, "display"),
                })
            })
            .collect::<Result<_, Error>>()?,
        filters: array(value, "filter")
            .map(|f| {
                let (Some(property), Some(code), Some(value)) =
                    (text(f, "property"), text(f, "op"), text(f, "value"))
                else {
                    return Err(refused(
                        path,
                        "a filter needs a property, an op and a value",
                    ));
                };
                Ok(Filter {
                    property,
                    op: op(path, &code)?,
                    value,
                })
            })
            .collect::<Result<_, Error>>()?,
        value_sets: array(value, "valueSet")
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect(),
    })
}

/// Every coded entry of an expansion, flattened: nested `contains` are its codes too.
fn contains(into: &mut Vec<Contains>, value: &Value) {
    for entry in array(value, "contains") {
        if let (Some(system), Some(code)) = (text(entry, "system"), text(entry, "code")) {
            into.push(Contains {
                system,
                version: text(entry, "version"),
                code,
                display: text(entry, "display"),
                inactive: entry
                    .get("inactive")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            });
        }
        contains(into, entry);
    }
}

/// A ValueSet resource, with its `expansion` as the pinned expansion.
pub fn value_set(path: &str, value: &Value) -> Result<ValueSet, Error> {
    let url = text(value, "url").ok_or_else(|| refused(path, "a ValueSet has no url"))?;
    let compose = value.get("compose").cloned().unwrap_or(Value::Null);
    let expansion = value.get("expansion").map(|expansion| {
        let mut codes = Vec::new();
        contains(&mut codes, expansion);
        codes
    });
    Ok(ValueSet {
        url,
        version: text(value, "version"),
        include: array(&compose, "include")
            .map(|r| rule(path, r))
            .collect::<Result<_, _>>()?,
        exclude: array(&compose, "exclude")
            .map(|r| rule(path, r))
            .collect::<Result<_, _>>()?,
        inactive: compose.get("inactive").and_then(Value::as_bool),
        expansion,
    })
}

fn relationship(path: &str, code: Option<&str>) -> Result<Relationship, Error> {
    Ok(match code {
        Some("equivalent") => Relationship::Equivalent,
        Some("related-to") => Relationship::RelatedTo,
        Some("source-is-narrower-than-target") => Relationship::SourceIsNarrowerThanTarget,
        Some("source-is-broader-than-target") => Relationship::SourceIsBroaderThanTarget,
        Some("not-related-to") => Relationship::NotRelatedTo,
        other => return Err(refused(path, format!("unknown relationship {other:?}"))),
    })
}

/// A ConceptMap resource (R5). An element with `noMap` maps to no target.
pub fn concept_map(path: &str, value: &Value) -> Result<ConceptMap, Error> {
    let url = text(value, "url").ok_or_else(|| refused(path, "a ConceptMap has no url"))?;
    let groups = array(value, "group")
        .map(|group| {
            let (Some(source), Some(target)) = (text(group, "source"), text(group, "target"))
            else {
                return Err(refused(path, "a group needs its source and target"));
            };
            let mappings = array(group, "element")
                .map(|element| {
                    let code = text(element, "code")
                        .ok_or_else(|| refused(path, "an element has no code"))?;
                    let mut targets = array(element, "target")
                        .map(|t| {
                            Ok(Target {
                                code: text(t, "code"),
                                display: text(t, "display"),
                                relationship: relationship(
                                    path,
                                    t.get("relationship").and_then(Value::as_str),
                                )?,
                            })
                        })
                        .collect::<Result<Vec<_>, Error>>()?;
                    if element.get("noMap").and_then(Value::as_bool) == Some(true) {
                        targets.push(Target {
                            code: None,
                            display: None,
                            relationship: Relationship::NotRelatedTo,
                        });
                    }
                    Ok(Mapping { code, targets })
                })
                .collect::<Result<_, Error>>()?;
            Ok(Group {
                source: unversioned(&source),
                target: unversioned(&target),
                mappings,
            })
        })
        .collect::<Result<_, Error>>()?;
    Ok(ConceptMap {
        url,
        version: text(value, "version"),
        groups,
    })
}

/// Adds every CodeSystem, ValueSet and ConceptMap among a package's top-level `files` to
/// `terminology`. Files that are not JSON resources of those types are left alone.
pub fn load(terminology: &mut Terminology, files: &Files) -> Result<(), Error> {
    for (path, bytes) in files {
        if path.contains('/') || !path.ends_with(".json") || path.starts_with('.') {
            continue;
        }
        let Ok(value) = serde_json::from_slice::<Value>(bytes) else {
            continue;
        };
        match value.get("resourceType").and_then(Value::as_str) {
            Some("CodeSystem") => terminology.add_code_system(code_system(path, &value)?),
            Some("ValueSet") => terminology.add_value_set(value_set(path, &value)?),
            Some("ConceptMap") => terminology.add_concept_map(concept_map(path, &value)?),
            _ => {}
        }
    }
    Ok(())
}

/// A CodeSystem or ValueSet of a package, as the Studio lists it (#523).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// Its file in the package.
    pub path: String,
    /// `CodeSystem` or `ValueSet`.
    pub resource_type: String,
    /// Its canonical URL.
    pub url: String,
    /// Its version.
    pub version: Option<String>,
    /// Its `name`.
    pub name: Option<String>,
    /// Its `title`.
    pub title: Option<String>,
}

/// A Questionnaire question that binds a ValueSet (`answerValueSet`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    /// The Questionnaire's file in the package.
    pub path: String,
    /// The Questionnaire's canonical URL.
    pub questionnaire: Option<String>,
    /// Its `title`, or its `name`.
    pub title: Option<String>,
    /// The question's `linkId`.
    pub link_id: String,
    /// The ValueSet bound, `url` or `url|version` as written.
    pub value_set: String,
}

/// What a package holds of terminology and binds to it: its CodeSystems and ValueSets, and its
/// Questionnaires' `answerValueSet` bindings.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Catalogue {
    /// Its CodeSystems and ValueSets, by path.
    pub entries: Vec<Entry>,
    /// Its bindings, by path then question order.
    pub bindings: Vec<Binding>,
}

fn bindings(into: &mut Vec<Binding>, path: &str, form: &Value, items: &Value) {
    for item in array(items, "item") {
        if let (Some(link_id), Some(value_set)) =
            (text(item, "linkId"), text(item, "answerValueSet"))
        {
            into.push(Binding {
                path: path.to_owned(),
                questionnaire: text(form, "url"),
                title: text(form, "title").or_else(|| text(form, "name")),
                link_id,
                value_set,
            });
        }
        bindings(into, path, form, item);
    }
}

/// The CodeSystems, ValueSets and bindings among a package's top-level `files`. Files that are
/// not JSON resources are left alone, as [`load`] does.
pub fn catalogue(files: &Files) -> Catalogue {
    let mut found = Catalogue::default();
    for (path, bytes) in files {
        if path.contains('/') || !path.ends_with(".json") || path.starts_with('.') {
            continue;
        }
        let Ok(value) = serde_json::from_slice::<Value>(bytes) else {
            continue;
        };
        match value.get("resourceType").and_then(Value::as_str) {
            Some(kind @ ("CodeSystem" | "ValueSet")) => {
                if let Some(url) = text(&value, "url") {
                    found.entries.push(Entry {
                        path: path.clone(),
                        resource_type: kind.to_owned(),
                        url,
                        version: text(&value, "version"),
                        name: text(&value, "name"),
                        title: text(&value, "title"),
                    });
                }
            }
            Some("Questionnaire") => bindings(&mut found.bindings, path, &value, &value),
            _ => {}
        }
    }
    found
}

#[cfg(test)]
mod tests;

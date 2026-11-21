//! Editing a local CodeSystem's concepts in the Studio (studio.md § Terminology, Editing a local
//! CodeSystem; #524), and the version the edit calls for (terminology.md's semver rules).
//!
//! - **A code is fixed once published.** A code the CodeSystem's previous published version
//!   holds cannot be deleted, have its code changed or move under another parent; it can only
//!   be **retired**: kept, so records that hold it still resolve, with its `status` property
//!   `retired`, so expansions for new forms (`activeOnly`) leave it out. A new code can be
//!   renamed, moved or deleted freely.
//! - **The version** follows from the change against the previous published CodeSystem,
//!   [`bump::changes`] element by element: adding or retiring a code is minor, a display a
//!   patch, redefining or removing a code major. [`versioning`] gives the next version and the
//!   changes that call for it, and [`apply`] writes that version into the CodeSystem.
//!
//! This module edits the resource as JSON, so everything it does not edit is kept as written.

use std::collections::BTreeSet;
use std::fmt;

use health_configuration::lifecycle::semver::{Bump, ContentChange};
use health_configuration::package::Version;
use serde_json::{Map, Value, json};

use crate::bump;

/// The concept property a retired code carries (FHIR's `status`).
pub const STATUS: &str = "status";
/// The URI FHIR gives the `status` concept property.
pub const STATUS_URI: &str = "http://hl7.org/fhir/concept-properties#status";

/// One edit to a CodeSystem's concepts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConceptEdit {
    /// A new code, at the top level or under `parent`.
    Add {
        /// Its code.
        code: String,
        /// Its display.
        display: Option<String>,
        /// Its definition.
        definition: Option<String>,
        /// The code it goes under, if any.
        parent: Option<String>,
    },
    /// A code's display, definition, and for a new code its code and parent too.
    Change {
        /// The code as it is now.
        code: String,
        /// Its code from now on: only a new code's may change.
        new_code: String,
        /// Its display.
        display: Option<String>,
        /// Its definition.
        definition: Option<String>,
        /// The code it goes under: only a new code may move.
        parent: Option<String>,
    },
    /// A new code removed, with nothing under it.
    Delete {
        /// The code.
        code: String,
    },
    /// A code retired: kept, no longer offered on forms.
    Retire {
        /// The code.
        code: String,
    },
}

impl ConceptEdit {
    /// The code the edit is about (the new one for a change).
    pub fn code(&self) -> &str {
        match self {
            Self::Add { code, .. } | Self::Delete { code } | Self::Retire { code } => code,
            Self::Change { new_code, .. } => new_code,
        }
    }
}

/// Why an edit is refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditRefused {
    /// The resource is not a CodeSystem.
    NotACodeSystem,
    /// The code is empty, or starts or ends with a space.
    BadCode(String),
    /// No concept has that code.
    UnknownCode(String),
    /// Another concept has that code already.
    Taken(String),
    /// A code cannot go under itself or under a code beneath it.
    UnderItself(String),
    /// A published code cannot be deleted, renamed or moved: it can only be retired.
    Published(String),
    /// A code with codes under it cannot be deleted.
    HasChildren(String),
}

impl fmt::Display for EditRefused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotACodeSystem => f.write_str("the file is not a CodeSystem"),
            Self::BadCode(code) => write!(
                f,
                "{code:?} is not a code: it must not be empty or start or end with a space"
            ),
            Self::UnknownCode(code) => write!(f, "the code system has no code {code}"),
            Self::Taken(code) => write!(f, "the code system has a code {code} already"),
            Self::UnderItself(code) => {
                write!(f, "{code} cannot go under itself or a code beneath it")
            }
            Self::Published(code) => write!(
                f,
                "{code} is published: it cannot be deleted, have its code changed or move; retire it instead"
            ),
            Self::HasChildren(code) => {
                write!(f, "{code} has codes under it: delete or move them first")
            }
        }
    }
}

impl std::error::Error for EditRefused {}

/// The next version of a CodeSystem and why, against its previous published version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Versioning {
    /// The previous published version; `None` when the CodeSystem was never published.
    pub published: Option<String>,
    /// The bump the changes need together.
    pub bump: Bump,
    /// The version the changes call for; `None` when never published, or when the published
    /// version is not an exact semver version.
    pub next: Option<String>,
    /// Each change, with the bump it needs.
    pub changes: Vec<ContentChange>,
}

/// The version `current` calls for against `published`, the previous published CodeSystem of the
/// same URL, if any.
pub fn versioning(published: Option<&Value>, current: &Value) -> Versioning {
    let Some(published) = published else {
        return Versioning {
            published: None,
            bump: Bump::None,
            next: None,
            changes: Vec::new(),
        };
    };
    let changes = bump::changes(published, current);
    let needed = bump::required(&changes);
    let was = published
        .get("version")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let next = was
        .as_deref()
        .and_then(|text| text.parse::<Version>().ok())
        .map(|version| bumped(&version, needed).to_string());
    Versioning {
        published: was,
        bump: needed,
        next,
        changes,
    }
}

/// `version` moved by `bump`, without a pre-release.
fn bumped(version: &Version, bump: Bump) -> Version {
    let (major, minor, patch) = (version.major, version.minor, version.patch);
    let (major, minor, patch) = match bump {
        Bump::None => (major, minor, patch),
        Bump::Patch => (major, minor, patch + 1),
        Bump::Minor => (major, minor + 1, 0),
        Bump::Major => (major + 1, 0, 0),
    };
    Version {
        major,
        minor,
        patch,
        pre: None,
    }
}

/// Every code of `code_system`, at any depth.
pub fn codes(code_system: &Value) -> BTreeSet<String> {
    fn walk(list: Option<&Value>, into: &mut BTreeSet<String>) {
        for concept in list.and_then(Value::as_array).into_iter().flatten() {
            if let Some(code) = concept.get("code").and_then(Value::as_str) {
                into.insert(code.to_owned());
            }
            walk(concept.get("concept"), into);
        }
    }
    let mut found = BTreeSet::new();
    walk(code_system.get("concept"), &mut found);
    found
}

/// The concept with `code`, at any depth.
fn find<'a>(list: &'a mut [Value], code: &str) -> Option<&'a mut Value> {
    if let Some(at) = list
        .iter()
        .position(|c| c.get("code").and_then(Value::as_str) == Some(code))
    {
        return list.get_mut(at);
    }
    list.iter_mut()
        .find_map(|concept| match concept.get_mut("concept") {
            Some(Value::Array(children)) => find(children, code),
            _ => None,
        })
}

/// Takes the concept with `code` out of `list`, at any depth, dropping an emptied `concept`.
fn take(list: &mut Vec<Value>, code: &str) -> Option<Value> {
    if let Some(at) = list
        .iter()
        .position(|c| c.get("code").and_then(Value::as_str) == Some(code))
    {
        return Some(list.remove(at));
    }
    for concept in list.iter_mut() {
        let Some(object) = concept.as_object_mut() else {
            continue;
        };
        if let Some(Value::Array(children)) = object.get_mut("concept")
            && let Some(found) = take(children, code)
        {
            if children.is_empty() {
                object.remove("concept");
            }
            return Some(found);
        }
    }
    None
}

/// The code of the concept directly above `code`, if any.
fn parent_of(list: &[Value], code: &str, above: Option<&str>) -> Option<Option<String>> {
    for concept in list {
        let here = concept.get("code").and_then(Value::as_str);
        if here == Some(code) {
            return Some(above.map(str::to_owned));
        }
        if let Some(children) = concept.get("concept").and_then(Value::as_array)
            && let Some(found) = parent_of(children, code, here)
        {
            return Some(found);
        }
    }
    None
}

/// Puts `concept` at the top of `list`, or under `parent`.
fn place(list: &mut Vec<Value>, parent: Option<&str>, concept: Value) -> Result<(), EditRefused> {
    match parent {
        None => list.push(concept),
        Some(parent) => {
            let above =
                find(list, parent).ok_or_else(|| EditRefused::UnknownCode(parent.to_owned()))?;
            let Some(object) = above.as_object_mut() else {
                return Err(EditRefused::UnknownCode(parent.to_owned()));
            };
            match object
                .entry("concept")
                .or_insert_with(|| Value::Array(Vec::new()))
            {
                Value::Array(children) => children.push(concept),
                _ => return Err(EditRefused::UnknownCode(parent.to_owned())),
            }
        }
    }
    Ok(())
}

fn checked(code: &str) -> Result<(), EditRefused> {
    if code.is_empty() || code.trim() != code {
        return Err(EditRefused::BadCode(code.to_owned()));
    }
    Ok(())
}

/// Sets `name` to `text`, or removes it when there is none or it is blank.
fn set_text(object: &mut Map<String, Value>, name: &str, text: Option<&str>) {
    match text.map(str::trim).filter(|t| !t.is_empty()) {
        Some(text) => {
            object.insert(name.to_owned(), json!(text));
        }
        None => {
            object.remove(name);
        }
    }
}

/// `status` declared among the code system's properties, so a retired concept's property names
/// a declared one.
fn declare_status(code_system: &mut Map<String, Value>) {
    let properties = code_system
        .entry("property")
        .or_insert_with(|| Value::Array(Vec::new()));
    if let Value::Array(list) = properties
        && !list
            .iter()
            .any(|p| p.get("code").and_then(Value::as_str) == Some(STATUS))
    {
        list.push(json!({ "code": STATUS, "uri": STATUS_URI, "type": "code" }));
    }
}

/// `code_system` with `edit` applied, given the codes its previous published version holds
/// (`published`), and its `version` set to the one the result calls for against `previous`, the
/// previous published CodeSystem, when there is one. `count`, if present, follows.
pub fn apply(
    code_system: &Value,
    edit: &ConceptEdit,
    published: &BTreeSet<String>,
    previous: Option<&Value>,
) -> Result<Value, EditRefused> {
    let mut result = code_system.clone();
    let Some(object) = result.as_object_mut() else {
        return Err(EditRefused::NotACodeSystem);
    };
    if object.get("resourceType").and_then(Value::as_str) != Some("CodeSystem") {
        return Err(EditRefused::NotACodeSystem);
    }
    let held = codes(code_system);
    let fixed = |code: &str| -> Result<(), EditRefused> {
        if published.contains(code) {
            Err(EditRefused::Published(code.to_owned()))
        } else {
            Ok(())
        }
    };
    let known = |code: &str| -> Result<(), EditRefused> {
        if held.contains(code) {
            Ok(())
        } else {
            Err(EditRefused::UnknownCode(code.to_owned()))
        }
    };
    let mut list = match object.remove("concept") {
        Some(Value::Array(list)) => list,
        _ => Vec::new(),
    };
    match edit {
        ConceptEdit::Add {
            code,
            display,
            definition,
            parent,
        } => {
            checked(code)?;
            if held.contains(code) {
                return Err(EditRefused::Taken(code.clone()));
            }
            let mut concept = Map::new();
            concept.insert("code".into(), json!(code));
            set_text(&mut concept, "display", display.as_deref());
            set_text(&mut concept, "definition", definition.as_deref());
            place(&mut list, parent.as_deref(), Value::Object(concept))?;
        }
        ConceptEdit::Change {
            code,
            new_code,
            display,
            definition,
            parent,
        } => {
            known(code)?;
            checked(new_code)?;
            let was_under = parent_of(&list, code, None).flatten();
            if new_code != code {
                fixed(code)?;
                if held.contains(new_code) {
                    return Err(EditRefused::Taken(new_code.clone()));
                }
            }
            if *parent != was_under {
                fixed(code)?;
                if let Some(parent) = parent {
                    known(parent)?;
                }
            }
            let edit = |concept: &mut Value| {
                if let Some(fields) = concept.as_object_mut() {
                    fields.insert("code".into(), json!(new_code));
                    set_text(fields, "display", display.as_deref());
                    set_text(fields, "definition", definition.as_deref());
                }
            };
            if *parent == was_under {
                let concept =
                    find(&mut list, code).ok_or_else(|| EditRefused::UnknownCode(code.clone()))?;
                edit(concept);
            } else {
                let mut concept =
                    take(&mut list, code).ok_or_else(|| EditRefused::UnknownCode(code.clone()))?;
                if let Some(parent) = parent
                    && codes(&json!({ "concept": [concept.clone()] })).contains(parent)
                {
                    return Err(EditRefused::UnderItself(code.clone()));
                }
                edit(&mut concept);
                place(&mut list, parent.as_deref(), concept)?;
            }
        }
        ConceptEdit::Delete { code } => {
            known(code)?;
            fixed(code)?;
            let concept =
                find(&mut list, code).ok_or_else(|| EditRefused::UnknownCode(code.clone()))?;
            if concept
                .get("concept")
                .and_then(Value::as_array)
                .is_some_and(|c| !c.is_empty())
            {
                return Err(EditRefused::HasChildren(code.clone()));
            }
            take(&mut list, code);
        }
        ConceptEdit::Retire { code } => {
            known(code)?;
            let concept =
                find(&mut list, code).ok_or_else(|| EditRefused::UnknownCode(code.clone()))?;
            if let Some(fields) = concept.as_object_mut() {
                let properties = fields
                    .entry("property")
                    .or_insert_with(|| Value::Array(Vec::new()));
                if let Value::Array(properties) = properties {
                    properties.retain(|p| p.get("code").and_then(Value::as_str) != Some(STATUS));
                    properties.push(json!({ "code": STATUS, "valueCode": "retired" }));
                }
            }
            declare_status(object);
        }
    }
    if !list.is_empty() {
        object.insert("concept".into(), Value::Array(list));
    }
    if object.contains_key("count") {
        let total = codes(&result).len();
        if let Some(object) = result.as_object_mut() {
            object.insert("count".into(), json!(total));
        }
    }
    if let Some(next) = versioning(previous, &result).next
        && let Some(object) = result.as_object_mut()
    {
        object.insert("version".into(), json!(next));
    }
    Ok(result)
}

#[cfg(test)]
mod tests;

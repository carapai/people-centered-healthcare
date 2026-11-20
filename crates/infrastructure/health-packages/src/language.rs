//! Language Packs on disk (ADR 0066; `health_configuration::language`): a pack's entries read
//! from its [`TRANSLATIONS`] file, the English a target package offers for translation, and the
//! report of which entries are current, stale or gone.
//!
//! The English of a target package, by [`Key`]:
//!
//! | Where | Key |
//! |---|---|
//! | A Questionnaire item's `text` and `prefix` | `url` + `linkId` |
//! | Its help: a child `display` item with the `help` item control | `url` + `linkId` (the parent's) |
//! | An answer option's `valueCoding.display` | `url` + `linkId` + code |
//! | A Form Layout section's `title`, when the section has an `id` | layout `url` + section id |
//! | A CodeSystem concept's `display`, nested ones too | system + code |
//! | A ValueSet's `compose.include.concept.display`, when no CodeSystem gives one | system + code |
//!
//! The target is passed with the packages it depends on, so a pack may also translate the
//! ValueSets and CodeSystems its package binds.

use std::collections::BTreeMap;

use health_configuration::language::{self, Entry, Key, PackProblem, Part, Report, TRANSLATIONS};
use health_configuration::package::{INDEX, PACKAGE_JSON, PackageContents, PackageKind};
use serde::Deserialize;
use serde_json::Value;

use crate::archive::Files;
use crate::schemas::FORM_LAYOUT;
use crate::{Error, Packed};

/// The `questionnaire-itemControl` extension.
const ITEM_CONTROL: &str = "http://hl7.org/fhir/StructureDefinition/questionnaire-itemControl";

/// A pack's entries file.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Translations {
    /// The language they are in.
    pub language: String,
    /// The entries.
    pub entries: Vec<Entry>,
}

/// The entries of the pack in `files`, if it has a [`TRANSLATIONS`] file.
pub fn translations(files: &Files) -> Result<Option<Translations>, Error> {
    let Some(bytes) = files.get(TRANSLATIONS) else {
        return Ok(None);
    };
    serde_json::from_slice(bytes)
        .map(Some)
        .map_err(|e| Error::Json {
            path: TRANSLATIONS.into(),
            detail: e.to_string(),
        })
}

/// What is wrong with a Language Pack's entries: in another language than the pack,
/// duplicated or blank. Nothing for another kind of package; an unreadable file is an error.
pub(crate) fn problems(
    contents: &PackageContents,
    files: &Files,
) -> Result<Vec<PackProblem>, Error> {
    if contents.package.kind != PackageKind::Language {
        return Ok(Vec::new());
    }
    let Some(read) = translations(files)? else {
        return Ok(Vec::new());
    };
    let mut problems = Vec::new();
    if let Some(translates) = &contents.package.translates
        && translates.language != read.language
    {
        problems.push(PackProblem::OtherLanguage {
            pack: translates.language.clone(),
            entries: read.language.clone(),
        });
    }
    problems.extend(language::entry_problems(&read.entries));
    Ok(problems)
}

fn text(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|text| !text.trim().is_empty())
        .map(str::to_owned)
}

fn array<'a>(value: &'a Value, key: &str) -> impl Iterator<Item = &'a Value> {
    value
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
}

/// Whether `item` is a help text: a `display` item with the `help` item control.
fn is_help(item: &Value) -> bool {
    item.get("type").and_then(Value::as_str) == Some("display")
        && array(item, "extension").any(|extension| {
            extension.get("url").and_then(Value::as_str) == Some(ITEM_CONTROL)
                && extension
                    .get("valueCodeableConcept")
                    .is_some_and(|concept| {
                        array(concept, "coding").any(|coding| {
                            coding.get("code").and_then(Value::as_str) == Some("help")
                        })
                    })
        })
}

fn items(url: &str, items: &Value, english: &mut BTreeMap<Key, String>) {
    for item in items.as_array().into_iter().flatten() {
        let Some(link_id) = text(item, "linkId") else {
            continue;
        };
        let key = |part| Key::Item {
            questionnaire: url.to_owned(),
            link_id: link_id.clone(),
            part,
        };
        if let Some(prefix) = text(item, "prefix") {
            english.insert(key(Part::Prefix), prefix);
        }
        if let Some(wording) = text(item, "text") {
            english.insert(key(Part::Text), wording);
        }
        for child in array(item, "item").filter(|child| is_help(child)) {
            if let Some(help) = text(child, "text") {
                english.insert(key(Part::Help), help);
            }
        }
        for option in array(item, "answerOption") {
            let Some(coding) = option.get("valueCoding") else {
                continue;
            };
            if let (Some(code), Some(display)) = (text(coding, "code"), text(coding, "display")) {
                let key = Key::Answer {
                    questionnaire: url.to_owned(),
                    link_id: link_id.clone(),
                    code,
                };
                english.insert(key, display);
            }
        }
        if let Some(children) = item.get("item") {
            let asked: Vec<Value> = children
                .as_array()
                .into_iter()
                .flatten()
                .filter(|child| !is_help(child))
                .cloned()
                .collect();
            self::items(url, &Value::Array(asked), english);
        }
    }
}

fn concepts(system: &str, concepts: &Value, english: &mut BTreeMap<Key, String>) {
    for concept in concepts.as_array().into_iter().flatten() {
        if let (Some(code), Some(display)) = (text(concept, "code"), text(concept, "display")) {
            let key = Key::Concept {
                system: system.to_owned(),
                code,
            };
            english.insert(key, display);
        }
        if let Some(nested) = concept.get("concept") {
            self::concepts(system, nested, english);
        }
    }
}

fn json(path: &str, bytes: &[u8]) -> Result<Value, Error> {
    serde_json::from_slice(bytes).map_err(|e| Error::Json {
        path: path.to_owned(),
        detail: e.to_string(),
    })
}

/// The English the packages in `packages` offer for translation, by key: the target package
/// first, then the packages it depends on.
pub fn english(packages: &[&Files]) -> Result<BTreeMap<Key, String>, Error> {
    let mut english = BTreeMap::new();
    let mut value_sets = BTreeMap::new();
    for files in packages {
        for (path, bytes) in *files {
            let top_level = !path.contains('/') && path.ends_with(".json");
            let configuration = path.starts_with("configuration/") && path.ends_with(".json");
            if (!top_level && !configuration) || path == PACKAGE_JSON || path == INDEX {
                continue;
            }
            let value = json(path, bytes)?;
            if configuration {
                if value.get("$schema").and_then(Value::as_str) != Some(FORM_LAYOUT) {
                    continue;
                }
                let Some(layout) = text(&value, "url") else {
                    continue;
                };
                for section in array(&value, "sections") {
                    if let (Some(id), Some(title)) = (text(section, "id"), text(section, "title")) {
                        let key = Key::Section {
                            layout: layout.clone(),
                            section: id,
                        };
                        english.insert(key, title);
                    }
                }
                continue;
            }
            let Some(url) = text(&value, "url") else {
                continue;
            };
            match value.get("resourceType").and_then(Value::as_str) {
                Some("Questionnaire") => {
                    if let Some(found) = value.get("item") {
                        items(&url, found, &mut english);
                    }
                }
                Some("CodeSystem") => {
                    if let Some(found) = value.get("concept") {
                        concepts(&url, found, &mut english);
                    }
                }
                Some("ValueSet") => {
                    for include in value
                        .get("compose")
                        .into_iter()
                        .flat_map(|compose| array(compose, "include"))
                    {
                        if let (Some(system), Some(found)) =
                            (text(include, "system"), include.get("concept"))
                        {
                            concepts(&system, found, &mut value_sets);
                        }
                    }
                }
                _ => {}
            }
        }
    }
    for (key, display) in value_sets {
        english.entry(key).or_insert(display);
    }
    Ok(english)
}

/// The pack `pack` checked against `target` (and the packages it depends on, after it): which
/// entries are current, stale or gone (ADR 0066). A pack with no entries file has no entries.
pub fn report(pack: &Packed, target: &[&Files]) -> Result<Report, Error> {
    let entries = translations(&pack.files)?
        .map(|read| read.entries)
        .unwrap_or_default();
    Ok(language::report(&entries, &english(target)?))
}

#[cfg(test)]
mod tests;

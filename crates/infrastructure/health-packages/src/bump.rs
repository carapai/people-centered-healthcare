//! What a change to one artefact's content requires of its own version (metadata.md § Versioning;
//! #88). Validation hands what it finds to `health_configuration::lifecycle::semver::check`.
//!
//! | Change | Bump |
//! |---|---|
//! | Display text, help text, typo | patch |
//! | New optional item, new code in a binding, relaxed constraint | minor |
//! | Removed item or code, changed answer type, tightened profile, changed extraction target | major |
//!
//! [`changes`] compares two versions of an artefact and finds every change with the bump it
//! needs. Questionnaires, CodeSystems, ValueSets and StructureDefinitions are compared element by
//! element. For every other resource type, and every element without a rule here, any change is
//! major: a rule that cannot tell must not let a breaking change through as a patch.
//!
//! **On every resource:**
//! - `version`, `date`, `meta` and the narrative `text` follow the release, and need nothing;
//! - descriptive metadata (`name`, `title`, `status`, `publisher`, `description`, `purpose`,
//!   `copyright` and the like) needs a patch;
//! - a changed `resourceType` or `url` is major;
//! - an extension in [`DISPLAY_EXTENSIONS`] needs a patch, and any other extension is major.
//!
//! **Questionnaire** items are matched by `linkId`, at any depth:
//! - a removed item is major, and an added one minor, or major if it is required;
//! - `text` and `prefix` need a patch; `initial` a minor;
//! - `required`, `repeats` and `readOnly` are major when they tighten and minor when they relax,
//!   and so is `maxLength`;
//! - an answer option removed is major, added minor;
//! - a changed `type`, `answerValueSet`, `definition` (the extraction target), `code` or
//!   `enableWhen` is major.
//!
//! **CodeSystem** concepts are matched by `code`, at any depth:
//! - a removed code is major, an added one minor;
//! - a code under another parent is major (subsumption changes);
//! - `display`, `definition` and designations need a patch;
//! - a concept's properties, such as marking it inactive, need a minor;
//! - `count` needs nothing; property and filter definitions removed are major, added minor.
//!
//! **ValueSet** `compose` is compared rule by rule:
//! - an `include` is matched by everything but its codes. Removed it is major, added minor, and
//!   inside a matched one, a code removed is major and a code added minor;
//! - an `exclude` works the other way: added or growing is major, removed or shrinking minor;
//! - `inactive` is major unless it goes from `false` to `true`; `lockedDate` is major;
//! - a stored `expansion` needs nothing, since the compose governs and nodes pin expansions.
//!
//! **StructureDefinition** elements are matched by `id`, in the differential if both versions
//! have one, otherwise in the snapshot:
//! - documentation (`short`, `definition`, `comment`, `requirements`, `alias`, `label`,
//!   `example`, `mapping`, `meaningWhenMissing`) needs a patch;
//! - cardinality: a higher `min` or lower `max` is major, the reverse minor;
//! - types: a removed type or profile is major, an added one minor;
//! - a binding: stronger or to another value set is major, weaker minor;
//! - `fixed[x]`, `pattern[x]`, an invariant, `mustSupport` or `maxLength` added or tightened is
//!   major, removed or relaxed minor; a reworded invariant needs a patch;
//! - an element removed from the differential drops its constraints (minor, unless a field
//!   says otherwise), and one added constrains (major, unless a field says otherwise). An element
//!   removed from a snapshot is major.

use std::collections::BTreeMap;

use health_configuration::lifecycle::semver::{Bump, ContentChange, Nature};
use serde_json::{Map, Value};

/// Extensions that only change how something is shown.
pub const DISPLAY_EXTENSIONS: &[&str] = &[
    "http://hl7.org/fhir/StructureDefinition/translation",
    "http://hl7.org/fhir/StructureDefinition/rendering-xhtml",
    "http://hl7.org/fhir/StructureDefinition/rendering-markdown",
    "http://hl7.org/fhir/StructureDefinition/rendering-style",
    "http://hl7.org/fhir/StructureDefinition/questionnaire-itemControl",
    "http://hl7.org/fhir/StructureDefinition/entryFormat",
    "http://hl7.org/fhir/StructureDefinition/questionnaire-displayCategory",
    "http://hl7.org/fhir/StructureDefinition/questionnaire-supportLink",
];

/// Elements that follow the release and need no bump.
const FOLLOWS_RELEASE: &[&str] = &["version", "date", "meta", "text"];

/// Descriptive metadata: a patch.
const DESCRIPTIVE: &[&str] = &[
    "id",
    "name",
    "title",
    "subtitle",
    "status",
    "experimental",
    "publisher",
    "contact",
    "description",
    "useContext",
    "jurisdiction",
    "purpose",
    "usage",
    "copyright",
    "copyrightLabel",
    "approvalDate",
    "lastReviewDate",
    "effectivePeriod",
    "topic",
    "author",
    "editor",
    "reviewer",
    "endorser",
    "relatedArtifact",
    "identifier",
    "versionAlgorithmString",
    "versionAlgorithmCoding",
];

/// Element documentation in a StructureDefinition: a patch.
const ELEMENT_DOCUMENTATION: &[&str] = &[
    "short",
    "definition",
    "comment",
    "requirements",
    "alias",
    "label",
    "example",
    "mapping",
    "meaningWhenMissing",
    "orderMeaning",
];

#[derive(Default)]
struct Found(Vec<ContentChange>);

impl Found {
    fn add(&mut self, at: impl Into<String>, nature: Nature, bump: Bump) {
        self.0.push(ContentChange {
            at: at.into(),
            nature,
            bump,
        });
    }
}

fn join(at: &str, name: &str) -> String {
    if at.is_empty() {
        name.to_owned()
    } else {
        format!("{at}.{name}")
    }
}

/// Every change between two versions of an artefact, with the bump each needs.
pub fn changes(before: &Value, after: &Value) -> Vec<ContentChange> {
    let mut found = Found::default();
    let (Some(old), Some(new)) = (before.as_object(), after.as_object()) else {
        if before != after {
            found.add("", Nature::Changed, Bump::Major);
        }
        return found.0;
    };
    let kind = old.get("resourceType").and_then(Value::as_str);
    if kind != new.get("resourceType").and_then(Value::as_str) {
        found.add("resourceType", Nature::Changed, Bump::Major);
        return found.0;
    }
    let handled: &[&str] = match kind {
        Some("Questionnaire") => {
            items(old, new, "", &mut found);
            &["item"]
        }
        Some("CodeSystem") => {
            code_system(old, new, &mut found);
            &["concept", "count", "property", "filter"]
        }
        Some("ValueSet") => {
            value_set(old, new, &mut found);
            &["compose", "expansion"]
        }
        Some("StructureDefinition") => {
            structure_definition(old, new, &mut found);
            &["differential", "snapshot"]
        }
        _ => &[],
    };
    rest(old, new, "", handled, &mut found, |name| {
        if FOLLOWS_RELEASE.contains(&name) {
            Some(Bump::None)
        } else if DESCRIPTIVE.contains(&name) {
            Some(Bump::Patch)
        } else {
            None
        }
    });
    found.0
}

/// The bump the changes need together.
pub fn required(changes: &[ContentChange]) -> Bump {
    changes
        .iter()
        .map(|change| change.bump)
        .max()
        .unwrap_or(Bump::None)
}

/// Compares every element not `handled`: `rule` gives the bump for a changed element, and `None`
/// means major. `_name` (a primitive's extensions) follows `name`; `extension` is compared by url.
fn rest(
    old: &Map<String, Value>,
    new: &Map<String, Value>,
    at: &str,
    handled: &[&str],
    found: &mut Found,
    rule: impl Fn(&str) -> Option<Bump>,
) {
    let mut names: Vec<&String> = old.keys().chain(new.keys()).collect();
    names.sort();
    names.dedup();
    for name in names {
        let base = name.strip_prefix('_').unwrap_or(name);
        if handled.contains(&base) || old.get(name) == new.get(name) {
            continue;
        }
        if base == "extension" || base == "modifierExtension" {
            extensions(old.get(name), new.get(name), &join(at, name), found);
            continue;
        }
        let bump = rule(base).unwrap_or(Bump::Major);
        if bump == Bump::None {
            continue;
        }
        let nature = match (old.get(name), new.get(name)) {
            (None, _) => Nature::Added,
            (_, None) => Nature::Removed,
            _ if bump == Bump::Patch => Nature::Reworded,
            _ => Nature::Changed,
        };
        found.add(join(at, name), nature, bump);
    }
}

fn extensions(old: Option<&Value>, new: Option<&Value>, at: &str, found: &mut Found) {
    fn by_url(list: Option<&Value>) -> BTreeMap<String, Vec<&Value>> {
        let mut map: BTreeMap<String, Vec<&Value>> = BTreeMap::new();
        for extension in list.and_then(Value::as_array).into_iter().flatten() {
            let url = extension.get("url").and_then(Value::as_str).unwrap_or("");
            map.entry(url.to_owned()).or_default().push(extension);
        }
        map
    }
    let (before, after) = (by_url(old), by_url(new));
    let mut urls: Vec<&String> = before.keys().chain(after.keys()).collect();
    urls.sort();
    urls.dedup();
    for url in urls {
        if before.get(url) == after.get(url) {
            continue;
        }
        let bump = if DISPLAY_EXTENSIONS.contains(&url.as_str()) {
            Bump::Patch
        } else {
            Bump::Major
        };
        let nature = match (before.contains_key(url), after.contains_key(url)) {
            (false, _) => Nature::Added,
            (_, false) => Nature::Removed,
            _ => Nature::Changed,
        };
        found.add(format!("{at}[{url}]"), nature, bump);
    }
}

fn truthy(map: &Map<String, Value>, name: &str) -> bool {
    map.get(name).and_then(Value::as_bool).unwrap_or(false)
}

/// A flag that allows less when true (`required`, `readOnly`, `mustSupport`).
fn restricting_flag(
    old: &Map<String, Value>,
    new: &Map<String, Value>,
    name: &str,
    at: &str,
    found: &mut Found,
) {
    match (truthy(old, name), truthy(new, name)) {
        (false, true) => found.add(join(at, name), Nature::Tightened, Bump::Major),
        (true, false) => found.add(join(at, name), Nature::Relaxed, Bump::Minor),
        _ => {}
    }
}

/// A limit where less allows less (`maxLength`); absent is no limit.
fn upper_limit(old: Option<&Value>, new: Option<&Value>, at: &str, found: &mut Found) {
    let number = |value: Option<&Value>| value.and_then(Value::as_u64).unwrap_or(u64::MAX);
    let (before, after) = (number(old), number(new));
    if after < before {
        found.add(at, Nature::Tightened, Bump::Major);
    } else if after > before {
        found.add(at, Nature::Relaxed, Bump::Minor);
    }
}

/// Entries of a list matched by `key`, or `None` if any entry has no key or two share one.
fn keyed<'a>(
    list: Option<&'a Value>,
    key: impl Fn(&'a Value) -> Option<String>,
) -> Option<BTreeMap<String, &'a Value>> {
    let mut map = BTreeMap::new();
    for entry in list.and_then(Value::as_array).into_iter().flatten() {
        if map.insert(key(entry)?, entry).is_some() {
            return None;
        }
    }
    Some(map)
}

fn text_key(name: &'static str) -> impl Fn(&Value) -> Option<String> {
    move |entry| entry.get(name).and_then(Value::as_str).map(str::to_owned)
}

// Questionnaire

fn items(old: &Map<String, Value>, new: &Map<String, Value>, at: &str, found: &mut Found) {
    let (Some(before), Some(after)) = (
        keyed(old.get("item"), text_key("linkId")),
        keyed(new.get("item"), text_key("linkId")),
    ) else {
        if old.get("item") != new.get("item") {
            found.add(join(at, "item"), Nature::Changed, Bump::Major);
        }
        return;
    };
    for (link, item) in &before {
        let place = format!("{}[{link}]", join(at, "item"));
        match after.get(link) {
            None => found.add(place, Nature::Removed, Bump::Major),
            Some(next) => {
                if let (Some(old), Some(new)) = (item.as_object(), next.as_object()) {
                    item_fields(old, new, &place, found);
                }
            }
        }
    }
    for (link, item) in &after {
        if !before.contains_key(link) {
            let place = format!("{}[{link}]", join(at, "item"));
            let required = item.get("required").and_then(Value::as_bool) == Some(true);
            let bump = if required { Bump::Major } else { Bump::Minor };
            found.add(place, Nature::Added, bump);
        }
    }
}

fn item_fields(old: &Map<String, Value>, new: &Map<String, Value>, at: &str, found: &mut Found) {
    items(old, new, at, found);
    restricting_flag(old, new, "required", at, found);
    restricting_flag(old, new, "readOnly", at, found);
    match (truthy(old, "repeats"), truthy(new, "repeats")) {
        (true, false) => found.add(join(at, "repeats"), Nature::Tightened, Bump::Major),
        (false, true) => found.add(join(at, "repeats"), Nature::Relaxed, Bump::Minor),
        _ => {}
    }
    upper_limit(
        old.get("maxLength"),
        new.get("maxLength"),
        &join(at, "maxLength"),
        found,
    );
    answer_options(old.get("answerOption"), new.get("answerOption"), at, found);
    rest(
        old,
        new,
        at,
        &[
            "item",
            "linkId",
            "required",
            "readOnly",
            "repeats",
            "maxLength",
            "answerOption",
        ],
        found,
        |name| match name {
            "text" | "prefix" => Some(Bump::Patch),
            "initial" => Some(Bump::Minor),
            _ => None,
        },
    );
}

fn answer_options(old: Option<&Value>, new: Option<&Value>, at: &str, found: &mut Found) {
    // An option is its value[x]; initialSelected and extensions only change how it is shown.
    let value = |option: &Value| {
        option.as_object().map(|map| {
            let value: Map<String, Value> = map
                .iter()
                .filter(|(name, _)| name.starts_with("value"))
                .map(|(name, value)| (name.clone(), value.clone()))
                .collect();
            Value::Object(value).to_string()
        })
    };
    let place = join(at, "answerOption");
    let (Some(before), Some(after)) = (keyed(old, value), keyed(new, value)) else {
        if old != new {
            found.add(place, Nature::Changed, Bump::Major);
        }
        return;
    };
    for (key, option) in &before {
        match after.get(key) {
            None => found.add(format!("{place}[{key}]"), Nature::Removed, Bump::Major),
            Some(next) if next != option => {
                found.add(format!("{place}[{key}]"), Nature::Reworded, Bump::Patch);
            }
            Some(_) => {}
        }
    }
    for key in after.keys() {
        if !before.contains_key(key) {
            found.add(format!("{place}[{key}]"), Nature::Added, Bump::Minor);
        }
    }
}

// CodeSystem

/// Every concept by code, with its parent's code, at any depth. `None` if a code is missing or
/// repeated.
fn concepts(map: &Map<String, Value>) -> Option<BTreeMap<String, (Option<String>, &Value)>> {
    fn walk<'a>(
        list: Option<&'a Value>,
        parent: Option<&str>,
        into: &mut BTreeMap<String, (Option<String>, &'a Value)>,
    ) -> Option<()> {
        for concept in list.and_then(Value::as_array).into_iter().flatten() {
            let code = concept.get("code").and_then(Value::as_str)?;
            if into
                .insert(code.to_owned(), (parent.map(str::to_owned), concept))
                .is_some()
            {
                return None;
            }
            walk(concept.get("concept"), Some(code), into)?;
        }
        Some(())
    }
    let mut into = BTreeMap::new();
    walk(map.get("concept"), None, &mut into)?;
    Some(into)
}

fn code_system(old: &Map<String, Value>, new: &Map<String, Value>, found: &mut Found) {
    match (concepts(old), concepts(new)) {
        (Some(before), Some(after)) => {
            for (code, (parent, concept)) in &before {
                let place = format!("concept[{code}]");
                let Some((next_parent, next)) = after.get(code) else {
                    found.add(place, Nature::Removed, Bump::Major);
                    continue;
                };
                if parent != next_parent {
                    found.add(join(&place, "parent"), Nature::Changed, Bump::Major);
                }
                if let (Some(old), Some(new)) = (concept.as_object(), next.as_object()) {
                    rest(
                        old,
                        new,
                        &place,
                        &["code", "concept"],
                        found,
                        |name| match name {
                            "display" | "definition" | "designation" => Some(Bump::Patch),
                            "property" => Some(Bump::Minor),
                            _ => None,
                        },
                    );
                }
            }
            for code in after.keys() {
                if !before.contains_key(code) {
                    found.add(format!("concept[{code}]"), Nature::Added, Bump::Minor);
                }
            }
        }
        _ => {
            if old.get("concept") != new.get("concept") {
                found.add("concept", Nature::Changed, Bump::Major);
            }
        }
    }
    for list in ["property", "filter"] {
        definitions(old.get(list), new.get(list), list, found);
    }
}

/// Property or filter definitions, by code: removed major, added minor, a reworded description a
/// patch, anything else major.
fn definitions(old: Option<&Value>, new: Option<&Value>, at: &str, found: &mut Found) {
    let (Some(before), Some(after)) = (keyed(old, text_key("code")), keyed(new, text_key("code")))
    else {
        if old != new {
            found.add(at, Nature::Changed, Bump::Major);
        }
        return;
    };
    for (code, definition) in &before {
        let place = format!("{at}[{code}]");
        match after.get(code) {
            None => found.add(place, Nature::Removed, Bump::Major),
            Some(next) => {
                if let (Some(old), Some(new)) = (definition.as_object(), next.as_object()) {
                    rest(old, new, &place, &["code"], found, |name| {
                        (name == "description").then_some(Bump::Patch)
                    });
                }
            }
        }
    }
    for code in after.keys() {
        if !before.contains_key(code) {
            found.add(format!("{at}[{code}]"), Nature::Added, Bump::Minor);
        }
    }
}

// ValueSet

fn value_set(old: &Map<String, Value>, new: &Map<String, Value>, found: &mut Found) {
    let empty = Map::new();
    let compose = |map: &Map<String, Value>| map.get("compose").and_then(Value::as_object).cloned();
    let (before, after) = (compose(old), compose(new));
    let (before, after) = (
        before.as_ref().unwrap_or(&empty),
        after.as_ref().unwrap_or(&empty),
    );
    rules(
        before.get("include"),
        after.get("include"),
        "compose.include",
        false,
        found,
    );
    rules(
        before.get("exclude"),
        after.get("exclude"),
        "compose.exclude",
        true,
        found,
    );
    match (
        before.get("inactive").and_then(Value::as_bool),
        after.get("inactive").and_then(Value::as_bool),
    ) {
        (Some(false), Some(true)) => {
            found.add("compose.inactive", Nature::Relaxed, Bump::Minor);
        }
        (old, new) if old != new => found.add("compose.inactive", Nature::Changed, Bump::Major),
        _ => {}
    }
    rest(
        before,
        after,
        "compose",
        &["include", "exclude", "inactive"],
        found,
        |_| None,
    );
}

/// Include or exclude rules, matched by everything but their codes. For an exclude, more is
/// fewer codes in the value set, so its bumps are reversed.
fn rules(old: Option<&Value>, new: Option<&Value>, at: &str, exclude: bool, found: &mut Found) {
    let selector = |rule: &Value| {
        rule.as_object().map(|map| {
            let mut map = map.clone();
            map.remove("concept");
            Value::Object(map).to_string()
        })
    };
    let (grows, shrinks) = if exclude {
        (Bump::Major, Bump::Minor)
    } else {
        (Bump::Minor, Bump::Major)
    };
    let (Some(before), Some(after)) = (keyed(old, selector), keyed(new, selector)) else {
        if old != new {
            found.add(at, Nature::Changed, Bump::Major);
        }
        return;
    };
    for (key, rule) in &before {
        let place = format!("{at}[{key}]");
        let Some(next) = after.get(key) else {
            found.add(place, Nature::Removed, shrinks);
            continue;
        };
        fn codes(rule: &Value) -> Option<BTreeMap<String, &Value>> {
            keyed(rule.get("concept"), text_key("code"))
        }
        match (codes(rule), codes(next)) {
            (Some(was), Some(now)) => {
                let (was_all, now_all) =
                    (rule.get("concept").is_none(), next.get("concept").is_none());
                if was_all != now_all {
                    // From every code in the system to a list, or back.
                    let bump = if now_all { grows } else { shrinks };
                    found.add(join(&place, "concept"), Nature::Changed, bump);
                    continue;
                }
                for (code, concept) in &was {
                    match now.get(code) {
                        None => {
                            found.add(format!("{place}.concept[{code}]"), Nature::Removed, shrinks)
                        }
                        Some(other) if other != concept => found.add(
                            format!("{place}.concept[{code}]"),
                            Nature::Reworded,
                            Bump::Patch,
                        ),
                        Some(_) => {}
                    }
                }
                for code in now.keys() {
                    if !was.contains_key(code) {
                        found.add(format!("{place}.concept[{code}]"), Nature::Added, grows);
                    }
                }
            }
            _ => {
                if rule.get("concept") != next.get("concept") {
                    found.add(join(&place, "concept"), Nature::Changed, Bump::Major);
                }
            }
        }
    }
    for key in after.keys() {
        if !before.contains_key(key) {
            found.add(format!("{at}[{key}]"), Nature::Added, grows);
        }
    }
}

// StructureDefinition

fn structure_definition(old: &Map<String, Value>, new: &Map<String, Value>, found: &mut Found) {
    let elements = |map: &Map<String, Value>, part: &str| {
        map.get(part).and_then(|part| part.get("element")).cloned()
    };
    let differential = old.contains_key("differential") && new.contains_key("differential");
    let part = if differential {
        "differential"
    } else {
        "snapshot"
    };
    let element_key = |element: &Value| {
        element
            .get("id")
            .or_else(|| element.get("path"))
            .and_then(Value::as_str)
            .map(str::to_owned)
    };
    let (old_elements, new_elements) = (elements(old, part), elements(new, part));
    let (Some(before), Some(after)) = (
        keyed(old_elements.as_ref(), element_key),
        keyed(new_elements.as_ref(), element_key),
    ) else {
        if old_elements != new_elements {
            found.add(part, Nature::Changed, Bump::Major);
        }
        return;
    };
    let empty = Map::new();
    for (id, element) in &before {
        let place = format!("element[{id}]");
        let old = element.as_object().unwrap_or(&empty);
        match after.get(id) {
            Some(next) => element_fields(old, next.as_object().unwrap_or(&empty), &place, found),
            None if differential => {
                // Its constraints are dropped: what each field's removal needs, at least minor.
                let before = found.0.len();
                element_fields(old, &empty, &place, found);
                if found.0.len() == before {
                    found.add(place, Nature::Removed, Bump::Minor);
                }
            }
            None => found.add(place, Nature::Removed, Bump::Major),
        }
    }
    for (id, element) in &after {
        if !before.contains_key(id) {
            let place = format!("element[{id}]");
            let start = found.0.len();
            element_fields(&empty, element.as_object().unwrap_or(&empty), &place, found);
            if !differential || found.0.len() == start {
                found.add(place, Nature::Added, Bump::Major);
            }
        }
    }
}

fn cardinality(value: Option<&Value>) -> Option<u64> {
    match value? {
        Value::String(text) if text == "*" => Some(u64::MAX),
        Value::String(text) => text.parse().ok(),
        other => other.as_u64(),
    }
}

/// `min`, where higher allows less, or `max`, where lower does. Absent is the base's own, so
/// adding one is taken as tightening and removing one as relaxing.
fn bound(
    old: Option<&Value>,
    new: Option<&Value>,
    higher_tightens: bool,
    at: &str,
    found: &mut Found,
) {
    let (before, after) = (cardinality(old), cardinality(new));
    let tightened = match (before, after) {
        (None, None) => return,
        (None, Some(_)) => true,
        (Some(_), None) => false,
        (Some(a), Some(b)) if a == b => return,
        (Some(a), Some(b)) => (b > a) == higher_tightens,
    };
    if tightened {
        found.add(at, Nature::Tightened, Bump::Major);
    } else {
        found.add(at, Nature::Relaxed, Bump::Minor);
    }
}

fn element_fields(old: &Map<String, Value>, new: &Map<String, Value>, at: &str, found: &mut Found) {
    bound(
        old.get("min"),
        new.get("min"),
        true,
        &join(at, "min"),
        found,
    );
    bound(
        old.get("max"),
        new.get("max"),
        false,
        &join(at, "max"),
        found,
    );
    restricting_flag(old, new, "mustSupport", at, found);
    upper_limit(
        old.get("maxLength"),
        new.get("maxLength"),
        &join(at, "maxLength"),
        found,
    );
    types(old.get("type"), new.get("type"), at, found);
    binding(old.get("binding"), new.get("binding"), at, found);
    invariants(old.get("constraint"), new.get("constraint"), at, found);
    let mut names: Vec<&String> = old.keys().chain(new.keys()).collect();
    names.sort();
    names.dedup();
    for name in names {
        if (name.starts_with("fixed") || name.starts_with("pattern"))
            && old.get(name) != new.get(name)
        {
            let place = join(at, name);
            match new.get(name) {
                None => found.add(place, Nature::Relaxed, Bump::Minor),
                Some(_) => found.add(place, Nature::Tightened, Bump::Major),
            }
        }
    }
    rest(
        old,
        new,
        at,
        &[
            "id",
            "path",
            "min",
            "max",
            "mustSupport",
            "maxLength",
            "type",
            "binding",
            "constraint",
        ],
        found,
        |name| {
            if ELEMENT_DOCUMENTATION.contains(&name) {
                Some(Bump::Patch)
            } else if name.starts_with("fixed") || name.starts_with("pattern") {
                Some(Bump::None)
            } else if name == "isSummary" || name == "condition" {
                Some(Bump::Minor)
            } else {
                None
            }
        },
    );
}

/// An element's types by code: a type removed is major, added minor, and so for each type's
/// `profile` and `targetProfile`, since each lists what is allowed.
fn types(old: Option<&Value>, new: Option<&Value>, at: &str, found: &mut Found) {
    let place = join(at, "type");
    if old.is_none() || new.is_none() {
        // Adding types to an element constrains it; removing them falls back to the base.
        match (old, new) {
            (None, Some(_)) => found.add(place, Nature::Tightened, Bump::Major),
            (Some(_), None) => found.add(place, Nature::Relaxed, Bump::Minor),
            _ => {}
        }
        return;
    }
    let (Some(before), Some(after)) = (keyed(old, text_key("code")), keyed(new, text_key("code")))
    else {
        if old != new {
            found.add(place, Nature::Changed, Bump::Major);
        }
        return;
    };
    for (code, kind) in &before {
        let here = format!("{place}[{code}]");
        let Some(next) = after.get(code) else {
            found.add(here, Nature::Removed, Bump::Major);
            continue;
        };
        let empty = Map::new();
        let (old, new) = (
            kind.as_object().unwrap_or(&empty),
            next.as_object().unwrap_or(&empty),
        );
        for list in ["profile", "targetProfile"] {
            let set = |map: &Map<String, Value>| -> Vec<String> {
                map.get(list)
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            };
            let (was, now) = (set(old), set(new));
            if was.is_empty() != now.is_empty() {
                // None listed allows any; listing some constrains.
                let bump = if now.is_empty() {
                    Bump::Minor
                } else {
                    Bump::Major
                };
                let nature = if now.is_empty() {
                    Nature::Relaxed
                } else {
                    Nature::Tightened
                };
                found.add(join(&here, list), nature, bump);
                continue;
            }
            for profile in &was {
                if !now.contains(profile) {
                    found.add(
                        format!("{here}.{list}[{profile}]"),
                        Nature::Removed,
                        Bump::Major,
                    );
                }
            }
            for profile in &now {
                if !was.contains(profile) {
                    found.add(
                        format!("{here}.{list}[{profile}]"),
                        Nature::Added,
                        Bump::Minor,
                    );
                }
            }
        }
        rest(
            old,
            new,
            &here,
            &["code", "profile", "targetProfile"],
            found,
            |_| None,
        );
    }
    for code in after.keys() {
        if !before.contains_key(code) {
            found.add(format!("{place}[{code}]"), Nature::Added, Bump::Minor);
        }
    }
}

fn strength(binding: &Map<String, Value>) -> u8 {
    match binding.get("strength").and_then(Value::as_str) {
        Some("required") => 4,
        Some("extensible") => 3,
        Some("preferred") => 2,
        _ => 1,
    }
}

fn binding(old: Option<&Value>, new: Option<&Value>, at: &str, found: &mut Found) {
    let place = join(at, "binding");
    let (old, new) = match (
        old.and_then(Value::as_object),
        new.and_then(Value::as_object),
    ) {
        (None, None) => return,
        (None, Some(_)) => return found.add(place, Nature::Tightened, Bump::Major),
        (Some(_), None) => return found.add(place, Nature::Relaxed, Bump::Minor),
        (Some(old), Some(new)) => (old, new),
    };
    let (was, now) = (strength(old), strength(new));
    if now > was {
        found.add(join(&place, "strength"), Nature::Tightened, Bump::Major);
    } else if now < was {
        found.add(join(&place, "strength"), Nature::Relaxed, Bump::Minor);
    }
    rest(old, new, &place, &["strength"], found, |name| {
        (name == "description").then_some(Bump::Patch)
    });
}

fn invariants(old: Option<&Value>, new: Option<&Value>, at: &str, found: &mut Found) {
    let place = join(at, "constraint");
    let (Some(before), Some(after)) = (keyed(old, text_key("key")), keyed(new, text_key("key")))
    else {
        if old != new {
            found.add(place, Nature::Changed, Bump::Major);
        }
        return;
    };
    for (key, invariant) in &before {
        let here = format!("{place}[{key}]");
        match after.get(key) {
            None => found.add(here, Nature::Removed, Bump::Minor),
            Some(next) => {
                if let (Some(old), Some(new)) = (invariant.as_object(), next.as_object()) {
                    rest(old, new, &here, &["key"], found, |name| match name {
                        "human" | "requirements" => Some(Bump::Patch),
                        _ => None,
                    });
                }
            }
        }
    }
    for key in after.keys() {
        if !before.contains_key(key) {
            found.add(format!("{place}[{key}]"), Nature::Added, Bump::Major);
        }
    }
}

#[cfg(test)]
mod tests;

//! Tier 3: profile validation from compiled StructureDefinition snapshots (ADR 0053;
//! fhir-r5-model.md § Validation).
//!
//! - **Compiling.** [`ProfileSet::compile`] turns profiles (snapshots only) into rule trees, keyed
//!   by element id: `Observation.value[x]:valueQuantity.code` is the `code` child of the
//!   `valueQuantity` slice of `value[x]`. A construct the validator does not support is a
//!   [`CompileError`], so the package cannot be activated. It is never silently passed.
//! - **Validating.** [`SnapshotValidator`] walks raw JSON against the rules, after tiers 1–2 have
//!   passed, so every violation it reports comes from the profile tightening the base:
//!   - cardinality;
//!   - `fixed[x]` (exact) and `pattern[x]` (subset);
//!   - type and target-profile restrictions;
//!   - extension definitions and their element contexts;
//!   - profile invariants, through the [`FhirPathEngine`];
//!   - slicing with the `value`, `pattern`, `exists`, `type` and `profile` discriminators, the
//!     `closed`, `open` and `openAtEnd` rules, and `ordered`.
//! - **Out of v1** (ADR 0053): `mustSupport`; bindings (tier 4); discriminators through
//!   `resolve()`; profiles outside the compiled set.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use serde_json::{Map, Value};

use crate::fhirpath::{FhirPathEngine, FhirPathItem};
use crate::structure::{Issue, IssueCode, is_primitive, resolve, table};

const SD: &str = "http://hl7.org/fhir/StructureDefinition/";

/// A profile construct the validator cannot enforce, found while compiling.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{url} at {element}: {reason}")]
pub struct CompileError {
    /// The profile's canonical URL.
    pub url: String,
    /// The element id, or the profile itself.
    pub element: String,
    /// What is not supported.
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Discriminator {
    Value,
    Exists,
    Type,
    Profile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SlicingRules {
    Closed,
    Open,
    OpenAtEnd,
}

#[derive(Debug, Clone)]
struct Slicing {
    discriminators: Vec<(Discriminator, String)>,
    rules: SlicingRules,
    ordered: bool,
}

#[derive(Debug, Clone, Default)]
struct TypeRule {
    code: String,
    profiles: Vec<String>,
    /// `targetProfile` URLs: base types or profiles in the set.
    target_profiles: Vec<String>,
}

#[derive(Debug, Clone)]
struct ProfileConstraint {
    key: String,
    expression: String,
    human: String,
}

/// The rules for one element (or slice) of a profile.
#[derive(Debug, Clone, Default)]
struct Rule {
    min: u64,
    max: Option<u64>,
    types: Vec<TypeRule>,
    fixed: Option<Value>,
    pattern: Option<Value>,
    constraints: Vec<ProfileConstraint>,
    children: BTreeMap<String, Rule>,
    slicing: Option<Slicing>,
    slices: Vec<(String, Rule)>,
}

/// One compiled profile.
#[derive(Debug, Clone)]
struct Compiled {
    url: String,
    type_name: String,
    root: Rule,
    /// For an extension definition, the element paths or types it may be used on.
    contexts: Vec<String>,
}

/// The profiles active on a node, compiled once at package activation (ADR 0025).
#[derive(Debug, Clone, Default)]
pub struct ProfileSet {
    profiles: HashMap<String, Arc<Compiled>>,
}

/// A canonical without its `|version`. Generated snapshots pin versions
/// (`…/Organization|5.0.0`); the active set holds one version of each profile.
fn unversioned(url: &str) -> &str {
    url.split_once('|').map_or(url, |(url, _)| url)
}

/// Whether `url` is the base definition of an R5 type (not a profile).
fn is_base_type(url: &str) -> bool {
    url.strip_prefix(SD).is_some_and(|name| {
        table().types.contains_key(name) || table().primitives.contains_key(name)
    })
}

/// Splits an element id into `(name, slice)` segments: `a.b:s.c` → a, b:s, c.
fn segments(id: &str) -> Vec<(&str, Option<&str>)> {
    id.split('.')
        .map(|segment| match segment.split_once(':') {
            Some((name, slice)) => (name, Some(slice)),
            None => (segment, None),
        })
        .collect()
}

impl ProfileSet {
    /// Compiles StructureDefinitions (profiles of resources, datatypes and extensions). Every
    /// construct that cannot be enforced is reported; any error means the set must not be
    /// activated.
    pub fn compile(definitions: &[Value]) -> Result<Self, Vec<CompileError>> {
        let mut errors = Vec::new();
        let mut profiles = HashMap::new();
        for definition in definitions {
            match compile_one(definition) {
                Ok(compiled) => {
                    profiles.insert(compiled.url.clone(), Arc::new(compiled));
                }
                Err(mut found) => errors.append(&mut found),
            }
        }
        let set = Self { profiles };
        errors.extend(set.check_references());
        if errors.is_empty() {
            Ok(set)
        } else {
            Err(errors)
        }
    }

    /// The compiled profile URLs.
    pub fn urls(&self) -> impl Iterator<Item = &str> {
        self.profiles.keys().map(String::as_str)
    }

    /// Profiles named by type profiles, target profiles and `profile` discriminators must be in
    /// the set, or be base types: anything else would go unchecked.
    fn check_references(&self) -> Vec<CompileError> {
        let mut errors = Vec::new();
        for compiled in self.profiles.values() {
            let mut stack: Vec<(String, &Rule)> =
                vec![(compiled.type_name.clone(), &compiled.root)];
            while let Some((id, rule)) = stack.pop() {
                for t in &rule.types {
                    for target in &t.target_profiles {
                        if !is_base_type(target) && !self.profiles.contains_key(target) {
                            errors.push(CompileError {
                                url: compiled.url.clone(),
                                element: id.clone(),
                                reason: format!("target profile {target} is not in the active set"),
                            });
                        }
                    }
                    for profile in &t.profiles {
                        if !is_base_type(profile) && !self.profiles.contains_key(profile) {
                            errors.push(CompileError {
                                url: compiled.url.clone(),
                                element: id.clone(),
                                reason: format!("type profile {profile} is not in the active set"),
                            });
                        }
                    }
                }
                for (name, child) in &rule.children {
                    stack.push((format!("{id}.{name}"), child));
                }
                for (name, slice) in &rule.slices {
                    stack.push((format!("{id}:{name}"), slice));
                }
            }
        }
        errors
    }
}

fn error(url: &str, element: &str, reason: impl Into<String>) -> CompileError {
    CompileError {
        url: url.to_owned(),
        element: element.to_owned(),
        reason: reason.into(),
    }
}

fn compile_one(definition: &Value) -> Result<Compiled, Vec<CompileError>> {
    let url = definition["url"].as_str().unwrap_or_default().to_owned();
    let type_name = definition["type"].as_str().unwrap_or_default().to_owned();
    let mut errors = Vec::new();
    if definition["derivation"].as_str() != Some("constraint") {
        return Err(vec![error(
            &url,
            &url,
            "only constraint profiles are compiled",
        )]);
    }
    if !table().types.contains_key(&type_name) {
        return Err(vec![error(
            &url,
            &url,
            format!("{type_name} is not an R5 type"),
        )]);
    }
    let Some(elements) = definition["snapshot"]["element"].as_array() else {
        // ADR 0053: packages must ship snapshots; nodes never generate them.
        return Err(vec![error(
            &url,
            &url,
            "the profile has no snapshot (differential-only profiles cannot be published)",
        )]);
    };
    let mut root = Rule::default();
    for element in elements {
        let id = element["id"].as_str().unwrap_or_default();
        let parts = segments(id);
        if parts.first().map(|(n, _)| *n) != Some(type_name.as_str()) {
            errors.push(error(
                &url,
                id,
                "element id does not start with the profile's type",
            ));
            continue;
        }
        if parts
            .iter()
            .any(|(_, slice)| slice.is_some_and(|s| s.contains('/')))
        {
            errors.push(error(&url, id, "re-slicing is not supported"));
            continue;
        }
        let mut rule = &mut root;
        if let Some(slice) = parts[0].1 {
            errors.push(error(
                &url,
                id,
                format!("a slice on the root ({slice}) is not supported"),
            ));
            continue;
        }
        for (name, slice) in &parts[1..] {
            rule = rule.children.entry((*name).to_owned()).or_default();
            if let Some(slice) = slice {
                let index = match rule.slices.iter().position(|(n, _)| n == slice) {
                    Some(index) => index,
                    None => {
                        rule.slices.push(((*slice).to_owned(), Rule::default()));
                        rule.slices.len() - 1
                    }
                };
                rule = &mut rule.slices[index].1;
            }
        }
        if let Err(reason) = fill(rule, element, &url) {
            errors.push(error(&url, id, reason));
        }
    }
    check_discriminators(&url, &type_name, &root, &mut errors);
    let contexts = definition["context"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|c| {
            if c["type"].as_str() == Some("element") {
                c["expression"].as_str().map(str::to_owned)
            } else {
                errors.push(error(
                    &url,
                    &url,
                    format!("extension context type {:?} is not supported", c["type"]),
                ));
                None
            }
        })
        .collect();
    if errors.is_empty() {
        Ok(Compiled {
            url,
            type_name,
            root,
            contexts,
        })
    } else {
        Err(errors)
    }
}

fn fill(rule: &mut Rule, element: &Value, url: &str) -> Result<(), String> {
    rule.min = element["min"].as_u64().unwrap_or(0);
    rule.max = element["max"].as_str().and_then(|m| m.parse().ok());
    if element["max"].as_str() == Some("*") {
        rule.max = None;
    }
    rule.types = element["type"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|t| {
            let code = t["code"].as_str()?;
            if code.starts_with("http://hl7.org/fhirpath/") {
                return None;
            }
            let strings = |key: &str| -> Vec<String> {
                t[key]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .map(|url| unversioned(url).to_owned())
                    .collect()
            };
            Some(TypeRule {
                code: code.to_owned(),
                profiles: strings("profile"),
                target_profiles: strings("targetProfile"),
            })
        })
        .collect();
    if let Some(object) = element.as_object() {
        for (key, value) in object {
            if key.starts_with("fixed") {
                rule.fixed = Some(value.clone());
            } else if key.starts_with("pattern") {
                rule.pattern = Some(value.clone());
            }
        }
    }
    rule.constraints = element["constraint"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|c| {
            let source = c["source"].as_str().unwrap_or(url);
            !is_base_type(source) && c["severity"].as_str() == Some("error")
        })
        .filter_map(|c| {
            Some(ProfileConstraint {
                key: c["key"].as_str()?.to_owned(),
                expression: c["expression"].as_str()?.to_owned(),
                human: c["human"].as_str().unwrap_or_default().to_owned(),
            })
        })
        .collect();
    if let Some(slicing) = element.get("slicing") {
        let rules = match slicing["rules"].as_str() {
            Some("closed") => SlicingRules::Closed,
            Some("openAtEnd") => SlicingRules::OpenAtEnd,
            _ => SlicingRules::Open,
        };
        let mut discriminators = Vec::new();
        for d in slicing["discriminator"].as_array().into_iter().flatten() {
            let kind = match d["type"].as_str() {
                Some("value" | "pattern") => Discriminator::Value,
                Some("exists") => Discriminator::Exists,
                Some("type") => Discriminator::Type,
                Some("profile") => Discriminator::Profile,
                other => return Err(format!("discriminator type {other:?} is not supported")),
            };
            let path = d["path"].as_str().unwrap_or("$this").to_owned();
            if path.contains("resolve()") {
                return Err(format!(
                    "discriminator path {path} needs resolve(), which is out of v1"
                ));
            }
            // A choice element is sliced by type only (the HL7 snapshot generator refuses
            // anything else, #166).
            if element["path"].as_str().is_some_and(|p| p.ends_with("[x]"))
                && kind != Discriminator::Type
            {
                return Err(
                    "a choice element can only be sliced with the type discriminator".to_owned(),
                );
            }
            discriminators.push((kind, path));
        }
        rule.slicing = Some(Slicing {
            discriminators,
            rules,
            ordered: slicing["ordered"].as_bool().unwrap_or(false),
        });
    }
    Ok(())
}

/// Every slice must give each `value` discriminator path a fixed or pattern value to compare
/// with; otherwise values could not be assigned to slices.
fn check_discriminators(url: &str, id: &str, rule: &Rule, errors: &mut Vec<CompileError>) {
    if let Some(slicing) = &rule.slicing {
        for (name, slice) in &rule.slices {
            for (kind, path) in &slicing.discriminators {
                if *kind == Discriminator::Value && expected_at(slice, path).is_none() {
                    errors.push(error(
                        url,
                        &format!("{id}:{name}"),
                        format!("no fixed or pattern value at discriminator path {path} (binding discriminators are not supported)"),
                    ));
                }
            }
        }
    }
    for (name, child) in &rule.children {
        check_discriminators(url, &format!("{id}.{name}"), child, errors);
    }
    for (name, slice) in &rule.slices {
        check_discriminators(url, &format!("{id}:{name}"), slice, errors);
    }
}

/// The rule a discriminator path leads to within a slice: `coding.code`, `$this`,
/// `extension('url').value`, `url`.
fn rule_at<'a>(rule: &'a Rule, path: &str) -> Option<&'a Rule> {
    let mut current = rule;
    for step in path_steps(path) {
        current = match step {
            Step::This => current,
            Step::Child(name) => current
                .children
                .get(name)
                .or_else(|| current.children.get(&format!("{name}[x]")))?,
            Step::Extension(url) => {
                let extension = current.children.get("extension")?;
                &extension
                    .slices
                    .iter()
                    .find(|(_, s)| {
                        s.children
                            .get("url")
                            .and_then(|u| u.fixed.as_ref())
                            .and_then(Value::as_str)
                            == Some(url)
                    })?
                    .1
            }
        };
    }
    Some(current)
}

/// The values a `value` discriminator compares with, and whether they are fixed (exact) or
/// patterns. The value may be on the element at the path; on an ancestor along it (a
/// `patternCodeableConcept` on `code` gives the expected `code.coding.code`); or on the
/// required slices of a sliced element along it (`code.coding:SBPCode.code` in the BP
/// profile).
fn expected_at(slice: &Rule, path: &str) -> Option<(Vec<Value>, bool)> {
    expected(slice, &path_steps(path), true)
}

fn expected(rule: &Rule, steps: &[Step<'_>], first: bool) -> Option<(Vec<Value>, bool)> {
    let own = rule
        .fixed
        .as_ref()
        .map(|v| (v, true))
        .or(rule.pattern.as_ref().map(|v| (v, false)));
    let Some((step, rest)) = steps.split_first() else {
        return own.map(|(v, fixed)| (vec![v.clone()], fixed));
    };
    if let Some((value, fixed)) = own
        && !first
    {
        let names: Vec<&str> = steps
            .iter()
            .filter_map(|s| match s {
                Step::Child(n) => Some(*n),
                _ => None,
            })
            .collect();
        let found: Vec<Value> = values_at(value, &names.join("."))
            .into_iter()
            .map(|(v, _)| v.clone())
            .collect();
        return (!found.is_empty()).then_some((found, fixed));
    }
    let next = match step {
        Step::This => return expected(rule, rest, false),
        Step::Child(name) => rule
            .children
            .get(*name)
            .or_else(|| rule.children.get(&format!("{name}[x]")))?,
        Step::Extension(url) => {
            let extension = rule.children.get("extension")?;
            &extension
                .slices
                .iter()
                .find(|(_, s)| {
                    s.children
                        .get("url")
                        .and_then(|u| u.fixed.as_ref())
                        .and_then(Value::as_str)
                        == Some(*url)
                })?
                .1
        }
    };
    expected(next, rest, false).or_else(|| {
        // A sliced element along the path: its required slices give the values.
        let mut values = Vec::new();
        let mut all_fixed = true;
        for (_, nested) in next.slices.iter().filter(|(_, s)| s.min > 0) {
            if let Some((found, fixed)) = expected(nested, rest, false) {
                values.extend(found);
                all_fixed &= fixed;
            }
        }
        (!values.is_empty()).then_some((values, all_fixed))
    })
}

enum Step<'a> {
    This,
    Child(&'a str),
    Extension(&'a str),
}

fn path_steps(path: &str) -> Vec<Step<'_>> {
    let mut steps = Vec::new();
    let mut rest = path;
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix("$this") {
            steps.push(Step::This);
            rest = after.trim_start_matches('.');
        } else if let Some(after) = rest.strip_prefix("extension('") {
            let (url, tail) = after.split_once("')").unwrap_or((after, ""));
            steps.push(Step::Extension(url));
            rest = tail.trim_start_matches('.');
        } else {
            let (name, tail) = rest.split_once('.').unwrap_or((rest, ""));
            steps.push(Step::Child(name));
            rest = tail;
        }
    }
    steps
}

/// The values a discriminator path selects in an instance.
fn values_at<'a>(value: &'a Value, path: &str) -> Vec<(&'a Value, Option<String>)> {
    let mut current: Vec<(&Value, Option<String>)> = vec![(value, None)];
    for step in path_steps(path) {
        let mut next = Vec::new();
        for (v, t) in current {
            match step {
                Step::This => next.push((v, t)),
                Step::Child(name) => {
                    let Some(object) = v.as_object() else {
                        continue;
                    };
                    if let Some(found) = object.get(name) {
                        flatten(found, None, &mut next);
                    } else {
                        for (key, found) in object {
                            if let Some(suffix) = key.strip_prefix(name)
                                && suffix.starts_with(|c: char| c.is_ascii_uppercase())
                            {
                                flatten(found, Some(lower_first(suffix)), &mut next);
                            }
                        }
                    }
                }
                Step::Extension(url) => {
                    for e in v
                        .get("extension")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                    {
                        if e.get("url").and_then(Value::as_str) == Some(url) {
                            next.push((e, None));
                        }
                    }
                }
            }
        }
        current = next;
    }
    current
}

fn flatten<'a>(
    value: &'a Value,
    type_name: Option<String>,
    out: &mut Vec<(&'a Value, Option<String>)>,
) {
    match value {
        Value::Array(items) => out.extend(items.iter().map(|i| (i, type_name.clone()))),
        other => out.push((other, type_name)),
    }
}

/// `Quantity` stays; `DateTime` becomes `dateTime` when it names a primitive.
fn lower_first(suffix: &str) -> String {
    let mut chars = suffix.chars();
    let lowered: String = chars
        .next()
        .map(|c| c.to_ascii_lowercase().to_string() + chars.as_str())
        .unwrap_or_default();
    if table().primitives.contains_key(&lowered) {
        lowered
    } else {
        suffix.to_owned()
    }
}

/// `fixed[x]`: equal, with no extra content.
fn equals_fixed(fixed: &Value, value: &Value) -> bool {
    fixed == value
}

/// `pattern[x]`: every element of the pattern is present with the same value; arrays match if
/// each pattern item matches some instance item.
fn matches_pattern(pattern: &Value, value: &Value) -> bool {
    match (pattern, value) {
        (Value::Object(p), Value::Object(v)) => p
            .iter()
            .all(|(k, pv)| v.get(k).is_some_and(|vv| matches_pattern(pv, vv))),
        (Value::Array(p), Value::Array(v)) => p
            .iter()
            .all(|pi| v.iter().any(|vi| matches_pattern(pi, vi))),
        (p, Value::Array(v)) => v.iter().any(|vi| matches_pattern(p, vi)),
        (p, v) => p == v,
    }
}

/// One value of an element, with where it is and its type.
struct Item<'a> {
    value: &'a Value,
    /// The JSON location, e.g. `Observation.category[0]`.
    location: String,
    /// A FHIRPath expression selecting exactly this value from the root.
    fhirpath: String,
    type_name: String,
}

struct Check {
    location: String,
    key: String,
    human: String,
    expression: String,
}

struct Walker<'a> {
    set: &'a ProfileSet,
    profile: &'a str,
    issues: Vec<Issue>,
    checks: Vec<Check>,
    /// Report constraint expressions (false while matching `profile` discriminators).
    collect_checks: bool,
}

impl Walker<'_> {
    /// Records an issue once: an extension is checked both as a slice's type profile and as
    /// an extension definition, and the same failure must not be reported twice.
    fn issue(&mut self, code: IssueCode, location: &str, message: String) {
        let issue = Issue {
            code,
            location: location.to_owned(),
            message: format!("{message} (profile {})", self.profile),
        };
        if !self.issues.contains(&issue) {
            self.issues.push(issue);
        }
    }

    /// The values of one element (all of them, for cardinality and slicing).
    fn element(&mut self, rule: &Rule, items: &[Item<'_>], parent: &str, name: &str) {
        let here = element_location(items, parent, name);
        let count = items.len() as u64;
        if count < rule.min {
            self.issue(
                IssueCode::Required,
                &here,
                format!("{name} needs at least {} value(s)", rule.min),
            );
        }
        if let Some(max) = rule.max
            && count > max
        {
            self.issue(
                IssueCode::Structure,
                &here,
                format!("{name} allows at most {max} value(s)"),
            );
        }
        for item in items {
            self.value(rule, item);
        }
        if let Some(slicing) = &rule.slicing {
            self.slicing(rule, slicing, items, parent, name);
        }
    }

    fn slicing(
        &mut self,
        rule: &Rule,
        slicing: &Slicing,
        items: &[Item<'_>],
        parent: &str,
        name: &str,
    ) {
        let here = element_location(items, parent, name);
        let mut assigned: Vec<Option<usize>> = Vec::with_capacity(items.len());
        for item in items {
            assigned.push(
                rule.slices
                    .iter()
                    .position(|(_, slice)| self.in_slice(slicing, slice, item)),
            );
        }
        for (index, (slice_name, slice)) in rule.slices.iter().enumerate() {
            let members: Vec<&Item<'_>> = items
                .iter()
                .zip(&assigned)
                .filter(|(_, a)| **a == Some(index))
                .map(|(i, _)| i)
                .collect();
            let count = members.len() as u64;
            if count < slice.min {
                self.issue(
                    IssueCode::Required,
                    &here,
                    format!(
                        "slice {slice_name} of {name} needs at least {} value(s)",
                        slice.min
                    ),
                );
            }
            if let Some(max) = slice.max
                && count > max
            {
                self.issue(
                    IssueCode::Structure,
                    &here,
                    format!("slice {slice_name} of {name} allows at most {max} value(s)"),
                );
            }
            for item in members {
                self.value(slice, item);
            }
        }
        let unmatched: Vec<usize> = assigned
            .iter()
            .enumerate()
            .filter(|(_, a)| a.is_none())
            .map(|(i, _)| i)
            .collect();
        match slicing.rules {
            SlicingRules::Closed => {
                for i in unmatched {
                    self.issue(
                        IssueCode::Structure,
                        &items[i].location,
                        format!("{name} is closed: this value matches no slice"),
                    );
                }
            }
            SlicingRules::OpenAtEnd => {
                let last_matched = assigned.iter().rposition(Option::is_some);
                for i in unmatched {
                    if last_matched.is_some_and(|last| i < last) {
                        self.issue(
                            IssueCode::Structure,
                            &items[i].location,
                            format!(
                                "{name} is open at the end: unsliced values must follow the slices"
                            ),
                        );
                    }
                }
            }
            SlicingRules::Open => {}
        }
        if slicing.ordered {
            let order: Vec<usize> = assigned.iter().flatten().copied().collect();
            if order.windows(2).any(|w| w[0] > w[1]) {
                self.issue(
                    IssueCode::Structure,
                    &here,
                    format!("the slices of {name} must appear in order"),
                );
            }
        }
    }

    fn in_slice(&self, slicing: &Slicing, slice: &Rule, item: &Item<'_>) -> bool {
        slicing.discriminators.iter().all(|(kind, path)| {
            let found = values_at(item.value, path);
            match kind {
                Discriminator::Value => {
                    let Some((expected, fixed)) = expected_at(slice, path) else {
                        return false;
                    };
                    expected.iter().all(|e| {
                        found.iter().any(|(v, _)| {
                            if fixed {
                                equals_fixed(e, v)
                            } else {
                                matches_pattern(e, v)
                            }
                        })
                    })
                }
                Discriminator::Exists => {
                    let required = rule_at(slice, path).is_some_and(|r| r.min > 0);
                    required == !found.is_empty()
                }
                Discriminator::Type => {
                    let allowed: Vec<&str> = rule_at(slice, path)
                        .map(|r| r.types.iter().map(|t| t.code.as_str()).collect())
                        .unwrap_or_default();
                    let type_name = if path == "$this" {
                        Some(item.type_name.clone())
                    } else {
                        found.first().and_then(|(_, t)| t.clone())
                    };
                    type_name.is_some_and(|t| allowed.contains(&t.as_str()))
                }
                Discriminator::Profile => {
                    let Some(rule) = rule_at(slice, path) else {
                        return false;
                    };
                    let profiles: Vec<&str> = rule
                        .types
                        .iter()
                        .flat_map(|t| t.profiles.iter().map(String::as_str))
                        .collect();
                    found
                        .iter()
                        .any(|(v, _)| profiles.iter().any(|p| self.conforms(p, v)))
                }
            }
        })
    }

    /// Whether `value` meets profile `url` structurally (cardinality, fixed, pattern, types,
    /// slicing); used by `profile` discriminators.
    fn conforms(&self, url: &str, value: &Value) -> bool {
        let Some(compiled) = self.set.profiles.get(url) else {
            return false;
        };
        let mut walker = Walker {
            set: self.set,
            profile: &compiled.url,
            issues: Vec::new(),
            checks: Vec::new(),
            collect_checks: false,
        };
        let item = Item {
            value,
            location: compiled.type_name.clone(),
            fhirpath: compiled.type_name.clone(),
            type_name: compiled.type_name.clone(),
        };
        walker.value(&compiled.root, &item);
        walker.issues.is_empty()
    }

    /// One value: fixed, pattern, type and target restrictions, invariants, then its members.
    fn value(&mut self, rule: &Rule, item: &Item<'_>) {
        if let Some(fixed) = &rule.fixed
            && !equals_fixed(fixed, item.value)
        {
            self.issue(
                IssueCode::Value,
                &item.location,
                format!("must be exactly {fixed}"),
            );
        }
        if let Some(pattern) = &rule.pattern
            && !matches_pattern(pattern, item.value)
        {
            self.issue(
                IssueCode::Value,
                &item.location,
                format!("must match the pattern {pattern}"),
            );
        }
        if !rule.types.is_empty()
            && !rule
                .types
                .iter()
                .any(|t| t.code == item.type_name || is_resource_slot(&t.code))
        {
            let allowed: Vec<&str> = rule.types.iter().map(|t| t.code.as_str()).collect();
            self.issue(
                IssueCode::Structure,
                &item.location,
                format!(
                    "type {} is not allowed here; allowed: {}",
                    item.type_name,
                    allowed.join(", ")
                ),
            );
        }
        self.targets(rule, item);
        if self.collect_checks {
            for c in &rule.constraints {
                self.checks.push(Check {
                    location: item.location.clone(),
                    key: c.key.clone(),
                    human: c.human.clone(),
                    expression: format!("{}.all({})", item.fhirpath, c.expression),
                });
            }
        }
        for t in rule.types.iter().filter(|t| t.code == item.type_name) {
            for profile in t.profiles.iter().filter(|p| !is_base_type(p)) {
                if let Some(compiled) = self.set.profiles.get(profile).cloned() {
                    self.value(&compiled.root, item);
                }
            }
        }
        if let Some(object) = item.value.as_object() {
            self.members(rule, object, item);
        }
    }

    /// `targetProfile`: a literal reference must name a resource type the target profiles
    /// allow. Whether the target conforms to a target profile needs the target itself, which
    /// is out of v1.
    fn targets(&mut self, rule: &Rule, item: &Item<'_>) {
        let allowed: Vec<String> = rule
            .types
            .iter()
            .filter(|t| t.code == "Reference" || t.code == "canonical")
            .flat_map(|t| t.target_profiles.iter())
            .filter_map(|url| match url.strip_prefix(SD) {
                Some(name) if is_base_type(url) => Some(name.to_owned()),
                _ => self.set.profiles.get(url).map(|p| p.type_name.clone()),
            })
            .collect();
        if allowed.is_empty() || allowed.iter().any(|t| t == "Resource") {
            return;
        }
        let Some(reference) = item.value.get("reference").and_then(Value::as_str) else {
            return;
        };
        if let Some(target) = reference_type(reference)
            && !allowed.iter().any(|t| t == target)
        {
            self.issue(
                IssueCode::Structure,
                &item.location,
                format!(
                    "a reference to {target} is not allowed here; allowed: {}",
                    allowed.join(", ")
                ),
            );
        }
    }

    fn members(&mut self, rule: &Rule, object: &Map<String, Value>, item: &Item<'_>) {
        for (name, child) in &rule.children {
            let mut items = Vec::new();
            match name.strip_suffix("[x]") {
                Some(stem) => {
                    for (key, value) in object {
                        let Some(suffix) = key.strip_prefix(stem) else {
                            continue;
                        };
                        if !suffix.starts_with(|c: char| c.is_ascii_uppercase()) {
                            continue;
                        }
                        let type_name = lower_first(suffix);
                        let fhirpath = format!("{}.{stem}.ofType({type_name})", item.fhirpath);
                        push_items(
                            value,
                            &format!("{}.{key}", item.location),
                            &fhirpath,
                            &type_name,
                            &mut items,
                        );
                    }
                }
                None => {
                    if let Some(value) = object.get(name) {
                        // A sliced element may leave its type to its slices.
                        let type_name = child
                            .types
                            .first()
                            .or_else(|| child.slices.iter().find_map(|(_, s)| s.types.first()))
                            .map_or_else(|| "BackboneElement".to_owned(), |t| t.code.clone());
                        let type_name = if is_resource_slot(&type_name) {
                            value
                                .get("resourceType")
                                .and_then(Value::as_str)
                                .unwrap_or("Resource")
                                .to_owned()
                        } else {
                            type_name
                        };
                        push_items(
                            value,
                            &format!("{}.{name}", item.location),
                            &format!("{}.{name}", item.fhirpath),
                            &type_name,
                            &mut items,
                        );
                    }
                }
            }
            // Type slices of a choice element name the JSON key: `value[x]:valueQuantity`.
            self.element(child, &items, &item.location, name);
            if name == "extension" || name == "modifierExtension" {
                self.extensions(&items, &item.location, &item.type_name);
            }
        }
        self.unconstrained(rule, object, item);
    }

    /// Members the profile does not constrain. A snapshot does not expand datatypes, so the
    /// extensions below them (`Patient.name[0].extension`) are reached through the core table.
    fn unconstrained(&mut self, rule: &Rule, object: &Map<String, Value>, item: &Item<'_>) {
        let Some(def) = table().types.get(&item.type_name) else {
            return;
        };
        for (key, value) in object {
            let constrained = rule.children.contains_key(key)
                || rule.children.keys().any(|name| {
                    name.strip_suffix("[x]")
                        .and_then(|stem| key.strip_prefix(stem))
                        .is_some_and(|suffix| suffix.starts_with(|c: char| c.is_ascii_uppercase()))
                });
            if constrained || key == "resourceType" || key.starts_with('_') {
                continue;
            }
            let Some((_, _, chosen)) = resolve(def, "", key) else {
                continue;
            };
            if is_primitive(&chosen) || is_resource_slot(&chosen) || chosen == "BackboneElement" {
                continue;
            }
            let mut items = Vec::new();
            push_items(
                value,
                &format!("{}.{key}", item.location),
                &format!("{}.{key}", item.fhirpath),
                &chosen,
                &mut items,
            );
            if key == "extension" || key == "modifierExtension" {
                self.extensions(&items, &item.location, &item.type_name);
            }
            for child in &items {
                let defined = child
                    .value
                    .get("url")
                    .and_then(Value::as_str)
                    .is_some_and(|url| self.set.profiles.contains_key(url));
                if !defined {
                    self.value(&Rule::default(), child);
                }
            }
        }
    }

    /// Extensions with a definition in the set: their value and their context.
    fn extensions(&mut self, items: &[Item<'_>], parent: &str, parent_type: &str) {
        for item in items {
            let Some(url) = item.value.get("url").and_then(Value::as_str) else {
                continue;
            };
            let Some(definition) = self.set.profiles.get(url).cloned() else {
                continue;
            };
            if !definition.contexts.is_empty() {
                let path = element_path(parent);
                let ok = definition.contexts.iter().any(|c| {
                    c == &path
                        || c == parent_type
                        || c == "Element"
                        || (c == "Resource"
                            && table()
                                .types
                                .get(parent_type)
                                .is_some_and(|d| d.kind == "resource"))
                });
                if !ok {
                    self.issue(
                        IssueCode::Structure,
                        &item.location,
                        format!(
                            "extension {url} is not allowed on {path}; its contexts are {}",
                            definition.contexts.join(", ")
                        ),
                    );
                }
            }
            self.value(&definition.root, item);
        }
    }
}

/// Where an element's own issues are reported: `Patient.identifier`, or for a choice element
/// the key present in the JSON (`Patient.deceasedDateTime`), not `Patient.deceased[x]`.
fn element_location(items: &[Item<'_>], parent: &str, name: &str) -> String {
    match items.first() {
        Some(item) if name.ends_with("[x]") => item.location.clone(),
        _ => format!("{parent}.{name}"),
    }
}

fn is_resource_slot(code: &str) -> bool {
    matches!(code, "Resource" | "DomainResource")
}

/// `Observation.component[0].code` → `Observation.component.code`, for extension contexts.
fn element_path(location: &str) -> String {
    let mut out = String::with_capacity(location.len());
    let mut depth = 0;
    for c in location.chars() {
        match c {
            '[' => depth += 1,
            ']' => depth -= 1,
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out
}

fn reference_type(reference: &str) -> Option<&str> {
    let without_history = reference
        .find("/_history/")
        .map_or(reference, |at| &reference[..at]);
    let mut segments = without_history.rsplit('/');
    let _id = segments.next()?;
    let resource_type = segments.next()?;
    (resource_type.starts_with(|c: char| c.is_ascii_uppercase()) && !reference.starts_with('#'))
        .then_some(resource_type)
}

fn push_items<'a>(
    value: &'a Value,
    location: &str,
    fhirpath: &str,
    type_name: &str,
    out: &mut Vec<Item<'a>>,
) {
    match value {
        Value::Array(values) => {
            for (i, v) in values.iter().enumerate() {
                out.push(Item {
                    value: v,
                    location: format!("{location}[{i}]"),
                    fhirpath: format!("{fhirpath}[{i}]"),
                    type_name: type_name.to_owned(),
                });
            }
        }
        single => out.push(Item {
            value: single,
            location: location.to_owned(),
            fhirpath: fhirpath.to_owned(),
            type_name: type_name.to_owned(),
        }),
    }
}

/// Tier 3 behind a trait (ADR 0053): raw JSON plus the profiles to check, returning issues.
pub trait ProfileValidator: Send + Sync {
    /// Validates `resource`, which passed tiers 1–2, against `profiles` (canonical URLs).
    fn validate<'a>(
        &'a self,
        resource: &'a Value,
        profiles: &'a [String],
    ) -> Pin<Box<dyn Future<Output = Vec<Issue>> + Send + 'a>>;
}

/// The snapshot-driven validator over a compiled [`ProfileSet`].
pub struct SnapshotValidator {
    set: Arc<ProfileSet>,
    engine: Arc<dyn FhirPathEngine>,
}

impl SnapshotValidator {
    /// A validator over `set`, evaluating profile invariants with `engine`.
    pub fn new(set: Arc<ProfileSet>, engine: Arc<dyn FhirPathEngine>) -> Self {
        Self { set, engine }
    }
}

impl ProfileValidator for SnapshotValidator {
    fn validate<'a>(
        &'a self,
        resource: &'a Value,
        profiles: &'a [String],
    ) -> Pin<Box<dyn Future<Output = Vec<Issue>> + Send + 'a>> {
        Box::pin(async move {
            let mut issues = Vec::new();
            // A profile's snapshot repeats the invariants of the profiles it derives from;
            // each is evaluated once.
            let mut evaluated = BTreeSet::new();
            let resource_type = resource
                .get("resourceType")
                .and_then(Value::as_str)
                .unwrap_or_default();
            for url in profiles {
                let Some(compiled) = self.set.profiles.get(unversioned(url)) else {
                    issues.push(Issue {
                        code: IssueCode::Processing,
                        location: resource_type.to_owned(),
                        message: format!("profile {url} is not in the active set"),
                    });
                    continue;
                };
                if compiled.type_name != resource_type {
                    issues.push(Issue {
                        code: IssueCode::Structure,
                        location: resource_type.to_owned(),
                        message: format!(
                            "profile {url} is for {}, not {resource_type}",
                            compiled.type_name
                        ),
                    });
                    continue;
                }
                let mut walker = Walker {
                    set: &self.set,
                    profile: &compiled.url,
                    issues: Vec::new(),
                    checks: Vec::new(),
                    collect_checks: true,
                };
                let item = Item {
                    value: resource,
                    location: resource_type.to_owned(),
                    fhirpath: resource_type.to_owned(),
                    type_name: resource_type.to_owned(),
                };
                walker.value(&compiled.root, &item);
                walker
                    .checks
                    .retain(|c| evaluated.insert((c.location.clone(), c.key.clone())));
                let expressions: Vec<String> =
                    walker.checks.iter().map(|c| c.expression.clone()).collect();
                let results = if expressions.is_empty() {
                    Vec::new()
                } else {
                    self.engine.evaluate_many(resource, &expressions).await
                };
                for (check, result) in walker.checks.iter().zip(results) {
                    match result {
                        Ok(items)
                            if matches!(items.as_slice(), [] | [FhirPathItem::Boolean(true)]) => {}
                        Ok(_) => walker.issues.push(Issue {
                            code: IssueCode::Invariant,
                            location: check.location.clone(),
                            message: format!("{}: {} (profile {url})", check.key, check.human),
                        }),
                        Err(failure) => walker.issues.push(Issue {
                            code: IssueCode::Processing,
                            location: check.location.clone(),
                            message: format!(
                                "{} could not be evaluated: {failure} (profile {url})",
                                check.key
                            ),
                        }),
                    }
                }
                issues.append(&mut walker.issues);
            }
            issues
        })
    }
}

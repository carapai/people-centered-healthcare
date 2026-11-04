//! Terminology (terminology.md; ADRs 0003, 0024): the operations every node serves over its own
//! terminology, with no external terminology server (#90).
//!
//! - [`Terminology`] holds the active CodeSystems, ValueSets and ConceptMaps, each by canonical
//!   URL and version. A reference without a version resolves to the latest one held, by semver
//!   when both versions are semver.
//! - [`Terminology::lookup`] (`$lookup`): a code's display, definition, designations,
//!   properties, parents and children.
//! - [`Terminology::subsumes`] (`$subsumes`): how two codes relate in their code system's
//!   hierarchy.
//! - [`Terminology::expand`] (`$expand`): a ValueSet's codes. Its pinned expansion, which its
//!   package ships, is used when it has one; otherwise the compose is computed from local
//!   CodeSystems. Text filtering and paging apply either way.
//! - [`Terminology::validate_code`] (`$validate-code`): whether a code is in a ValueSet or a
//!   CodeSystem, and whether its display is right.
//! - [`Terminology::translate`] (`$translate`): a code's targets in a ConceptMap.
//!
//! **Offline, so fail closed.** What would need another terminology server is an error, never
//! an empty answer: a code system that is not held ([`Error::UnknownCodeSystem`]), a value set
//! that is not held, a filter on a code system held only in part, or a `regex` filter.

mod model;

use std::collections::{BTreeMap, BTreeSet};

pub use model::{
    Canonical, CodeSystem, Concept, ConceptMap, Contains, Content, Designation, Filter, FilterOp,
    Group, Listed, Mapping, Property, PropertyValue, Relationship, Rule, Target, ValueSet,
};

/// Why an operation could not answer.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// The code system is not held on this node.
    #[error("the code system {0} is not available on this node")]
    UnknownCodeSystem(String),
    /// The value set is not held on this node.
    #[error("the value set {0} is not available on this node")]
    UnknownValueSet(String),
    /// The concept map is not held on this node.
    #[error("the concept map {0} is not available on this node")]
    UnknownConceptMap(String),
    /// The code is not in its code system.
    #[error("{code} is not a code in {system}")]
    UnknownCode {
        /// The code system.
        system: String,
        /// The code.
        code: String,
    },
    /// A filter on a code system held only in part, which could miss codes.
    #[error("{0} is not held completely, so it cannot be filtered")]
    Incomplete(String),
    /// A filter this service cannot apply.
    #[error("the filter {property} {op:?} {value} is not supported")]
    UnsupportedFilter {
        /// Its property.
        property: String,
        /// Its operator.
        op: FilterOp,
        /// Its value.
        value: String,
    },
    /// A value set that includes itself, directly or not.
    #[error("the value set {0} includes itself")]
    Cycle(String),
}

/// What [`Terminology::lookup`] finds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lookup {
    /// The code system's URL.
    pub system: String,
    /// Its version.
    pub version: Option<String>,
    /// The concept.
    pub concept: Concept,
    /// The codes directly above it.
    pub parents: Vec<String>,
    /// The codes directly under it.
    pub children: Vec<String>,
}

/// How two codes relate in their code system (`$subsumes`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Subsumption {
    /// The same concept.
    Equivalent,
    /// The first subsumes the second.
    Subsumes,
    /// The second subsumes the first.
    SubsumedBy,
    /// Neither subsumes the other.
    NotSubsumed,
}

/// What `$expand` was asked for.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExpandRequest {
    /// Only codes whose code or display contains this text, ignoring case.
    pub filter: Option<String>,
    /// Leave inactive codes out, unless the value set's compose says to keep them.
    pub active_only: bool,
    /// Skip this many codes.
    pub offset: usize,
    /// Return at most this many codes.
    pub count: Option<usize>,
}

/// An expansion: the codes in order, and how many there are before paging.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expansion {
    /// The value set expanded, `url|version`.
    pub value_set: String,
    /// Codes before paging.
    pub total: usize,
    /// Whether it is the package's pinned expansion rather than a computed one.
    pub pinned: bool,
    /// The codes on this page.
    pub contains: Vec<Contains>,
}

/// What `$validate-code` found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Validation {
    /// The code is valid, and so is its display if one was given.
    Valid {
        /// The code's display.
        display: Option<String>,
    },
    /// The code is valid, but the display given is not its display.
    WrongDisplay {
        /// The code's display.
        expected: Option<String>,
    },
    /// The code is not valid here.
    Invalid,
}

/// One target `$translate` found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Translation {
    /// The ConceptMap that maps it, `url|version`.
    pub concept_map: String,
    /// The target code system.
    pub system: String,
    /// The target.
    pub target: Target,
}

type Versions<T> = BTreeMap<String, BTreeMap<String, T>>;

/// A node's terminology: what it serves from.
#[derive(Debug, Clone, Default)]
pub struct Terminology {
    code_systems: Versions<CodeSystem>,
    value_sets: Versions<ValueSet>,
    concept_maps: Versions<ConceptMap>,
}

/// Versions compare as semver when both are, otherwise as text.
fn newer(a: &str, b: &str) -> std::cmp::Ordering {
    let parse = |text: &str| -> Option<Vec<u64>> {
        let core = text.split(['-', '+']).next()?;
        core.split('.').map(|part| part.parse().ok()).collect()
    };
    match (parse(a), parse(b)) {
        (Some(x), Some(y)) => x.cmp(&y),
        _ => a.cmp(b),
    }
}

fn resolve<'a, T>(map: &'a Versions<T>, url: &str, version: Option<&str>) -> Option<&'a T> {
    let versions = map.get(url)?;
    match version {
        Some(version) => versions.get(version),
        None => versions
            .iter()
            .max_by(|(a, _), (b, _)| newer(a, b))
            .map(|(_, item)| item),
    }
}

fn label(url: &str, version: Option<&str>) -> String {
    match version {
        Some(version) => format!("{url}|{version}"),
        None => url.to_owned(),
    }
}

/// A code system's concepts by code, each with its parents and children.
struct Index<'a> {
    system: &'a CodeSystem,
    concepts: BTreeMap<String, &'a Concept>,
    parents: BTreeMap<String, BTreeSet<String>>,
    children: BTreeMap<String, BTreeSet<String>>,
}

impl<'a> Index<'a> {
    fn new(system: &'a CodeSystem) -> Self {
        let mut index = Self {
            system,
            concepts: BTreeMap::new(),
            parents: BTreeMap::new(),
            children: BTreeMap::new(),
        };
        index.walk(&system.concepts, None);
        // A `parent` property states a parent too, for a code system written flat.
        let stated: Vec<(String, String)> = index
            .concepts
            .values()
            .flat_map(|concept| {
                concept
                    .properties
                    .iter()
                    .filter(|p| p.code == "parent")
                    .map(|p| (concept.code.clone(), p.value.text()))
            })
            .collect();
        for (child, parent) in stated {
            index.link(&parent, &child);
        }
        index
    }

    fn walk(&mut self, concepts: &'a [Concept], parent: Option<&str>) {
        for concept in concepts {
            self.concepts.insert(self.key(&concept.code), concept);
            if let Some(parent) = parent {
                self.link(parent, &concept.code);
            }
            self.walk(&concept.children, Some(&concept.code));
        }
    }

    fn link(&mut self, parent: &str, child: &str) {
        let (parent, child) = (self.key(parent), self.key(child));
        self.parents
            .entry(child.clone())
            .or_default()
            .insert(parent.clone());
        self.children.entry(parent).or_default().insert(child);
    }

    fn key(&self, code: &str) -> String {
        if self.system.case_sensitive {
            code.to_owned()
        } else {
            code.to_lowercase()
        }
    }

    fn get(&self, code: &str) -> Option<&'a Concept> {
        self.concepts.get(&self.key(code)).copied()
    }

    /// Every code under `code`, not `code` itself.
    fn descendants(&self, code: &str) -> BTreeSet<String> {
        let mut found = BTreeSet::new();
        let mut stack = vec![self.key(code)];
        while let Some(next) = stack.pop() {
            for child in self.children.get(&next).into_iter().flatten() {
                if found.insert(child.clone()) {
                    stack.push(child.clone());
                }
            }
        }
        found
    }

    /// Every code above `code`, not `code` itself.
    fn ancestors(&self, code: &str) -> BTreeSet<String> {
        let mut found = BTreeSet::new();
        let mut stack = vec![self.key(code)];
        while let Some(next) = stack.pop() {
            for parent in self.parents.get(&next).into_iter().flatten() {
                if found.insert(parent.clone()) {
                    stack.push(parent.clone());
                }
            }
        }
        found
    }

    fn contains(&self, concept: &Concept) -> Contains {
        Contains {
            system: self.system.url.clone(),
            version: self.system.version.clone(),
            code: concept.code.clone(),
            display: concept.display.clone(),
            inactive: concept.is_inactive(),
        }
    }

    /// Whether `concept` passes `filter`.
    fn passes(&self, concept: &Concept, filter: &Filter) -> Result<bool, Error> {
        let key = self.key(&concept.code);
        let value = self.key(&filter.value);
        let property_values = || -> Vec<String> {
            concept
                .properties
                .iter()
                .filter(|p| p.code == filter.property)
                .map(|p| p.value.text())
                .collect()
        };
        let list = || -> Vec<String> {
            filter
                .value
                .split(',')
                .map(|v| v.trim().to_owned())
                .collect()
        };
        let hierarchy = filter.property == "concept";
        Ok(match filter.op {
            FilterOp::IsA if hierarchy => key == value || self.ancestors(&key).contains(&value),
            FilterOp::DescendentOf if hierarchy => self.ancestors(&key).contains(&value),
            FilterOp::IsNotA if hierarchy => key != value && !self.ancestors(&key).contains(&value),
            FilterOp::Generalizes if hierarchy => {
                key == value || self.descendants(&key).contains(&value)
            }
            FilterOp::ChildOf if hierarchy => self
                .parents
                .get(&key)
                .is_some_and(|parents| parents.contains(&value)),
            FilterOp::In if hierarchy => list().iter().any(|v| self.key(v) == key),
            FilterOp::NotIn if hierarchy => !list().iter().any(|v| self.key(v) == key),
            FilterOp::Equals if !hierarchy => property_values().contains(&filter.value),
            FilterOp::In if !hierarchy => {
                let values = property_values();
                list().iter().any(|v| values.contains(v))
            }
            FilterOp::NotIn if !hierarchy => {
                let values = property_values();
                !list().iter().any(|v| values.contains(v))
            }
            FilterOp::Exists if !hierarchy => {
                property_values().is_empty() == (filter.value == "false")
            }
            _ => {
                return Err(Error::UnsupportedFilter {
                    property: filter.property.clone(),
                    op: filter.op,
                    value: filter.value.clone(),
                });
            }
        })
    }
}

impl Terminology {
    /// An empty terminology.
    pub fn new() -> Self {
        Self::default()
    }

    /// Holds `system`, replacing the same version.
    pub fn add_code_system(&mut self, system: CodeSystem) {
        let version = system.version.clone().unwrap_or_default();
        self.code_systems
            .entry(system.url.clone())
            .or_default()
            .insert(version, system);
    }

    /// Holds `value_set`, replacing the same version.
    pub fn add_value_set(&mut self, value_set: ValueSet) {
        let version = value_set.version.clone().unwrap_or_default();
        self.value_sets
            .entry(value_set.url.clone())
            .or_default()
            .insert(version, value_set);
    }

    /// Holds `map`, replacing the same version.
    pub fn add_concept_map(&mut self, map: ConceptMap) {
        let version = map.version.clone().unwrap_or_default();
        self.concept_maps
            .entry(map.url.clone())
            .or_default()
            .insert(version, map);
    }

    fn code_system(&self, url: &str, version: Option<&str>) -> Result<&CodeSystem, Error> {
        resolve(&self.code_systems, url, version)
            .ok_or_else(|| Error::UnknownCodeSystem(label(url, version)))
    }

    fn value_set(&self, reference: &str) -> Result<&ValueSet, Error> {
        let canonical = Canonical::parse(reference);
        resolve(
            &self.value_sets,
            &canonical.url,
            canonical.version.as_deref(),
        )
        .ok_or_else(|| Error::UnknownValueSet(reference.to_owned()))
    }

    /// `$lookup`: `code` in `system` (at `version`, or the latest held).
    pub fn lookup(&self, system: &str, version: Option<&str>, code: &str) -> Result<Lookup, Error> {
        let code_system = self.code_system(system, version)?;
        let index = Index::new(code_system);
        let concept = index.get(code).ok_or_else(|| Error::UnknownCode {
            system: system.to_owned(),
            code: code.to_owned(),
        })?;
        let key = index.key(&concept.code);
        let codes = |set: Option<&BTreeSet<String>>| -> Vec<String> {
            set.into_iter()
                .flatten()
                .filter_map(|k| index.concepts.get(k).map(|c| c.code.clone()))
                .collect()
        };
        Ok(Lookup {
            system: code_system.url.clone(),
            version: code_system.version.clone(),
            parents: codes(index.parents.get(&key)),
            children: codes(index.children.get(&key)),
            concept: Concept {
                children: Vec::new(),
                ..concept.clone()
            },
        })
    }

    /// `$subsumes`: how `a` relates to `b` in `system`.
    pub fn subsumes(
        &self,
        system: &str,
        version: Option<&str>,
        a: &str,
        b: &str,
    ) -> Result<Subsumption, Error> {
        let code_system = self.code_system(system, version)?;
        let index = Index::new(code_system);
        for code in [a, b] {
            if index.get(code).is_none() {
                return Err(Error::UnknownCode {
                    system: system.to_owned(),
                    code: code.to_owned(),
                });
            }
        }
        let (a, b) = (index.key(a), index.key(b));
        Ok(if a == b {
            Subsumption::Equivalent
        } else if index.descendants(&a).contains(&b) {
            Subsumption::Subsumes
        } else if index.ancestors(&a).contains(&b) {
            Subsumption::SubsumedBy
        } else {
            Subsumption::NotSubsumed
        })
    }

    /// The codes `rule` selects, by `(system, code)`.
    fn select(
        &self,
        rule: &Rule,
        seen: &mut Vec<String>,
    ) -> Result<BTreeMap<(String, String), Contains>, Error> {
        let mut selected: Option<BTreeMap<(String, String), Contains>> = None;
        if let Some(system) = &rule.system {
            let mut codes = BTreeMap::new();
            let held = resolve(&self.code_systems, system, rule.version.as_deref());
            let complete = || -> Result<&CodeSystem, Error> {
                let code_system = held.ok_or_else(|| {
                    Error::UnknownCodeSystem(label(system, rule.version.as_deref()))
                })?;
                if code_system.content == Content::Complete {
                    Ok(code_system)
                } else {
                    Err(Error::Incomplete(system.clone()))
                }
            };
            if !rule.filters.is_empty() {
                complete()?;
            }
            if rule.concepts.is_empty() {
                let index = Index::new(complete()?);
                for concept in index.concepts.values() {
                    let mut passes = true;
                    for filter in &rule.filters {
                        passes &= index.passes(concept, filter)?;
                    }
                    if passes {
                        let contains = index.contains(concept);
                        codes.insert((system.clone(), contains.code.clone()), contains);
                    }
                }
            } else {
                // Listed codes: checked against the code system when it is held, and taken as
                // listed when it is not, which is how a pinned subset of a large external code
                // system arrives.
                let index = held.map(Index::new);
                for listed in &rule.concepts {
                    let contains = match &index {
                        Some(index) => {
                            let concept =
                                index.get(&listed.code).ok_or_else(|| Error::UnknownCode {
                                    system: system.clone(),
                                    code: listed.code.clone(),
                                })?;
                            let mut contains = index.contains(concept);
                            if listed.display.is_some() {
                                contains.display.clone_from(&listed.display);
                            }
                            contains
                        }
                        None => Contains {
                            system: system.clone(),
                            version: rule.version.clone(),
                            code: listed.code.clone(),
                            display: listed.display.clone(),
                            inactive: false,
                        },
                    };
                    let mut passes = true;
                    if let Some(index) = &index
                        && let Some(concept) = index.get(&listed.code)
                    {
                        for filter in &rule.filters {
                            passes &= index.passes(concept, filter)?;
                        }
                    }
                    if passes {
                        codes.insert((system.clone(), contains.code.clone()), contains);
                    }
                }
            }
            selected = Some(codes);
        }
        for reference in &rule.value_sets {
            let codes: BTreeMap<_, _> = self
                .compute(reference, seen)?
                .into_iter()
                .map(|c| ((c.system.clone(), c.code.clone()), c))
                .collect();
            selected = Some(match selected {
                None => codes,
                Some(previous) => previous
                    .into_iter()
                    .filter(|(key, _)| codes.contains_key(key))
                    .collect(),
            });
        }
        Ok(selected.unwrap_or_default())
    }

    /// Every code in value set `reference`, inactive ones included, in system and code order.
    fn compute(&self, reference: &str, seen: &mut Vec<String>) -> Result<Vec<Contains>, Error> {
        let value_set = self.value_set(reference)?;
        let id = label(&value_set.url, value_set.version.as_deref());
        if seen.contains(&id) {
            return Err(Error::Cycle(id));
        }
        if let Some(expansion) = &value_set.expansion {
            return Ok(expansion.clone());
        }
        seen.push(id);
        let mut codes = BTreeMap::new();
        for rule in &value_set.include {
            codes.extend(self.select(rule, seen)?);
        }
        for rule in &value_set.exclude {
            for key in self.select(rule, seen)?.keys() {
                codes.remove(key);
            }
        }
        seen.pop();
        let mut codes: Vec<Contains> = codes.into_values().collect();
        if value_set.inactive == Some(false) {
            codes.retain(|c| !c.inactive);
        }
        Ok(codes)
    }

    /// `$expand`: value set `reference` (`url` or `url|version`), filtered and paged.
    pub fn expand(&self, reference: &str, request: &ExpandRequest) -> Result<Expansion, Error> {
        let value_set = self.value_set(reference)?;
        let mut codes = self.compute(reference, &mut Vec::new())?;
        if request.active_only && value_set.inactive != Some(true) {
            codes.retain(|c| !c.inactive);
        }
        if let Some(text) = &request.filter {
            let text = text.to_lowercase();
            codes.retain(|c| {
                c.code.to_lowercase().contains(&text)
                    || c.display
                        .as_deref()
                        .is_some_and(|d| d.to_lowercase().contains(&text))
            });
        }
        let total = codes.len();
        let contains = codes
            .into_iter()
            .skip(request.offset)
            .take(request.count.unwrap_or(usize::MAX))
            .collect();
        Ok(Expansion {
            value_set: label(&value_set.url, value_set.version.as_deref()),
            total,
            pinned: value_set.expansion.is_some(),
            contains,
        })
    }

    /// `$validate-code` against value set `reference`: whether `code` from `system` is in it,
    /// and whether `display`, if given, is its display or one of its designations.
    pub fn validate_code(
        &self,
        reference: &str,
        system: &str,
        code: &str,
        display: Option<&str>,
    ) -> Result<Validation, Error> {
        let codes = self.compute(reference, &mut Vec::new())?;
        let held = resolve(&self.code_systems, system, None);
        let same = |a: &str, b: &str| match held {
            Some(cs) if !cs.case_sensitive => a.eq_ignore_ascii_case(b),
            _ => a == b,
        };
        let Some(found) = codes
            .iter()
            .find(|c| c.system == system && same(&c.code, code))
        else {
            return Ok(Validation::Invalid);
        };
        let designations: Vec<String> = held
            .and_then(|cs| Index::new(cs).get(code).cloned())
            .map(|concept| concept.designations.into_iter().map(|d| d.value).collect())
            .unwrap_or_default();
        Ok(check_display(found.display.clone(), &designations, display))
    }

    /// `$validate-code` against code system `system`: whether `code` is one of its codes.
    pub fn validate_in_code_system(
        &self,
        system: &str,
        version: Option<&str>,
        code: &str,
        display: Option<&str>,
    ) -> Result<Validation, Error> {
        let code_system = self.code_system(system, version)?;
        let index = Index::new(code_system);
        let Some(concept) = index.get(code) else {
            return if code_system.content == Content::Complete {
                Ok(Validation::Invalid)
            } else {
                Err(Error::Incomplete(system.to_owned()))
            };
        };
        let designations: Vec<String> = concept
            .designations
            .iter()
            .map(|d| d.value.clone())
            .collect();
        Ok(check_display(
            concept.display.clone(),
            &designations,
            display,
        ))
    }

    /// `$translate`: the targets of `code` from `system`, in concept map `reference`, or in
    /// every held concept map when `reference` is `None`.
    pub fn translate(
        &self,
        reference: Option<&str>,
        system: &str,
        code: &str,
        target_system: Option<&str>,
    ) -> Result<Vec<Translation>, Error> {
        let maps: Vec<&ConceptMap> = match reference {
            Some(reference) => {
                let canonical = Canonical::parse(reference);
                vec![
                    resolve(
                        &self.concept_maps,
                        &canonical.url,
                        canonical.version.as_deref(),
                    )
                    .ok_or_else(|| Error::UnknownConceptMap(reference.to_owned()))?,
                ]
            }
            None => self
                .concept_maps
                .keys()
                .filter_map(|url| resolve(&self.concept_maps, url, None))
                .collect(),
        };
        let mut found = Vec::new();
        for map in maps {
            for group in &map.groups {
                if group.source != system
                    || target_system.is_some_and(|target| target != group.target)
                {
                    continue;
                }
                for mapping in group.mappings.iter().filter(|m| m.code == code) {
                    for target in &mapping.targets {
                        found.push(Translation {
                            concept_map: label(&map.url, map.version.as_deref()),
                            system: group.target.clone(),
                            target: target.clone(),
                        });
                    }
                }
            }
        }
        Ok(found)
    }
}

fn check_display(
    expected: Option<String>,
    designations: &[String],
    given: Option<&str>,
) -> Validation {
    match given {
        Some(given)
            if expected.as_deref() != Some(given) && !designations.iter().any(|d| d == given) =>
        {
            Validation::WrongDisplay { expected }
        }
        _ => Validation::Valid { display: expected },
    }
}

#[cfg(test)]
mod tests;

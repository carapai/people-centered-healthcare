//! What the Studio's terminology browser asks of a node's terminology (studio.md § Terminology;
//! #523): the code systems held, a code system's hierarchy one level at a time, search by code or
//! words, and how a ValueSet's expansion changed against its published one.
//!
//! Search and the hierarchy run here, over the code system held, so a browser never loads a large
//! external code system such as ICD-11: it asks for one page of concepts at a time.

use std::collections::BTreeMap;

use crate::{CodeSystem, Concept, Contains, Content, Error, Index, Terminology, label, resolve};

/// A code system held, as the browser lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeSystemSummary {
    /// Its canonical URL.
    pub url: String,
    /// Its version.
    pub version: Option<String>,
    /// How much of it is held.
    pub content: Content,
    /// How many concepts it holds, at every level.
    pub concepts: usize,
}

/// One concept as the browser lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConceptRow {
    /// Its code.
    pub code: String,
    /// Its display.
    pub display: Option<String>,
    /// Whether it is inactive.
    pub inactive: bool,
    /// How many concepts are directly under it.
    pub children: usize,
}

/// A page of concepts: the rows asked for, and how many there are before paging.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConceptPage {
    /// The code system, `url|version`.
    pub code_system: String,
    /// Concepts before paging.
    pub total: usize,
    /// The concepts on this page.
    pub concepts: Vec<ConceptRow>,
}

/// How a ValueSet's expansion changed against its published one, each list in system and code
/// order.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExpansionChange {
    /// Codes it now has that the published expansion did not.
    pub added: Vec<Contains>,
    /// Codes the published expansion had that it no longer has. Records holding them stay valid
    /// against the version that issued them; new forms no longer offer them.
    pub removed: Vec<Contains>,
}

/// The codes in `draft` and not in `published` (added), and the reverse (removed), compared by
/// system and code.
pub fn compare(published: &[Contains], draft: &[Contains]) -> ExpansionChange {
    let key = |c: &Contains| (c.system.clone(), c.code.clone());
    let before: BTreeMap<_, _> = published.iter().map(|c| (key(c), c)).collect();
    let after: BTreeMap<_, _> = draft.iter().map(|c| (key(c), c)).collect();
    ExpansionChange {
        added: after
            .iter()
            .filter(|(k, _)| !before.contains_key(*k))
            .map(|(_, c)| (*c).clone())
            .collect(),
        removed: before
            .iter()
            .filter(|(k, _)| !after.contains_key(*k))
            .map(|(_, c)| (*c).clone())
            .collect(),
    }
}

fn count(concepts: &[Concept]) -> usize {
    concepts.iter().map(|c| 1 + count(&c.children)).sum()
}

impl Index<'_> {
    fn row(&self, concept: &Concept) -> ConceptRow {
        ConceptRow {
            code: concept.code.clone(),
            display: concept.display.clone(),
            inactive: concept.is_inactive(),
            children: self
                .children
                .get(&self.key(&concept.code))
                .map_or(0, |c| c.len()),
        }
    }
}

fn page(
    system: &CodeSystem,
    rows: Vec<ConceptRow>,
    offset: usize,
    count: Option<usize>,
) -> ConceptPage {
    ConceptPage {
        code_system: label(&system.url, system.version.as_deref()),
        total: rows.len(),
        concepts: rows
            .into_iter()
            .skip(offset)
            .take(count.unwrap_or(usize::MAX))
            .collect(),
    }
}

impl Terminology {
    /// Every code system held, each version once, by URL then version.
    pub fn code_systems(&self) -> Vec<CodeSystemSummary> {
        self.code_systems
            .values()
            .flat_map(BTreeMap::values)
            .map(|system| CodeSystemSummary {
                url: system.url.clone(),
                version: system.version.clone(),
                content: system.content,
                concepts: count(&system.concepts),
            })
            .collect()
    }

    /// The canonical (`url|version`) of every ValueSet held, each version once.
    pub fn value_set_references(&self) -> Vec<String> {
        self.value_sets
            .values()
            .flat_map(BTreeMap::values)
            .map(|v| label(&v.url, v.version.as_deref()))
            .collect()
    }

    /// Holds everything `other` holds too, replacing the same versions.
    pub fn extend(&mut self, other: &Terminology) {
        for system in other.code_systems.values().flat_map(BTreeMap::values) {
            self.add_code_system(system.clone());
        }
        for value_set in other.value_sets.values().flat_map(BTreeMap::values) {
            self.add_value_set(value_set.clone());
        }
        for map in other.concept_maps.values().flat_map(BTreeMap::values) {
            self.add_concept_map(map.clone());
        }
    }

    fn held(&self, system: &str, version: Option<&str>) -> Result<&CodeSystem, Error> {
        resolve(&self.code_systems, system, version)
            .ok_or_else(|| Error::UnknownCodeSystem(label(system, version)))
    }

    /// The concepts directly under `parent` in `system` (at `version`, or the latest held), or
    /// its top-level concepts when `parent` is `None`, by code and paged.
    pub fn children(
        &self,
        system: &str,
        version: Option<&str>,
        parent: Option<&str>,
        offset: usize,
        count: Option<usize>,
    ) -> Result<ConceptPage, Error> {
        let code_system = self.held(system, version)?;
        let index = Index::new(code_system);
        let rows: Vec<ConceptRow> = match parent {
            Some(parent) => {
                let found = index.get(parent).ok_or_else(|| Error::UnknownCode {
                    system: system.to_owned(),
                    code: parent.to_owned(),
                })?;
                index
                    .children
                    .get(&index.key(&found.code))
                    .into_iter()
                    .flatten()
                    .filter_map(|key| index.concepts.get(key))
                    .map(|concept| index.row(concept))
                    .collect()
            }
            None => index
                .concepts
                .iter()
                .filter(|(key, _)| !index.parents.contains_key(*key))
                .map(|(_, concept)| index.row(concept))
                .collect(),
        };
        Ok(page(code_system, rows, offset, count))
    }

    /// The concepts of `system` (at `version`, or the latest held) whose code, display or a
    /// designation holds every word of `text`, ignoring case: a code equal to the text first,
    /// then codes starting with it, then the rest, each by code; paged.
    pub fn search(
        &self,
        system: &str,
        version: Option<&str>,
        text: &str,
        offset: usize,
        count: Option<usize>,
    ) -> Result<ConceptPage, Error> {
        let code_system = self.held(system, version)?;
        let index = Index::new(code_system);
        let wanted = text.trim().to_lowercase();
        let words: Vec<&str> = wanted.split_whitespace().collect();
        let mut found: Vec<(u8, &Concept)> = index
            .concepts
            .values()
            .filter(|concept| {
                let mut haystack = concept.code.to_lowercase();
                for text in concept
                    .display
                    .iter()
                    .chain(concept.designations.iter().map(|d| &d.value))
                {
                    haystack.push('\n');
                    haystack.push_str(&text.to_lowercase());
                }
                words.iter().all(|word| haystack.contains(word))
            })
            .map(|concept| {
                let code = concept.code.to_lowercase();
                let rank = if code == wanted {
                    0
                } else if code.starts_with(&wanted) {
                    1
                } else {
                    2
                };
                (rank, *concept)
            })
            .collect();
        found.sort_by(|(a, x), (b, y)| a.cmp(b).then_with(|| x.code.cmp(&y.code)));
        let rows = found
            .into_iter()
            .map(|(_, concept)| index.row(concept))
            .collect();
        Ok(page(code_system, rows, offset, count))
    }
}

//! Language Packs (ADR 0066; configuration.md § Package format): one language's wording for
//! another package's forms and code lists, published and activated on its own.
//!
//! - **One pack per package and language.** A pack of kind [`PackageKind::Language`] names what
//!   it translates in `package.json`'s `translates` ([`Translates`]): the target package, the one
//!   major version it targets (`2.x`) and the language. It is named `<target>.<language>`, e.g.
//!   `ug.moh.anc.lg` for `ug.moh.anc`.
//! - **Entries are keyed by a stable address** ([`Key`]), and each records the English it
//!   translates ([`Entry`]). They live in [`TRANSLATIONS`].
//! - **Staleness** ([`report`]): against the target's English, an entry whose English still
//!   matches is current, one whose English changed is stale (it is not served, and the Studio
//!   lists it for re-translation), and one whose target is gone is flagged and ignored.
//! - **Lifecycle:** one approval by someone who never held the draft
//!   ([`crate::lifecycle::approvals_required`]), and semver of its own: a new or changed
//!   translation is at least a patch, a new or dropped one a minor, and a new target major
//!   version a major ([`crate::lifecycle::semver`]).
//!
//! English is the base and the fallback, string by string, so a pack need not be complete.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::package::{PackageKind, valid_name};

/// Where a pack's entries live, under the package root.
pub const TRANSLATIONS: &str = "configuration/translations.json";

/// What a Language Pack translates: `package.json`'s `translates`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Translates {
    /// The target package's name, e.g. `ug.moh.anc`.
    pub package: String,
    /// The one major version it targets, as `MAJOR.x`, e.g. `2.x`.
    pub version: String,
    /// The language, a BCP 47 tag, e.g. `lg`.
    pub language: String,
}

impl Translates {
    /// The major version targeted, if [`Translates::version`] reads `MAJOR.x`.
    pub fn major(&self) -> Option<u64> {
        let major = self.version.strip_suffix(".x")?;
        let leading_zero = major.len() > 1 && major.starts_with('0');
        if major.is_empty() || leading_zero || !major.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        major.parse().ok()
    }

    /// The name the pack must have: `<target>.<language>`, lower-cased.
    pub fn pack_name(&self) -> String {
        format!("{}.{}", self.package, self.language.to_ascii_lowercase())
    }
}

/// A BCP 47 tag of the shape packs use: a 2–3 letter lower-case language, then optional
/// subtags of 2–8 letters or digits, e.g. `lg`, `nyn`, `sw-KE`.
pub fn valid_language(tag: &str) -> bool {
    let mut parts = tag.split('-');
    let language = parts.next().unwrap_or_default();
    (2..=3).contains(&language.len())
        && language.bytes().all(|b| b.is_ascii_lowercase())
        && parts.all(|part| {
            (2..=8).contains(&part.len()) && part.bytes().all(|b| b.is_ascii_alphanumeric())
        })
}

/// Why a pack's `package.json` or entries may not be published.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PackProblem {
    /// A Language Pack with no `translates`.
    #[error("a language pack must say what it translates")]
    NoTarget,
    /// `translates` on a package that is not a Language Pack.
    #[error("only a language pack translates another package")]
    NotAPack,
    /// The target is not a package name, or is the pack itself.
    #[error("{0:?} is not a package a pack can translate")]
    Target(String),
    /// The targeted version is not `MAJOR.x`.
    #[error("a pack targets one major version, as MAJOR.x, not {0:?}")]
    TargetVersion(String),
    /// Not a language tag.
    #[error("{0:?} is not a language tag")]
    Language(String),
    /// The pack's name is not `<target>.<language>`.
    #[error("the pack must be named {expected}, not {actual}")]
    Name {
        /// `<target>.<language>`.
        expected: String,
        /// Its name.
        actual: String,
    },
    /// A pack with dependencies: it targets a major version, never an exact one.
    #[error("a language pack has no dependencies: it targets {0}")]
    Dependencies(String),
    /// Two entries with one address.
    #[error("{0} is translated twice")]
    Duplicate(Key),
    /// An entry whose English or translation is blank.
    #[error("{0} has no English or no translation")]
    Blank(Key),
    /// The entries file names another language than `package.json`.
    #[error("the entries are in {entries}, the pack in {pack}")]
    OtherLanguage {
        /// `translates.language`.
        pack: String,
        /// The entries file's language.
        entries: String,
    },
}

/// The problems with a package's `translates`, given its name, kind, dependencies and target.
pub fn target_problems(
    name: &str,
    kind: PackageKind,
    has_dependencies: bool,
    translates: Option<&Translates>,
) -> Vec<PackProblem> {
    let mut problems = Vec::new();
    let translates = match (kind, translates) {
        (PackageKind::Language, None) => return vec![PackProblem::NoTarget],
        (PackageKind::Language, Some(translates)) => translates,
        (_, Some(_)) => return vec![PackProblem::NotAPack],
        (_, None) => return problems,
    };
    if !valid_name(&translates.package) || translates.package == name {
        problems.push(PackProblem::Target(translates.package.clone()));
    }
    if translates.major().is_none() {
        problems.push(PackProblem::TargetVersion(translates.version.clone()));
    }
    if !valid_language(&translates.language) {
        problems.push(PackProblem::Language(translates.language.clone()));
    }
    let expected = translates.pack_name();
    if name != expected {
        problems.push(PackProblem::Name {
            expected,
            actual: name.to_owned(),
        });
    }
    if has_dependencies {
        problems.push(PackProblem::Dependencies(format!(
            "{}@{}",
            translates.package, translates.version
        )));
    }
    problems
}

/// Which wording of a Questionnaire item an entry translates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Part {
    /// `item.text`.
    Text,
    /// `item.prefix`.
    Prefix,
    /// The text of its help: a child `display` item with the `help` item control.
    Help,
}

impl fmt::Display for Part {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Text => "text",
            Self::Prefix => "prefix",
            Self::Help => "help",
        })
    }
}

/// The stable address of one translatable string (ADR 0066).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum Key {
    /// A Questionnaire item's text, prefix or help: `url` + `linkId`.
    #[serde(rename_all = "camelCase")]
    Item {
        /// The Questionnaire's canonical URL.
        questionnaire: String,
        /// The item's `linkId`.
        link_id: String,
        /// Which wording.
        part: Part,
    },
    /// An answer option's display: `url` + `linkId` + code.
    #[serde(rename_all = "camelCase")]
    Answer {
        /// The Questionnaire's canonical URL.
        questionnaire: String,
        /// The item's `linkId`.
        link_id: String,
        /// The option's code.
        code: String,
    },
    /// A Form Layout section's title: layout `url` + section id.
    Section {
        /// The Form Layout's `url`.
        layout: String,
        /// The section's `id`.
        section: String,
    },
    /// A code-list display: system + code.
    Concept {
        /// The CodeSystem's canonical URL.
        system: String,
        /// The code.
        code: String,
    },
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Item {
                questionnaire,
                link_id,
                part,
            } => write!(f, "{questionnaire} item {link_id} {part}"),
            Self::Answer {
                questionnaire,
                link_id,
                code,
            } => write!(f, "{questionnaire} item {link_id} answer {code}"),
            Self::Section { layout, section } => write!(f, "{layout} section {section}"),
            Self::Concept { system, code } => write!(f, "{system} code {code}"),
        }
    }
}

/// One translated string.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    /// What it translates.
    pub key: Key,
    /// The English it was translated from.
    pub english: String,
    /// The translation.
    pub text: String,
}

/// The problems with a pack's entries: duplicates and blanks.
pub fn entry_problems(entries: &[Entry]) -> Vec<PackProblem> {
    let mut problems = Vec::new();
    let mut seen = BTreeSet::new();
    for entry in entries {
        if entry.english.trim().is_empty() || entry.text.trim().is_empty() {
            problems.push(PackProblem::Blank(entry.key.clone()));
        }
        if !seen.insert(&entry.key) {
            problems.push(PackProblem::Duplicate(entry.key.clone()));
        }
    }
    problems
}

/// Where an entry stands against its target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "status")]
pub enum Status {
    /// Its English still matches: it is served.
    Current,
    /// Its English no longer matches: English is served instead, and it needs re-translating.
    Stale {
        /// The target's English now.
        now: String,
    },
    /// Its target no longer exists: it is ignored.
    Gone,
}

/// One entry and where it stands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Checked {
    /// The entry.
    pub entry: Entry,
    /// Where it stands.
    #[serde(flatten)]
    pub status: Status,
}

/// A pack's entries checked against its target's English.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Report {
    /// Every entry, in key order.
    pub entries: Vec<Checked>,
    /// How many of the target's strings have a current translation.
    pub translated: usize,
    /// How many translatable strings the target has.
    pub strings: usize,
}

impl Report {
    /// The stale entries: listed for re-translation.
    pub fn stale(&self) -> impl Iterator<Item = &Checked> {
        self.entries
            .iter()
            .filter(|checked| matches!(checked.status, Status::Stale { .. }))
    }

    /// The entries whose target is gone: flagged.
    pub fn gone(&self) -> impl Iterator<Item = &Checked> {
        self.entries
            .iter()
            .filter(|checked| checked.status == Status::Gone)
    }

    /// The translations served, by key: the current entries only. Every other string falls
    /// back to English.
    pub fn served(&self) -> BTreeMap<&Key, &str> {
        self.entries
            .iter()
            .filter(|checked| checked.status == Status::Current)
            .map(|checked| (&checked.entry.key, checked.entry.text.as_str()))
            .collect()
    }
}

/// Checks `entries` against `english`, the target's translatable strings by key.
pub fn report(entries: &[Entry], english: &BTreeMap<Key, String>) -> Report {
    let mut checked: Vec<Checked> = entries
        .iter()
        .map(|entry| {
            let status = match english.get(&entry.key) {
                None => Status::Gone,
                Some(now) if *now == entry.english => Status::Current,
                Some(now) => Status::Stale { now: now.clone() },
            };
            Checked {
                entry: entry.clone(),
                status,
            }
        })
        .collect();
    checked.sort_by(|a, b| a.entry.key.cmp(&b.entry.key));
    let translated = checked
        .iter()
        .filter(|checked| checked.status == Status::Current)
        .map(|checked| &checked.entry.key)
        .collect::<BTreeSet<_>>()
        .len();
    Report {
        entries: checked,
        translated,
        strings: english.len(),
    }
}

/// A pack's translations, as the semver check compares them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Translations {
    /// The target's major version.
    pub major: u64,
    /// Each translation, by key.
    pub entries: BTreeMap<Key, String>,
}

impl Translations {
    /// The translations of `entries`, for the target's `major` version. Of two entries with one
    /// key, the last counts (a duplicate is a publication problem anyway).
    pub fn of(major: u64, entries: &[Entry]) -> Self {
        Self {
            major,
            entries: entries
                .iter()
                .map(|entry| (entry.key.clone(), entry.text.clone()))
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests;

//! The terminology resources, as the operations need them: CodeSystems, ValueSets and
//! ConceptMaps. Adapters read them from packages; this crate reads no JSON.

use std::fmt;

/// A canonical reference, `url` with an optional `|version`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Canonical {
    /// The canonical URL.
    pub url: String,
    /// The version, if pinned.
    pub version: Option<String>,
}

impl Canonical {
    /// Reads `url|version`, or a bare `url`.
    pub fn parse(text: &str) -> Self {
        match text.split_once('|') {
            Some((url, version)) if !version.is_empty() => Self {
                url: url.to_owned(),
                version: Some(version.to_owned()),
            },
            Some((url, _)) => Self {
                url: url.to_owned(),
                version: None,
            },
            None => Self {
                url: text.to_owned(),
                version: None,
            },
        }
    }
}

impl fmt::Display for Canonical {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.version {
            Some(version) => write!(f, "{}|{version}", self.url),
            None => f.write_str(&self.url),
        }
    }
}

/// A property's value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PropertyValue {
    /// A code, e.g. `status` = `retired`, or a parent's code.
    Code(String),
    /// A string.
    String(String),
    /// A boolean, e.g. `inactive` = `true`.
    Boolean(bool),
    /// Any other value, as its text (an integer, a decimal, a date-time or a coding's code).
    Other(String),
}

impl PropertyValue {
    /// The value as text, for `=` and `in` filters.
    pub fn text(&self) -> String {
        match self {
            Self::Code(text) | Self::String(text) | Self::Other(text) => text.clone(),
            Self::Boolean(value) => value.to_string(),
        }
    }
}

/// A concept's property.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Property {
    /// The property's code, e.g. `inactive`, `status` or `parent`.
    pub code: String,
    /// Its value.
    pub value: PropertyValue,
}

/// Another name for a concept: a translation or a synonym.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Designation {
    /// Its language, e.g. `lg`.
    pub language: Option<String>,
    /// What kind of designation, as a code, e.g. `synonym`.
    pub use_code: Option<String>,
    /// The text.
    pub value: String,
}

/// A concept, with the concepts under it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Concept {
    /// Its code.
    pub code: String,
    /// Its display.
    pub display: Option<String>,
    /// Its definition.
    pub definition: Option<String>,
    /// Its designations.
    pub designations: Vec<Designation>,
    /// Its properties.
    pub properties: Vec<Property>,
    /// The concepts it subsumes, nested.
    pub children: Vec<Concept>,
}

impl Concept {
    /// Whether the concept is inactive: its `inactive` property is true, or its `status` is
    /// `retired`. A `deprecated` concept is still active.
    pub fn is_inactive(&self) -> bool {
        self.properties
            .iter()
            .any(|property| match (property.code.as_str(), &property.value) {
                ("inactive", PropertyValue::Boolean(inactive)) => *inactive,
                ("status", value) => value.text() == "retired",
                _ => false,
            })
    }
}

/// How much of a code system a CodeSystem resource holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Content {
    /// Every concept.
    Complete,
    /// Some concepts: a code not in it may still exist.
    Fragment,
    /// None: the codes are elsewhere.
    NotPresent,
    /// An example subset.
    Example,
    /// A supplement to another code system.
    Supplement,
}

/// A code system.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeSystem {
    /// Its canonical URL: the `system` of its codings.
    pub url: String,
    /// Its version.
    pub version: Option<String>,
    /// Whether codes compare case-sensitively.
    pub case_sensitive: bool,
    /// How much of it is here.
    pub content: Content,
    /// Its top-level concepts.
    pub concepts: Vec<Concept>,
}

/// A filter operator (FHIR `filter-operator`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterOp {
    /// The property equals the value.
    Equals,
    /// The concept is the value or under it.
    IsA,
    /// The concept is under the value, not the value itself.
    DescendentOf,
    /// The concept is neither the value nor under it.
    IsNotA,
    /// The concept is the value or above it.
    Generalizes,
    /// The concept is directly under the value.
    ChildOf,
    /// The property's value is one of a comma-separated list.
    In,
    /// The property's value is none of a comma-separated list.
    NotIn,
    /// The property exists (value `true`) or does not (`false`).
    Exists,
    /// A regular expression: refused, since a domain crate has no regex engine to run it with
    /// the guarantees a terminology server needs.
    Regex,
}

/// One filter of an include or exclude rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Filter {
    /// The property, or `concept` for the hierarchy.
    pub property: String,
    /// The operator.
    pub op: FilterOp,
    /// The value.
    pub value: String,
}

/// A code listed in a rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listed {
    /// The code.
    pub code: String,
    /// The display the value set gives it, if any.
    pub display: Option<String>,
}

/// An include or exclude rule of a ValueSet's compose.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Rule {
    /// The code system, if the rule selects codes.
    pub system: Option<String>,
    /// Its version, if pinned.
    pub version: Option<String>,
    /// Codes listed by hand.
    pub concepts: Vec<Listed>,
    /// Filters, all of which a code must pass.
    pub filters: Vec<Filter>,
    /// Value sets whose codes, intersected, the rule selects.
    pub value_sets: Vec<String>,
}

/// One code of an expansion.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Contains {
    /// The code system.
    pub system: String,
    /// Its version.
    pub version: Option<String>,
    /// The code.
    pub code: String,
    /// Its display.
    pub display: Option<String>,
    /// Whether the code is inactive.
    pub inactive: bool,
}

/// A value set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueSet {
    /// Its canonical URL.
    pub url: String,
    /// Its version.
    pub version: Option<String>,
    /// The rules that define it.
    pub include: Vec<Rule>,
    /// The rules that take codes out.
    pub exclude: Vec<Rule>,
    /// Whether inactive codes are included: `None` leaves it to the expansion request.
    pub inactive: Option<bool>,
    /// The pinned expansion its package ships (terminology.md § Scope), if any. Expansion and
    /// validation use it rather than computing one.
    pub expansion: Option<Vec<Contains>>,
}

/// How a source concept relates to a target (FHIR `concept-map-relationship`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Relationship {
    /// The same meaning.
    Equivalent,
    /// Related, with no closer statement.
    RelatedTo,
    /// The source is narrower than the target.
    SourceIsNarrowerThanTarget,
    /// The source is broader than the target.
    SourceIsBroaderThanTarget,
    /// No target means the source.
    NotRelatedTo,
}

/// A target of a mapped concept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    /// The target code; `None` with [`Relationship::NotRelatedTo`].
    pub code: Option<String>,
    /// Its display.
    pub display: Option<String>,
    /// How the source relates to it.
    pub relationship: Relationship,
}

/// A source concept and its targets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mapping {
    /// The source code.
    pub code: String,
    /// Its targets.
    pub targets: Vec<Target>,
}

/// A group of mappings from one code system to another.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    /// The source code system.
    pub source: String,
    /// The target code system.
    pub target: String,
    /// The mappings.
    pub mappings: Vec<Mapping>,
}

/// A concept map.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConceptMap {
    /// Its canonical URL.
    pub url: String,
    /// Its version.
    pub version: Option<String>,
    /// The groups.
    pub groups: Vec<Group>,
}

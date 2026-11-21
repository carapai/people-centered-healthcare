//! A pasted Questionnaire read as R5, or as R4 converted to R5, with its conversion report
//! (studio.md § The form designer; metadata.md § Importing R4 content; #497, #853).
//!
//! - R4's `choice` and `open-choice` item types become `coding`, with `answerConstraint`
//!   `optionsOnly` or `optionsOrString`. An `answerConstraint` already there is left as it says.
//! - An element neither version's Questionnaire or item has, `contained` resources, a
//!   `modifierExtension` and an item type R5 does not know cannot be converted: they are
//!   reported, and [`blocks`] says the import must not go on.
//! - Read as R5, a Questionnaire holding R4's choice types is refused: it was meant to be read as
//!   R4.
//!
//! Everything else is carried as it was.

use serde_json::{Map, Value};

/// The FHIR version a pasted Questionnaire is read as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Version {
    /// FHIR R4 (4.0.1), converted to R5.
    R4,
    /// FHIR R5 (5.0.0), read as it is.
    R5,
}

/// Why a paste is not read at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// The text is not JSON.
    NotJson,
    /// The JSON is not a Questionnaire.
    NotQuestionnaire,
    /// Read as R5, it holds R4's choice item types.
    LooksR4,
}

impl Refusal {
    /// Its code on the wire.
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::NotJson => "not-json",
            Self::NotQuestionnaire => "not-questionnaire",
            Self::LooksR4 => "looks-r4",
        }
    }
}

/// One line of the conversion report. `at` is `Questionnaire` or the item's `linkId` (`?` when
/// it has none).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Finding {
    /// An R4 `choice` item became `coding`, answers only from its options.
    ChoiceToCoding {
        /// Where.
        at: String,
    },
    /// An R4 `open-choice` item became `coding`, its options or free text.
    OpenChoiceToCoding {
        /// Where.
        at: String,
    },
    /// An element neither version knows, or one the import cannot carry.
    UnknownElement {
        /// Where.
        at: String,
        /// Its name as it was written.
        element: String,
    },
    /// Contained resources, which the import cannot carry.
    Contained {
        /// Where.
        at: String,
    },
    /// A modifier extension, whose meaning the import cannot know.
    ModifierExtension {
        /// Where.
        at: String,
    },
    /// An item type R5 does not have.
    UnknownType {
        /// Where.
        at: String,
        /// The type as it was written.
        item_type: String,
    },
}

impl Finding {
    /// Its code on the wire.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::ChoiceToCoding { .. } => "choice-to-coding",
            Self::OpenChoiceToCoding { .. } => "open-choice-to-coding",
            Self::UnknownElement { .. } => "unknown-element",
            Self::Contained { .. } => "contained",
            Self::ModifierExtension { .. } => "modifier-extension",
            Self::UnknownType { .. } => "unknown-type",
        }
    }

    /// Whether it was converted; one that was not blocks the import.
    #[must_use]
    pub fn converted(&self) -> bool {
        matches!(
            self,
            Self::ChoiceToCoding { .. } | Self::OpenChoiceToCoding { .. }
        )
    }

    /// Where it is: `Questionnaire`, or an item's `linkId`.
    #[must_use]
    pub fn at(&self) -> &str {
        match self {
            Self::ChoiceToCoding { at }
            | Self::OpenChoiceToCoding { at }
            | Self::UnknownElement { at, .. }
            | Self::Contained { at }
            | Self::ModifierExtension { at }
            | Self::UnknownType { at, .. } => at,
        }
    }
}

/// A Questionnaire read, as R5, with its conversion report.
#[derive(Debug, Clone, PartialEq)]
pub struct Conversion {
    /// The R5 Questionnaire.
    pub questionnaire: Value,
    /// What was converted, and what could not be.
    pub report: Vec<Finding>,
}

/// Whether `report` blocks the import: something could not be converted.
#[must_use]
pub fn blocks(report: &[Finding]) -> bool {
    report.iter().any(|finding| !finding.converted())
}

// The elements of an R5 Questionnaire and of its items. R4's are a subset of these, so an element
// outside the list is one neither version knows, or one this import cannot carry.
const QUESTIONNAIRE_ELEMENTS: &[&str] = &[
    "resourceType",
    "id",
    "meta",
    "implicitRules",
    "language",
    "text",
    "contained",
    "extension",
    "modifierExtension",
    "url",
    "identifier",
    "version",
    "versionAlgorithmString",
    "versionAlgorithmCoding",
    "name",
    "title",
    "derivedFrom",
    "status",
    "experimental",
    "subjectType",
    "date",
    "publisher",
    "contact",
    "description",
    "useContext",
    "jurisdiction",
    "purpose",
    "copyright",
    "copyrightLabel",
    "approvalDate",
    "lastReviewDate",
    "effectivePeriod",
    "code",
    "item",
];
const ITEM_ELEMENTS: &[&str] = &[
    "id",
    "extension",
    "modifierExtension",
    "linkId",
    "definition",
    "code",
    "prefix",
    "text",
    "type",
    "enableWhen",
    "enableBehavior",
    "disabledDisplay",
    "required",
    "repeats",
    "readOnly",
    "maxLength",
    "answerConstraint",
    "answerValueSet",
    "answerOption",
    "initial",
    "item",
];
const R5_TYPES: &[&str] = &[
    "group",
    "display",
    "boolean",
    "decimal",
    "integer",
    "date",
    "dateTime",
    "time",
    "string",
    "text",
    "url",
    "coding",
    "attachment",
    "reference",
    "quantity",
];
const R4_CHOICE: &str = "choice";
const R4_OPEN_CHOICE: &str = "open-choice";

/// An element name with its primitive extension's `_` removed.
fn bare(key: &str) -> &str {
    key.strip_prefix('_').unwrap_or(key)
}

/// The objects in a JSON array, or none.
fn objects(items: Option<&Value>) -> impl Iterator<Item = &Map<String, Value>> {
    items
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_object)
}

fn type_of(item: &Map<String, Value>) -> String {
    match item.get("type") {
        Some(Value::String(t)) => t.clone(),
        Some(other) => other.to_string(),
        None => "undefined".to_owned(),
    }
}

fn holds_r4_choices(items: Option<&Value>) -> bool {
    objects(items).any(|item| {
        let t = type_of(item);
        t == R4_CHOICE || t == R4_OPEN_CHOICE || holds_r4_choices(item.get("item"))
    })
}

fn non_empty_array(value: Option<&Value>) -> bool {
    value
        .and_then(Value::as_array)
        .is_some_and(|a| !a.is_empty())
}

/// Reads `text` as a Questionnaire in `version`, converting R4 to R5, with its report.
///
/// # Errors
///
/// A [`Refusal`] when the text is not JSON, not a Questionnaire, or, read as R5, holds R4's
/// choice types.
pub fn read(text: &str, version: Version) -> Result<Conversion, Refusal> {
    let value: Value = serde_json::from_str(text).map_err(|_| Refusal::NotJson)?;
    let Value::Object(source) = value else {
        return Err(Refusal::NotQuestionnaire);
    };
    if source.get("resourceType").and_then(Value::as_str) != Some("Questionnaire") {
        return Err(Refusal::NotQuestionnaire);
    }
    if version == Version::R5 && holds_r4_choices(source.get("item")) {
        return Err(Refusal::LooksR4);
    }

    let mut report = Vec::new();
    let at = "Questionnaire";
    for key in source.keys() {
        if !QUESTIONNAIRE_ELEMENTS.contains(&bare(key)) {
            report.push(Finding::UnknownElement {
                at: at.into(),
                element: key.clone(),
            });
        }
    }
    if non_empty_array(source.get("contained")) {
        report.push(Finding::Contained { at: at.into() });
    }
    if non_empty_array(source.get("modifierExtension")) {
        report.push(Finding::ModifierExtension { at: at.into() });
    }

    let items = convert(source.get("item"), version, &mut report);
    let mut questionnaire = source;
    if items.is_empty() {
        questionnaire.remove("item");
    } else {
        questionnaire.insert("item".into(), Value::Array(items));
    }
    Ok(Conversion {
        questionnaire: Value::Object(questionnaire),
        report,
    })
}

fn convert(items: Option<&Value>, version: Version, report: &mut Vec<Finding>) -> Vec<Value> {
    objects(items)
        .map(|item| {
            let at = match item.get("linkId") {
                Some(Value::String(id)) if !id.is_empty() => id.clone(),
                _ => "?".to_owned(),
            };
            for key in item.keys() {
                if !ITEM_ELEMENTS.contains(&bare(key)) {
                    report.push(Finding::UnknownElement {
                        at: at.clone(),
                        element: key.clone(),
                    });
                }
            }
            if non_empty_array(item.get("modifierExtension")) {
                report.push(Finding::ModifierExtension { at: at.clone() });
            }
            let mut out = item.clone();
            let item_type = type_of(item);
            let constraint = match (version, item_type.as_str()) {
                (Version::R4, R4_CHOICE) => {
                    Some(("optionsOnly", Finding::ChoiceToCoding { at: at.clone() }))
                }
                (Version::R4, R4_OPEN_CHOICE) => Some((
                    "optionsOrString",
                    Finding::OpenChoiceToCoding { at: at.clone() },
                )),
                _ => None,
            };
            if let Some((constraint, finding)) = constraint {
                out.insert("type".into(), Value::from("coding"));
                out.entry("answerConstraint")
                    .or_insert_with(|| Value::from(constraint));
                report.push(finding);
            } else if !R5_TYPES.contains(&item_type.as_str()) {
                report.push(Finding::UnknownType { at, item_type });
            }
            if item.contains_key("item") {
                out.insert(
                    "item".into(),
                    Value::Array(convert(item.get("item"), version, report)),
                );
            }
            Value::Object(out)
        })
        .collect()
}

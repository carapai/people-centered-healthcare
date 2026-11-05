//! Completing a QuestionnaireResponse (questionnaire-extraction.md; ADR 0022, 0023; #93): what a
//! completion writes besides the response. The node's write path calls these in the
//! response's own transaction, so completion is all or nothing.
//!
//! - **The mapping** ([`choose`]): the newest Extraction Mapping for the response's
//!   Questionnaire whose version range takes the Questionnaire's version. The response records
//!   it ([`MAPPING_EXTENSION`]), and later amendments reuse it.
//! - **Owners** ([`admit`]): each planned resource goes to the Owner use case its template names,
//!   which must accept that type ([`OPT_INS`]). Laboratory takes point-of-care results only.
//! - **Reconciling** ([`reconcile`]) the plan with what the last run wrote:
//!   - a **managed** target is written when new or changed, and a managed target the plan no
//!     longer has is retracted;
//!   - a **seed** target is created once; an amendment that would change it raises a review
//!     item instead of rewriting it.
//! - **Provenance** ([`provenance`]): one per response, versioned per run (its id is derived
//!   from the response), naming every version written or retracted, the response version and
//!   the mapping. The last run's targets are read back from it.

pub mod consultation;

use std::collections::BTreeMap;

use health_configuration::package::Version;
use health_fhir_extraction::{EXTRACTION_TAG_SYSTEM, Mapping, Mode, Planned};
use health_identifiers::{DerivationName, ResourceId, derive_id};
use serde_json::{Value, json};

/// The extension on a QuestionnaireResponse naming the mapping (`url|version`) it was extracted
/// with.
pub const MAPPING_EXTENSION: &str = "https://carapai.github.io/people-centered-healthcare/fhir/StructureDefinition/extraction-mapping";

/// The rule a run's Provenance id is derived by, from the response id.
const PROVENANCE_RULE: &str =
    "https://carapai.github.io/people-centered-healthcare/fhir/rules/extraction-provenance";

/// The platform's CodeSystem (ADR 0049).
const PLATFORM_CODES: &str =
    "https://carapai.github.io/people-centered-healthcare/fhir/CodeSystem/platform";

/// Why a completion was refused. Nothing of it is kept.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CompletionError {
    /// The template names an Owner use case that does not take what it produces.
    #[error("template {template}: {use_case} does not accept {reason}")]
    NotAccepted {
        /// The template.
        template: String,
        /// The use case it names.
        use_case: String,
        /// What it was given.
        reason: String,
    },
}

/// An Owner's opt-in: the use case, the type it accepts, and whether it takes a given resource.
pub struct OptIn {
    /// The Owner's use case, as templates name it.
    pub use_case: &'static str,
    /// The type it accepts.
    pub resource_type: &'static str,
    /// Whether it takes this resource.
    pub accepts: fn(&Value) -> bool,
}

fn any(_: &Value) -> bool {
    true
}

fn is_laboratory(observation: &Value) -> bool {
    observation["category"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|c| c["coding"].as_array().into_iter().flatten())
        .any(|c| c["code"] == "laboratory")
}

fn not_laboratory(observation: &Value) -> bool {
    !is_laboratory(observation)
}

/// What each Owner accepts from extraction (questionnaire-extraction.md § Ownership). Until the
/// Owners' own crates declare it, the list lives here.
pub const OPT_INS: &[OptIn] = &[
    OptIn {
        use_case: "clinical-data.record-observation",
        resource_type: "Observation",
        accepts: not_laboratory,
    },
    OptIn {
        use_case: "laboratory.record-point-of-care",
        resource_type: "Observation",
        accepts: is_laboratory,
    },
    OptIn {
        use_case: "clinical-data.record-condition",
        resource_type: "Condition",
        accepts: any,
    },
    OptIn {
        use_case: "medications.prescribe",
        resource_type: "MedicationRequest",
        accepts: any,
    },
    OptIn {
        use_case: "orders.request",
        resource_type: "ServiceRequest",
        accepts: any,
    },
    OptIn {
        use_case: "scheduling.book",
        resource_type: "Appointment",
        accepts: any,
    },
];

/// Whether `planned`'s Owner use case accepts it.
pub fn admit(planned: &Planned) -> Result<(), CompletionError> {
    let refused = |reason: String| CompletionError::NotAccepted {
        template: planned.template.clone(),
        use_case: planned.owner_use_case.clone(),
        reason,
    };
    let Some(opt_in) = OPT_INS
        .iter()
        .find(|o| o.use_case == planned.owner_use_case)
    else {
        return Err(refused("anything: no such use case".into()));
    };
    if opt_in.resource_type != planned.resource_type {
        return Err(refused(format!("a {}", planned.resource_type)));
    }
    if !(opt_in.accepts)(&planned.resource) {
        return Err(refused(format!("this {}", planned.resource_type)));
    }
    Ok(())
}

/// Whether `version` is in `range`: comparators (`>=1.2.0 <2.0.0`, `=1.4.0`, `1.4.0`), all of
/// which must hold. An unreadable range or version takes nothing.
pub fn in_range(range: &str, version: &str) -> bool {
    let Ok(version) = version.parse::<Version>() else {
        return false;
    };
    let comparators: Vec<&str> = range.split_whitespace().collect();
    !comparators.is_empty()
        && comparators.iter().all(|comparator| {
            let (op, bound) = [">=", "<=", ">", "<", "="]
                .iter()
                .find_map(|op| comparator.strip_prefix(op).map(|rest| (*op, rest)))
                .unwrap_or(("=", comparator));
            let Ok(bound) = bound.parse::<Version>() else {
                return false;
            };
            match op {
                ">=" => version >= bound,
                "<=" => version <= bound,
                ">" => version > bound,
                "<" => version < bound,
                _ => version == bound,
            }
        })
}

/// The mapping a response answering `questionnaire` (`url|version`) is extracted with: the
/// newest of `mappings` for that Questionnaire whose range takes its version.
pub fn choose<'a>(mappings: &'a [Mapping], questionnaire: &str) -> Option<&'a Mapping> {
    let (url, version) = questionnaire.split_once('|')?;
    mappings
        .iter()
        .filter(|m| m.questionnaire == url && in_range(&m.versions, version))
        .filter_map(|m| m.version.parse::<Version>().ok().map(|v| (v, m)))
        .max_by(|(a, _), (b, _)| a.cmp(b))
        .map(|(_, m)| m)
}

/// The mapping (`url|version`) a response records, if any.
pub fn recorded_mapping(response: &Value) -> Option<&str> {
    response["extension"]
        .as_array()?
        .iter()
        .find(|e| e["url"] == MAPPING_EXTENSION)?["valueCanonical"]
        .as_str()
}

/// `response` with `mapping` (`url|version`) recorded on it.
pub fn record_mapping(response: &mut Value, mapping: &str) {
    let extension = json!({ "url": MAPPING_EXTENSION, "valueCanonical": mapping });
    match response["extension"].as_array_mut() {
        Some(list) => {
            list.retain(|e| e["url"] != MAPPING_EXTENSION);
            list.push(extension);
        }
        None => response["extension"] = json!([extension]),
    }
}

/// One thing a completion writes.
#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    /// A new resource, or a new version of a managed one.
    Write {
        /// Its type.
        resource_type: String,
        /// Its id.
        id: ResourceId,
        /// The resource.
        resource: Value,
    },
    /// A managed resource the plan no longer has, with its retraction status set.
    Retract {
        /// Its type.
        resource_type: String,
        /// Its id.
        id: ResourceId,
        /// The resource, retracted.
        resource: Value,
    },
    /// A seed resource the amended response would change: a "source form amended" review item
    /// for the responsible clinician. It is not rewritten.
    Review {
        /// Its type.
        resource_type: String,
        /// Its id.
        id: ResourceId,
    },
}

/// `Type/id`.
pub fn key(resource_type: &str, id: impl std::fmt::Display) -> String {
    format!("{resource_type}/{id}")
}

/// The resource without `meta`, which the store and the write path stamp.
fn content(resource: &Value) -> Value {
    let mut copy = resource.clone();
    if let Some(object) = copy.as_object_mut() {
        object.remove("meta");
    }
    copy
}

fn mode_of(resource: &Value) -> Option<&str> {
    resource["meta"]["tag"]
        .as_array()?
        .iter()
        .find(|t| t["system"] == EXTRACTION_TAG_SYSTEM)?["code"]
        .as_str()
}

/// `resource` with its status set to `status`: Condition's verification status, else `status`.
fn retracted(resource: &Value, status: &str) -> Value {
    let mut copy = resource.clone();
    if copy["resourceType"] == "Condition" {
        copy["verificationStatus"] = json!({ "coding": [{
            "system": "http://terminology.hl7.org/CodeSystem/condition-ver-status",
            "code": status
        }] });
    } else {
        copy["status"] = json!(status);
    }
    copy
}

/// What a completion writes, given the plan, the current version of every resource it or the last
/// run names (`Type/id` → resource), and the last run's targets (`Type/id`). `mapping` gives a
/// retracted resource's status (its type's managed template), `entered-in-error` otherwise.
pub fn reconcile(
    plan: &[Planned],
    current: &BTreeMap<String, Value>,
    previous: &[String],
    mapping: &Mapping,
) -> Vec<Step> {
    let mut steps = Vec::new();
    for planned in plan {
        let here = key(&planned.resource_type, planned.id);
        let existing = current.get(&here);
        let unchanged = existing.is_some_and(|e| content(e) == content(&planned.resource));
        match (planned.mode, existing) {
            (_, None) => steps.push(Step::Write {
                resource_type: planned.resource_type.clone(),
                id: planned.id,
                resource: planned.resource.clone(),
            }),
            (Mode::Managed, Some(_)) if !unchanged => steps.push(Step::Write {
                resource_type: planned.resource_type.clone(),
                id: planned.id,
                resource: planned.resource.clone(),
            }),
            (Mode::Seed, Some(_)) if !unchanged => steps.push(Step::Review {
                resource_type: planned.resource_type.clone(),
                id: planned.id,
            }),
            _ => {}
        }
    }
    let planned: Vec<String> = plan.iter().map(|p| key(&p.resource_type, p.id)).collect();
    for gone in previous.iter().filter(|k| !planned.contains(k)) {
        let Some(resource) = current.get(gone) else {
            continue;
        };
        if mode_of(resource) != Some(Mode::Managed.code()) {
            continue;
        }
        let Some((resource_type, id)) = gone.split_once('/') else {
            continue;
        };
        let Ok(id) = id.parse::<ResourceId>() else {
            continue;
        };
        let status = mapping
            .templates
            .iter()
            .find(|t| t.mode == Mode::Managed && t.resource_type == resource_type)
            .and_then(|t| t.retraction.clone())
            .unwrap_or_else(|| "entered-in-error".into());
        let already = if resource_type == "Condition" {
            resource["verificationStatus"]["coding"][0]["code"] == status.as_str()
        } else {
            resource["status"] == status.as_str()
        };
        if !already {
            steps.push(Step::Retract {
                resource_type: resource_type.to_owned(),
                id,
                resource: retracted(resource, &status),
            });
        }
    }
    steps
}

/// The id of the Provenance of `response`'s extraction runs.
pub fn provenance_id(response: ResourceId) -> ResourceId {
    derive_id(&DerivationName::rule(response, PROVENANCE_RULE))
}

/// The targets (`Type/id`) a run's Provenance names, without their versions.
pub fn targets(provenance: &Value) -> Vec<String> {
    provenance["target"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|t| t["reference"].as_str())
        .map(|r| r.split("/_history/").next().unwrap_or(r).to_owned())
        .collect()
}

/// What a run's Provenance records.
pub struct Run<'a> {
    /// The response, as stored.
    pub response: &'a Value,
    /// The response's stored version id.
    pub response_version: &'a str,
    /// The mapping, `url|version`.
    pub mapping: &'a str,
    /// Every version this run wrote or retracted, and every target it left as it was:
    /// `Type/id/_history/vid`.
    pub targets: &'a [String],
    /// When, as an instant.
    pub recorded: &'a str,
}

/// The Provenance of a run.
pub fn provenance(run: &Run<'_>, response: ResourceId) -> Value {
    let mut agents = Vec::new();
    if run.response["author"].is_object() {
        agents.push(json!({
            "type": { "coding": [{ "system": "http://terminology.hl7.org/CodeSystem/provenance-participant-type", "code": "author" }] },
            "who": run.response["author"]
        }));
    }
    agents.push(json!({
        "type": { "coding": [{ "system": "http://terminology.hl7.org/CodeSystem/provenance-participant-type", "code": "assembler" }] },
        "who": { "display": "health-fhir-extraction" }
    }));
    json!({
        "resourceType": "Provenance",
        "id": provenance_id(response).to_string(),
        "target": run.targets.iter().map(|t| json!({ "reference": t })).collect::<Vec<_>>(),
        "recorded": run.recorded,
        "activity": { "coding": [{ "system": PLATFORM_CODES, "code": "extraction" }] },
        "agent": agents,
        "entity": [
            {
                "role": "source",
                "what": { "reference": format!("QuestionnaireResponse/{response}/_history/{}", run.response_version) }
            },
            { "role": "instantiates", "what": { "display": run.mapping } }
        ]
    })
}

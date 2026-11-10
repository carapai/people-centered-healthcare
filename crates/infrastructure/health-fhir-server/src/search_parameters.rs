//! The search parameters a node supports (#104, #106): the built-in registry, and the active
//! packages' SearchParameters added to it (#89). Central and the edge share it, so a search
//! means the same on both.

/// Each patient summary section's type (#104), the notes' (#107), the requests' (#114) and the
/// rest of the timeline's (#115), with the element naming its patient.
const SUMMARY_PATIENT_PATHS: [(&str, &str); 11] = [
    ("AllergyIntolerance", "patient"),
    ("Condition", "subject"),
    ("MedicationRequest", "subject"),
    ("Observation", "subject"),
    ("Encounter", "subject"),
    ("EpisodeOfCare", "patient"),
    // A patient's notes (#107).
    ("Composition", "subject"),
    // A patient's requests (#114).
    ("ServiceRequest", "subject"),
    ("Appointment", "subject"),
    // The rest of the timeline (#115).
    ("DiagnosticReport", "subject"),
    ("Task", "for"),
];

/// The search parameters a node supports, central and edge alike (#106):
/// - `_tag` on every facility-scope type (ADR 0057), and `_security` on them, Patient and the
///   summary's types, and every other type searched here, which the search policy excludes
///   labelled records by (a type without it would find nothing once a label rule exists);
/// - Questionnaire `status`, `url` and `version`, and QuestionnaireResponse `status`, for forms
///   and their re-validation (#92);
/// - Library `type`, `url` and `version`, which completion finds Extraction Mappings by (#93);
/// - Patient `family` and `given`, which reception finds a patient by (#102);
/// - `patient` on the types the patient summary is drawn from (#104);
/// - PlanDefinition `type`, which the programs are found by (#116).
///
/// Activation (#89) replaces this with the SearchParameters of the published packages a node has
/// active.
pub fn search_registry() -> health_fhir_search::Registry {
    registry_with(&[], None).0
}

/// The built-in parameters with the active packages' SearchParameters `extra` (#89). A
/// package parameter `previous` already searches stays active; a new one is `Reindexing` until
/// its backfill finishes, and is returned by resource type to be reindexed.
pub fn registry_with(
    extra: &[serde_json::Value],
    previous: Option<&health_fhir_search::Registry>,
) -> (
    health_fhir_search::Registry,
    std::collections::BTreeMap<String, Vec<String>>,
) {
    let definition = |url: &str, code: &str, base: &str, kind: &str, expression: &str| {
        serde_json::json!({
            "resourceType": "SearchParameter",
            "url": url,
            "code": code,
            "base": [base],
            "type": kind,
            "expression": expression,
        })
    };
    let mut definitions = vec![
        definition(
            "http://hl7.org/fhir/SearchParameter/Resource-security",
            "_security",
            "Resource",
            "token",
            "Resource.meta.security",
        ),
        definition(
            "http://hl7.org/fhir/SearchParameter/Resource-tag",
            "_tag",
            "Resource",
            "token",
            "Resource.meta.tag",
        ),
        definition(
            "http://hl7.org/fhir/SearchParameter/Questionnaire-status",
            "status",
            "Questionnaire",
            "token",
            "Questionnaire.status",
        ),
        definition(
            "http://hl7.org/fhir/SearchParameter/Questionnaire-url",
            "url",
            "Questionnaire",
            "uri",
            "Questionnaire.url",
        ),
        definition(
            "http://hl7.org/fhir/SearchParameter/Questionnaire-version",
            "version",
            "Questionnaire",
            "token",
            "Questionnaire.version",
        ),
        definition(
            "http://hl7.org/fhir/SearchParameter/QuestionnaireResponse-status",
            "status",
            "QuestionnaireResponse",
            "token",
            "QuestionnaireResponse.status",
        ),
        definition(
            "http://hl7.org/fhir/SearchParameter/PlanDefinition-type",
            "type",
            "PlanDefinition",
            "token",
            "PlanDefinition.type",
        ),
        definition(
            "http://hl7.org/fhir/SearchParameter/Library-type",
            "type",
            "Library",
            "token",
            "Library.type",
        ),
        definition(
            "http://hl7.org/fhir/SearchParameter/Library-url",
            "url",
            "Library",
            "uri",
            "Library.url",
        ),
        definition(
            "http://hl7.org/fhir/SearchParameter/Library-version",
            "version",
            "Library",
            "token",
            "Library.version",
        ),
        definition(
            "http://hl7.org/fhir/SearchParameter/individual-family",
            "family",
            "Patient",
            "string",
            "Patient.name.family",
        ),
        definition(
            "http://hl7.org/fhir/SearchParameter/individual-given",
            "given",
            "Patient",
            "string",
            "Patient.name.given",
        ),
    ];
    // The patient a patient summary section is searched by (#104).
    for (base, path) in SUMMARY_PATIENT_PATHS {
        let mut patient = definition(
            &format!("http://hl7.org/fhir/SearchParameter/{base}-patient"),
            "patient",
            base,
            "reference",
            &format!("{base}.{path}"),
        );
        patient["target"] = serde_json::json!(["Patient"]);
        definitions.push(patient);
    }
    let catalog =
        health_fhir_search::Catalog::from_definitions(definitions.iter().chain(extra.iter()));
    let mut registry = health_fhir_search::Registry::new(jiff::tz::Offset::UTC);
    let mut labelled: Vec<&str> = health_access_policy::FACILITY_SCOPE_TYPES.to_vec();
    labelled.extend(SUMMARY_PATIENT_PATHS.iter().map(|(t, _)| *t));
    labelled.push("Patient");
    // Every other type a node searches carries `_security` too: the search policy excludes a
    // label rule's records by `_security:not`, which finds nothing of a type without it (#106).
    labelled.extend(["Questionnaire", "Library", "PlanDefinition"]);
    labelled.sort_unstable();
    labelled.dedup();
    let supported = health_access_policy::FACILITY_SCOPE_TYPES
        .iter()
        .map(|t| (*t, "_tag"))
        // Labels: the search policy leaves out what a subject may not see by `_security:not`.
        .chain(labelled.into_iter().map(|t| (t, "_security")))
        .chain(SUMMARY_PATIENT_PATHS.iter().map(|(t, _)| (*t, "patient")))
        .chain([
            ("Questionnaire", "status"),
            ("Questionnaire", "url"),
            ("Questionnaire", "version"),
            ("QuestionnaireResponse", "status"),
            ("PlanDefinition", "type"),
            ("Library", "type"),
            ("Library", "url"),
            ("Library", "version"),
            ("Patient", "family"),
            ("Patient", "given"),
        ]);
    for (resource_type, code) in supported {
        if registry
            .support(
                &catalog,
                resource_type,
                code,
                health_fhir_search::ParamState::Active,
            )
            .is_err()
        {
            tracing::warn!(resource_type, code, "a search parameter is not supported");
        }
    }
    let mut reindex: std::collections::BTreeMap<String, Vec<String>> =
        std::collections::BTreeMap::new();
    for parameter in extra {
        let Some(code) = parameter["code"].as_str() else {
            continue;
        };
        for base in parameter["base"].as_array().into_iter().flatten() {
            let Some(resource_type) = base.as_str() else {
                continue;
            };
            if registry.get(resource_type, code).is_some() {
                continue;
            }
            let searched = previous
                .and_then(|r| r.get(resource_type, code))
                .is_some_and(|s| s.state == health_fhir_search::ParamState::Active);
            let state = if searched {
                health_fhir_search::ParamState::Active
            } else {
                health_fhir_search::ParamState::Reindexing
            };
            // Searchable under the label rules, as every type is.
            if registry.get(resource_type, "_security").is_none() {
                let _ = registry.support(&catalog, resource_type, "_security", state);
            }
            match registry.support(&catalog, resource_type, code, state) {
                Ok(()) if !searched => reindex
                    .entry(resource_type.to_owned())
                    .or_default()
                    .push(code.to_owned()),
                Ok(()) => {}
                Err(_) => tracing::warn!(
                    resource_type,
                    code,
                    "an active package's search parameter does not compile"
                ),
            }
        }
    }
    (registry, reindex)
}

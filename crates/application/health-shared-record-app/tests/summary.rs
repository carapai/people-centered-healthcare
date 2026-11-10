//! The patient summary from FHIR R5 resources (#104): each section mapped from its type, vitals
//! only from vital-signs Observations, decimals in their scale, and the patients a golden record
//! links.

use health_shared_record_app::{Found, SECTIONS, golden_of, linked, record, summary};
use serde_json::{Value, json};

fn found() -> Found {
    let weight: Value = serde_json::from_str(
        r#"{ "resourceType": "Observation", "id": "w1", "status": "final",
             "category": [{ "coding": [{ "system": "http://terminology.hl7.org/CodeSystem/observation-category", "code": "vital-signs" }] }],
             "code": { "coding": [{ "system": "http://loinc.org", "code": "29463-7", "display": "Body weight" }] },
             "effectiveDateTime": "2026-11-05T09:30:00Z",
             "valueQuantity": { "value": 61.50, "unit": "kg" } }"#,
    )
    .expect("json");
    [
        Some(vec![json!({
            "resourceType": "AllergyIntolerance", "id": "a1", "criticality": "high",
            "clinicalStatus": { "coding": [{ "code": "active" }] },
            "code": { "text": "Penicillin" }, "recordedDate": "2026-01-01"
        })]),
        Some(vec![json!({
            "resourceType": "Condition", "id": "c1",
            "clinicalStatus": { "coding": [{ "code": "active" }] },
            "code": { "coding": [{ "system": "http://snomed.info/sct", "code": "38341003", "display": "Hypertension" }] },
            "onsetDateTime": "2025-03-01"
        })]),
        Some(vec![
            json!({
                "resourceType": "MedicationRequest", "id": "m1", "status": "active", "intent": "order",
                "medication": { "concept": { "coding": [{ "code": "387517004", "display": "Paracetamol" }] } },
                "dosageInstruction": [{ "text": "1 g every 8 hours" }], "authoredOn": "2026-11-05"
            }),
            json!({ "resourceType": "MedicationRequest", "id": "no-status", "intent": "order" }),
        ]),
        Some(vec![
            weight,
            json!({ "resourceType": "Observation", "id": "lab", "status": "final",
                    "category": [{ "coding": [{ "code": "laboratory" }] }], "code": { "text": "Hb" } }),
        ]),
        Some(vec![json!({
            "resourceType": "Encounter", "id": "e1", "status": "in-progress",
            "class": [{ "coding": [{ "code": "AMB" }] }],
            "actualPeriod": { "start": "2026-11-05T08:00:00Z" },
            "location": [{ "location": { "display": "KAM013/triage" } }]
        })]),
        None,
    ]
}

#[test]
fn each_section_is_mapped_from_its_type() {
    assert_eq!(SECTIONS.len(), found().len());
    let record = record(&found());
    let allergy = &record.allergies.as_ref().expect("allergies")[0];
    assert_eq!(
        (allergy.reference.as_str(), allergy.substance.text.as_str()),
        ("AllergyIntolerance/a1", "Penicillin")
    );
    assert_eq!(allergy.criticality.as_deref(), Some("high"));
    let condition = &record.conditions.as_ref().expect("conditions")[0];
    assert_eq!(condition.code.text, "Hypertension");
    assert_eq!(condition.code.code.as_deref(), Some("38341003"));
    let medications = record.medications.as_ref().expect("medications");
    assert_eq!(
        medications.len(),
        1,
        "a MedicationRequest without a status is left out"
    );
    assert_eq!(medications[0].dosage.as_deref(), Some("1 g every 8 hours"));
    let vitals = record.vitals.as_ref().expect("vitals");
    assert_eq!(vitals.len(), 1, "only vital signs");
    assert_eq!(
        vitals[0].value.as_deref(),
        Some("61.50"),
        "the decimal keeps its scale"
    );
    let visit = &record.visits.as_ref().expect("visits")[0];
    assert_eq!(
        (visit.class.as_deref(), visit.location.as_deref()),
        (Some("AMB"), Some("KAM013/triage"))
    );
    assert_eq!(record.programs, None, "a section not searched is not seen");
}

#[test]
fn the_summary_serializes_with_unseen_sections_as_null() {
    let text = serde_json::to_value(summary(&found())).expect("json");
    assert_eq!(text["programs"], Value::Null);
    assert_eq!(text["vitals"][0]["value"], "61.50");
    assert_eq!(text["allergies"][0]["clinicalStatus"], "active");
    let mut program = found();
    program[5] = Some(vec![json!({
        "resourceType": "EpisodeOfCare", "id": "art", "status": "active",
        "type": [{ "text": "HIV care" }], "period": { "start": "2024-01-01" }
    })]);
    let text = serde_json::to_value(summary(&program)).expect("json");
    assert_eq!(text["programs"][0]["kind"]["text"], "HIV care");
}

#[test]
fn the_golden_record_links_the_same_person_elsewhere() {
    let here = json!({ "resourceType": "Patient", "id": "p-here", "link": [
        { "other": { "reference": "Patient/golden" }, "type": "refer" } ] });
    let golden = json!({ "resourceType": "Patient", "id": "golden", "link": [
        { "other": { "reference": "Patient/p-here" }, "type": "seealso" },
        { "other": { "reference": "Patient/p-there" }, "type": "seealso" },
        { "other": { "reference": "RelatedPerson/r" }, "type": "seealso" },
        { "other": { "reference": "Patient/old" }, "type": "replaces" } ] });
    assert_eq!(golden_of(&here).as_deref(), Some("Patient/golden"));
    assert_eq!(
        linked(&here, Some(&golden)),
        ["Patient/p-here", "Patient/p-there"]
    );
    let alone = json!({ "resourceType": "Patient", "id": "p" });
    assert_eq!(golden_of(&alone), None);
    assert_eq!(linked(&alone, None), ["Patient/p"]);
}

#[test]
fn a_panel_shows_its_components_in_their_unit() {
    let vital = |id: &str, components: Value| {
        json!({ "resourceType": "Observation", "id": id, "status": "final",
                "category": [{ "coding": [{ "code": "vital-signs" }] }],
                "code": { "coding": [{ "system": "http://loinc.org", "code": "85354-9", "display": "Blood pressure panel" }] },
                "component": components })
    };
    let quantity = |code: &str, value: i64, unit: &str| json!({ "code": { "coding": [{ "code": code }] }, "valueQuantity": { "value": value, "unit": unit } });
    let mut sections = found();
    sections[3] = Some(vec![
        vital(
            "bp",
            json!([
                quantity("8480-6", 104, "mmHg"),
                quantity("8462-4", 66, "mmHg")
            ]),
        ),
        vital(
            "mixed",
            json!([quantity("a", 1, "kg"), quantity("b", 2, "cm")]),
        ),
    ]);
    let record = record(&sections);
    let vitals = record.vitals.as_ref().expect("vitals");
    assert_eq!(
        (vitals[0].value.as_deref(), vitals[0].unit.as_deref()),
        (Some("104/66"), Some("mmHg"))
    );
    assert_eq!(
        vitals[1].value, None,
        "components in different units are not joined"
    );
}

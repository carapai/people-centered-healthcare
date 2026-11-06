//! The timeline from FHIR R5 resources (#115): what each type shows, what stays off it, and that
//! the same resources in any order make the same timeline.

use health_shared_record::timeline::{Change, Kind};
use health_shared_record_app::timeline::{EPISODE_EXTENSION, change, timeline};
use serde_json::{Value, json};

fn versioned(mut resource: Value, id: &str, version: &str) -> Value {
    resource["id"] = json!(id);
    resource["meta"] = json!({ "versionId": version, "tag": [{ "system": "urn:health:facility", "code": "KAM013" }] });
    resource
}

fn resources() -> Vec<Value> {
    vec![
        versioned(
            json!({ "resourceType": "Encounter", "status": "completed", "type": [{ "coding": [{ "code": "OPD", "display": "Outpatient" }] }],
            "episodeOfCare": [{ "reference": "EpisodeOfCare/e1" }], "actualPeriod": { "start": "2026-11-02T09:00:00Z" } }),
            "v1",
            "a1",
        ),
        versioned(
            json!({ "resourceType": "EpisodeOfCare", "status": "active", "type": [{ "text": "Diabetes care" }],
            "period": { "start": "2026-10-01T08:00:00Z" } }),
            "e1",
            "a2",
        ),
        versioned(
            json!({ "resourceType": "Observation", "status": "final", "code": { "text": "HbA1c" },
            "extension": [{ "url": EPISODE_EXTENSION, "valueReference": { "reference": "EpisodeOfCare/e1" } }],
            "effectiveDateTime": "2026-11-02T09:30:00Z" }),
            "o1",
            "a3",
        ),
        versioned(
            json!({ "resourceType": "ServiceRequest", "status": "active", "intent": "order",
            "code": { "concept": { "text": "Lipid panel" } }, "authoredOn": "2026-11-02T09:40:00Z" }),
            "s1",
            "a4",
        ),
        versioned(
            json!({ "resourceType": "MedicationRequest", "status": "active", "intent": "order",
            "medication": { "concept": { "text": "Metformin 500 mg" } }, "authoredOn": "2026-11-02T09:50:00Z" }),
            "m1",
            "a5",
        ),
        versioned(
            json!({ "resourceType": "DiagnosticReport", "status": "final", "code": { "text": "Lipid panel" },
            "issued": "2026-11-03T10:00:00Z" }),
            "d1",
            "a6",
        ),
        versioned(
            json!({ "resourceType": "Appointment", "status": "booked", "description": "Diabetes review",
            "start": "2026-12-01T09:00:00Z" }),
            "p1",
            "a7",
        ),
        versioned(
            json!({ "resourceType": "Composition", "status": "final", "title": "Progress note",
            "date": "2026-11-02T10:00:00Z" }),
            "c1",
            "a8",
        ),
        versioned(
            json!({ "resourceType": "Task", "status": "requested", "description": "Collect specimen",
            "authoredOn": "2026-11-02T09:45:00Z" }),
            "t1",
            "a9",
        ),
    ]
}

#[test]
fn each_type_shows_what_happened_when_and_where() {
    let built = timeline(&resources());
    let shown: Vec<(Kind, &str, &str)> = built
        .entries()
        .iter()
        .map(|e| (e.kind, e.title.as_str(), e.at.as_str()))
        .collect();
    assert_eq!(
        shown,
        [
            (Kind::Appointment, "Diabetes review", "2026-12-01T09:00:00Z"),
            (Kind::Result, "Lipid panel", "2026-11-03T10:00:00Z"),
            (Kind::Note, "Progress note", "2026-11-02T10:00:00Z"),
            (Kind::Medication, "Metformin 500 mg", "2026-11-02T09:50:00Z"),
            (Kind::Task, "Collect specimen", "2026-11-02T09:45:00Z"),
            (Kind::Order, "Lipid panel", "2026-11-02T09:40:00Z"),
            (Kind::Observation, "HbA1c", "2026-11-02T09:30:00Z"),
            (Kind::Visit, "Outpatient", "2026-11-02T09:00:00Z"),
            (Kind::Program, "Diabetes care", "2026-10-01T08:00:00Z"),
        ]
    );
    let entries = built.entries();
    let visit = entries
        .iter()
        .find(|e| e.kind == Kind::Visit)
        .expect("visit");
    let reading = entries
        .iter()
        .find(|e| e.kind == Kind::Observation)
        .expect("observation");
    assert_eq!(
        visit.episode.as_deref(),
        Some("EpisodeOfCare/e1"),
        "from episodeOfCare"
    );
    assert_eq!(
        reading.episode.as_deref(),
        Some("EpisodeOfCare/e1"),
        "from the extension"
    );
    assert!(
        entries
            .iter()
            .all(|e| e.facility.as_deref() == Some("KAM013"))
    );
    assert_eq!(
        (visit.reference.as_str(), visit.version.as_str()),
        ("Encounter/v1", "a1")
    );
}

#[test]
fn the_same_resources_in_any_order_make_the_same_timeline() {
    let forward = timeline(&resources());
    let mut reversed = resources();
    reversed.reverse();
    assert_eq!(timeline(&reversed).entries(), forward.entries());
    let mut twice = resources();
    twice.extend(resources());
    assert_eq!(timeline(&twice).entries(), forward.entries());
}

#[test]
fn errors_drafts_and_unverified_results_stay_off() {
    let off = [
        json!({ "resourceType": "Observation", "status": "entered-in-error", "code": { "text": "x" }, "effectiveDateTime": "2026-11-02T09:30:00Z" }),
        json!({ "resourceType": "Observation", "status": "cancelled", "code": { "text": "x" }, "effectiveDateTime": "2026-11-02T09:30:00Z" }),
        json!({ "resourceType": "DiagnosticReport", "status": "preliminary", "code": { "text": "x" }, "issued": "2026-11-02T09:30:00Z" }),
        json!({ "resourceType": "Composition", "status": "preliminary", "title": "x", "date": "2026-11-02T09:30:00Z" }),
        json!({ "resourceType": "MedicationRequest", "status": "draft", "intent": "order", "authoredOn": "2026-11-02T09:30:00Z" }),
    ];
    for (i, resource) in off.into_iter().enumerate() {
        let version = format!("b{i}");
        assert!(
            matches!(
                change(&versioned(resource, "x1", &version)),
                Some(Change::Gone { .. })
            ),
            "case {i}"
        );
    }
    // Not a timeline type, or without a version or a time: nothing.
    assert_eq!(
        change(&versioned(json!({ "resourceType": "Patient" }), "p", "v")),
        None
    );
    assert_eq!(
        change(&json!({ "resourceType": "Encounter", "id": "e", "status": "planned" })),
        None
    );
    assert_eq!(
        change(&versioned(
            json!({ "resourceType": "Encounter", "status": "planned" }),
            "e",
            "v"
        )),
        None
    );
}

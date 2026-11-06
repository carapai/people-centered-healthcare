//! The Care Journey from FHIR R5 resources (#115): programs, steps in their program, what waits on
//! a Task, what stays off, and the same journey whatever the order.

use health_care_journeys::Stage;
use health_care_journeys_app::{EPISODE_EXTENSION, REFERRAL, read};
use serde_json::{Value, json};

fn versioned(mut resource: Value, id: &str, version: &str) -> Value {
    resource["id"] = json!(id);
    resource["meta"] = json!({ "versionId": version, "tag": [{ "system": "urn:health:facility", "code": "KAM013" }] });
    resource
}

fn resources() -> Vec<Value> {
    vec![
        versioned(
            json!({ "resourceType": "EpisodeOfCare", "status": "active", "type": [{ "text": "Diabetes care" }],
            "period": { "start": "2026-10-01T08:00:00Z" } }),
            "dm",
            "a1",
        ),
        versioned(
            json!({ "resourceType": "Encounter", "status": "completed", "type": [{ "text": "Diabetes clinic" }],
            "episodeOfCare": [{ "reference": "EpisodeOfCare/dm" }], "actualPeriod": { "start": "2026-10-02T09:00:00Z" } }),
            "v1",
            "a2",
        ),
        versioned(
            json!({ "resourceType": "ServiceRequest", "status": "active", "intent": "order", "code": { "concept": { "text": "HbA1c" } },
            "extension": [{ "url": EPISODE_EXTENSION, "valueReference": { "reference": "EpisodeOfCare/dm" } }],
            "authoredOn": "2026-10-02T09:30:00Z" }),
            "o1",
            "a3",
        ),
        versioned(
            json!({ "resourceType": "ServiceRequest", "status": "active", "intent": "order", "code": { "concept": { "text": "Eye clinic" } },
            "category": [{ "coding": [{ "system": "http://snomed.info/sct", "code": REFERRAL }] }],
            "authoredOn": "2026-10-02T09:40:00Z" }),
            "r1",
            "a4",
        ),
        versioned(
            json!({ "resourceType": "DiagnosticReport", "status": "final", "code": { "text": "HbA1c" },
            "issued": "2026-10-03T10:00:00Z" }),
            "d1",
            "a5",
        ),
        versioned(
            json!({ "resourceType": "MedicationRequest", "status": "active", "intent": "order",
            "medication": { "concept": { "text": "Metformin 500 mg" } }, "authoredOn": "2026-10-02T09:50:00Z" }),
            "m1",
            "a6",
        ),
        versioned(
            json!({ "resourceType": "Appointment", "status": "booked", "description": "Diabetes review",
            "start": "2026-12-01T09:00:00Z" }),
            "p1",
            "a7",
        ),
        versioned(
            json!({ "resourceType": "Task", "status": "in-progress", "intent": "order",
            "focus": { "reference": "ServiceRequest/r1" } }),
            "t1",
            "a8",
        ),
        versioned(
            json!({ "resourceType": "Task", "status": "completed", "intent": "order",
            "focus": { "reference": "ServiceRequest/o1" } }),
            "t2",
            "a9",
        ),
    ]
}

/// A stretch's program title and its steps: stage, title and whether it waits.
type Shape<'a> = (Option<&'a str>, Vec<(Stage, &'a str, bool)>);

#[test]
fn programs_hold_their_steps_and_tasks_say_what_waits() {
    let built = read(&resources());
    let shape: Vec<Shape<'_>> = built
        .stretches
        .iter()
        .map(|s| {
            (
                s.program.as_ref().map(|p| p.title.as_str()),
                s.steps
                    .iter()
                    .map(|st| (st.stage, st.title.as_str(), st.waiting))
                    .collect(),
            )
        })
        .collect();
    assert_eq!(
        shape,
        [
            (
                Some("Diabetes care"),
                vec![
                    (Stage::Visit, "Diabetes clinic", false),
                    (Stage::Order, "HbA1c", false)
                ]
            ),
            (
                None,
                vec![
                    (Stage::Referral, "Eye clinic", true),
                    (Stage::Medication, "Metformin 500 mg", false),
                    (Stage::Result, "HbA1c", false),
                    (Stage::Appointment, "Diabetes review", false),
                ]
            ),
        ]
    );
    let waiting: Vec<&str> = built
        .waiting()
        .iter()
        .map(|s| s.reference.as_str())
        .collect();
    assert_eq!(
        waiting,
        ["ServiceRequest/r1"],
        "the referral's Task is open; the order's is done"
    );
    let program = built.stretches[0].program.as_ref().expect("program");
    assert_eq!(
        (program.reference.as_str(), program.facility.as_deref()),
        ("EpisodeOfCare/dm", Some("KAM013"))
    );
}

#[test]
fn the_same_resources_in_any_order_make_the_same_journey() {
    let built = read(&resources());
    let mut reversed = resources();
    reversed.reverse();
    assert_eq!(read(&reversed), built);
    let mut twice = resources();
    twice.extend(resources());
    assert_eq!(read(&twice), built);
}

#[test]
fn errors_drafts_and_unverified_results_stay_off() {
    let off = [
        versioned(
            json!({ "resourceType": "Encounter", "status": "entered-in-error", "actualPeriod": { "start": "2026-10-02T09:00:00Z" } }),
            "x1",
            "b1",
        ),
        versioned(
            json!({ "resourceType": "DiagnosticReport", "status": "preliminary", "issued": "2026-10-02T09:00:00Z" }),
            "x2",
            "b2",
        ),
        versioned(
            json!({ "resourceType": "MedicationRequest", "status": "draft", "authoredOn": "2026-10-02T09:00:00Z" }),
            "x3",
            "b3",
        ),
        versioned(
            json!({ "resourceType": "EpisodeOfCare", "status": "entered-in-error" }),
            "x4",
            "b4",
        ),
        // Not a step, or missing its version or time.
        versioned(
            json!({ "resourceType": "Observation", "status": "final", "effectiveDateTime": "2026-10-02T09:00:00Z" }),
            "x5",
            "b5",
        ),
        json!({ "resourceType": "Encounter", "id": "x6", "status": "completed", "actualPeriod": { "start": "2026-10-02T09:00:00Z" } }),
        versioned(
            json!({ "resourceType": "Encounter", "status": "planned" }),
            "x7",
            "b7",
        ),
        // A Task about nothing is no one's work.
        versioned(
            json!({ "resourceType": "Task", "status": "requested" }),
            "x8",
            "b8",
        ),
    ];
    assert_eq!(read(&off).stretches, vec![]);
}

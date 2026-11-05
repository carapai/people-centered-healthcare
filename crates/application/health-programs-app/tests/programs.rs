//! Programs over SurrealDB 3.3.5 in memory (#116): a patient enrolled in ART and TB concurrently,
//! one open episode per program, the episode lifecycle and the transfer hand-over, and program
//! definitions with their eligibility.

use std::sync::Arc;

use health_fhir_validation::{OctofhirEngine, validate_structure};
use health_persistence::{ResourceType, Store, Transaction};
use health_programs::{Enrolment, EpisodeStatus, Program, ProgramRefused};
use health_programs_app::{ProgramError, Programs, eligible, episode, fhir, program};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::{SystemClock, Timestamp};
use serde_json::{Value, json};

fn at(text: &str) -> Timestamp {
    text.parse().expect("timestamp")
}

async fn store() -> SurrealStore {
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: "programs".into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    SurrealStore::new(connection, Arc::new(SystemClock))
}

fn plan(name: &str, episode_type: &str, eligibility: Option<&str>) -> Value {
    let mut plan = json!({
        "resourceType": "PlanDefinition", "id": name, "status": "active",
        "url": format!("https://health.go.ug/fhir/PlanDefinition/{name}"), "version": "1.0.0",
        "type": { "coding": [{ "system": "http://terminology.hl7.org/CodeSystem/plan-definition-type", "code": "clinical-protocol" }] },
        "extension": [{
            "url": "https://carapai.github.io/people-centered-healthcare/fhir/StructureDefinition/program-episode-type",
            "valueCodeableConcept": { "text": episode_type }
        }],
        "relatedArtifact": [
            { "type": "citation", "resource": "https://health.go.ug/protocols/consolidated-2024" },
            { "type": "documentation", "document": { "url": "https://health.go.ug/protocols/handbook.pdf" } },
            { "type": "composed-of", "resource": format!("https://health.go.ug/fhir/ActivityDefinition/{name}-first-line") },
            { "type": "composed-of", "resource": format!("https://health.go.ug/fhir/Library/{name}-visit") }
        ]
    });
    if let Some(expression) = eligibility {
        plan["extension"].as_array_mut().expect("extensions").push(json!({
            "url": "https://carapai.github.io/people-centered-healthcare/fhir/StructureDefinition/program-eligibility",
            "valueExpression": { "language": "text/fhirpath", "expression": expression }
        }));
    }
    plan
}

fn art() -> Program {
    program(&plan("art-program", "HIV care", Some("%patient.active"))).expect("program")
}

fn tb() -> Program {
    program(&plan("tb-program", "TB treatment", None)).expect("program")
}

fn enrolment<'a>(id: u32, program: &'a Program, facility: &str) -> Enrolment<'a> {
    Enrolment {
        id: format!("0190f1a2-7c3d-7e4f-8a5b-{id:012}"),
        patient: "Patient/p-1".into(),
        program,
        facility: facility.into(),
        eligible: true,
        by: "clinician-1".into(),
        at: at("2026-11-05T10:00:00Z"),
        transferred_from: None,
    }
}

#[test]
fn a_program_plan_definition_is_read() {
    let art = art();
    assert_eq!(
        art.url,
        "https://health.go.ug/fhir/PlanDefinition/art-program"
    );
    assert_eq!(art.episode_type.text, "HIV care");
    assert_eq!(art.eligibility.as_deref(), Some("%patient.active"));
    assert_eq!(
        art.sources,
        [
            "https://health.go.ug/protocols/consolidated-2024",
            "https://health.go.ug/protocols/handbook.pdf"
        ]
    );
    assert_eq!(
        art.regimens,
        ["https://health.go.ug/fhir/ActivityDefinition/art-program-first-line"]
    );
    assert_eq!(
        art.workflows,
        ["https://health.go.ug/fhir/Library/art-program-visit"]
    );

    let mut order_set = plan("x", "X", None);
    order_set["type"]["coding"][0]["code"] = json!("order-set");
    assert!(matches!(
        program(&order_set),
        Err(ProgramError::NotAProgram(_))
    ));
    let mut untyped = plan("x", "X", None);
    untyped["extension"] = json!([]);
    assert!(matches!(
        program(&untyped),
        Err(ProgramError::NotAProgram(_))
    ));
    assert!(matches!(
        program(&json!({ "resourceType": "Library" })),
        Err(ProgramError::NotAProgram(_))
    ));
}

#[tokio::test]
async fn eligibility_is_the_programs_fhirpath_over_the_patient() {
    let engine = OctofhirEngine::new();
    let active = json!({ "resourceType": "Patient", "active": true });
    let inactive = json!({ "resourceType": "Patient", "active": false });
    let silent = json!({ "resourceType": "Patient" });
    assert!(eligible(&engine, &art(), &active).await.expect("decided"));
    assert!(!eligible(&engine, &art(), &inactive).await.expect("decided"));
    assert!(
        !eligible(&engine, &art(), &silent).await.expect("decided"),
        "nothing known is not eligible"
    );
    assert!(
        eligible(&engine, &tb(), &silent).await.expect("decided"),
        "no eligibility takes everyone"
    );
    let odd = program(&plan("odd", "Odd", Some("%patient.name"))).expect("program");
    let named = json!({ "resourceType": "Patient", "name": [{ "family": "A" }] });
    assert!(matches!(
        eligible(&engine, &odd, &named).await,
        Err(ProgramError::Eligibility { .. })
    ));
}

#[tokio::test]
async fn a_patient_is_enrolled_in_art_and_tb_concurrently() {
    let store = store().await;
    let programs = Programs::new(store.clone());
    let (art, tb) = (art(), tb());
    let (on_art, event) = programs
        .enrol(&enrolment(501, &art, "KAM013"))
        .await
        .expect("enrolled in ART");
    assert_eq!(event.name(), "programs.enrolled.v1");
    let (on_tb, _) = programs
        .enrol(&enrolment(502, &tb, "KAM013"))
        .await
        .expect("enrolled in TB as well");
    assert_eq!(
        (on_art.status, on_tb.status),
        (EpisodeStatus::Active, EpisodeStatus::Active)
    );
    assert_eq!(
        programs.enrol(&enrolment(503, &art, "KAM013")).await,
        Err(ProgramError::AlreadyEnrolled(art.url.clone())),
        "one open episode per program"
    );

    // The EpisodeOfCare is valid R5, and reads back as the episode.
    let kind: ResourceType = "EpisodeOfCare".parse().expect("type");
    let mut tx = store.begin().await.expect("begin");
    let head = tx
        .read(&kind, on_art.id.parse().expect("id"))
        .await
        .expect("read")
        .expect("written");
    let issues = validate_structure(head.json.as_str());
    assert!(issues.is_empty(), "{issues:?}");
    let stored: Value = serde_json::from_str(head.json.as_str()).expect("json");
    assert_eq!(stored["type"][0]["text"], "HIV care");
    assert_eq!(stored["meta"]["tag"][0]["code"], "KAM013");
    assert_eq!(episode(&stored), Some(on_art.clone()));
    assert_eq!(fhir(&on_art)["status"], "active");

    // Finishing ART releases its guard: the patient may be enrolled again later.
    let (done, _) = programs
        .transition(
            &on_art.id,
            EpisodeStatus::Finished,
            at("2027-01-01T00:00:00Z"),
        )
        .await
        .expect("finished");
    assert_eq!(done.end, Some(at("2027-01-01T00:00:00Z")));
    programs
        .enrol(&enrolment(504, &art, "KAM013"))
        .await
        .expect("enrolled again");
    assert_eq!(
        programs
            .transition(
                &on_art.id,
                EpisodeStatus::Active,
                at("2027-01-02T00:00:00Z")
            )
            .await,
        Err(ProgramError::Refused(ProgramRefused::Transition {
            from: "finished",
            to: "active"
        }))
    );
}

#[tokio::test]
async fn a_transfer_hands_the_program_over_to_the_new_facility() {
    let store = store().await;
    let programs = Programs::new(store.clone());
    let art = art();
    let (here, _) = programs
        .enrol(&enrolment(601, &art, "KAM013"))
        .await
        .expect("enrolled");
    let (left, event) = programs
        .transfer_out(&here.id, "MBR001", at("2027-03-01T00:00:00Z"))
        .await
        .expect("transferred");
    assert_eq!(event.name(), "programs.transferred-out.v1");
    assert_eq!(
        (left.status, left.transferred_to.as_deref()),
        (EpisodeStatus::Finished, Some("MBR001"))
    );
    assert_eq!(programs.episode(&here.id).await.expect("episode"), left);

    // The new facility enrols the patient in an episode that continues the old one.
    let mut arriving = enrolment(602, &art, "MBR001");
    arriving.transferred_from = Some(format!("EpisodeOfCare/{}", here.id));
    let (there, _) = programs
        .enrol(&arriving)
        .await
        .expect("the guard was released by the transfer");
    assert_eq!(
        programs
            .episode(&there.id)
            .await
            .expect("episode")
            .transferred_from,
        Some(format!("EpisodeOfCare/{}", here.id))
    );

    let unknown = "0190f1a2-7c3d-7e4f-8a5b-000000000999";
    assert_eq!(
        programs.episode(unknown).await,
        Err(ProgramError::NotFound(unknown.into()))
    );
}

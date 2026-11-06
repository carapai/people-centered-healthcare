//! Prescribing over SurrealDB 3.3.5 in memory (#108): a prescription written as a valid R5
//! MedicationRequest; safety warnings acknowledged before anything is written; the lifecycle; and
//! the pharmacy worklist and the safety checks' view of the patient from FHIR.

use std::sync::Arc;

use health_fhir_validation::validate_structure;
use health_identifiers::ResourceId;
use health_medications::{
    BasicChecks, Medicine, Prescribe, PrescriptionRefused, PrescriptionStatus,
};
use health_medications_app::{MedicationError, Medications, fhir, patient, prescription, worklist};
use health_persistence::{ResourceType, Store, Transaction};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::{SystemClock, Timestamp};
use serde_json::json;

fn at(text: &str) -> Timestamp {
    text.parse().expect("timestamp")
}

async fn store() -> SurrealStore {
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: "medications".into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    SurrealStore::new(connection, Arc::new(SystemClock))
}

fn request(n: u32, medicine: &str) -> Prescribe {
    Prescribe {
        id: format!("0190f1a2-7c3d-7e4f-8a5b-{n:012}"),
        patient: "Patient/p-1".into(),
        encounter: Some("Encounter/e-1".into()),
        facility: "KAM013".into(),
        medicine: Medicine {
            text: medicine.into(),
            system: None,
            code: None,
        },
        dosage: "4 tablets twice daily".into(),
        days: Some(3),
        prescriber: "clinician-1".into(),
        at: at("2026-11-05T10:00:00Z"),
        based_on: None,
    }
}

async fn stored(store: &SurrealStore, id: &str) -> Option<serde_json::Value> {
    let kind: ResourceType = "MedicationRequest".parse().expect("type");
    let mut tx = store.begin().await.expect("begin");
    tx.read(&kind, id.parse::<ResourceId>().expect("id"))
        .await
        .expect("read")
        .map(|h| serde_json::from_str(h.json.as_str()).expect("json"))
}

#[tokio::test]
async fn a_prescription_is_written_once_its_warnings_are_acknowledged() {
    let store = store().await;
    let medications = Medications::new(store.clone());
    let quiet = health_medications::Patient::default();
    let (written, warnings) = medications
        .prescribe(
            &request(1, "Artemether/lumefantrine"),
            &quiet,
            &BasicChecks,
            false,
        )
        .await
        .expect("prescribed");
    assert!(warnings.is_empty());
    let resource = stored(&store, &written.id).await.expect("written");
    let issues = validate_structure(&resource.to_string());
    assert!(issues.is_empty(), "{issues:?}");
    assert_eq!(
        resource["dispenseRequest"]["expectedSupplyDuration"]["value"],
        3
    );
    assert_eq!(prescription(&resource).as_ref(), Some(&written));
    assert_eq!(
        medications
            .prescribe(
                &request(1, "Artemether/lumefantrine"),
                &quiet,
                &BasicChecks,
                false
            )
            .await,
        Err(MedicationError::AlreadyWritten(written.id.clone()))
    );

    // Allergic to penicillin: amoxicillin's warning comes back, and nothing is written.
    let allergic = health_medications::Patient {
        prescriptions: vec![],
        allergies: vec!["amoxicillin".into()],
    };
    let amoxicillin = request(2, "Amoxicillin 500 mg");
    let Err(MedicationError::Unacknowledged(found)) = medications
        .prescribe(&amoxicillin, &allergic, &BasicChecks, false)
        .await
    else {
        panic!("the warning must be acknowledged first");
    };
    assert_eq!(found[0].code, "allergy");
    assert!(
        stored(&store, &amoxicillin.id).await.is_none(),
        "nothing written"
    );
    let (_, acknowledged) = medications
        .prescribe(&amoxicillin, &allergic, &BasicChecks, true)
        .await
        .expect("prescribed once acknowledged");
    assert_eq!(acknowledged.len(), 1);
    assert!(stored(&store, &amoxicillin.id).await.is_some());
    assert_eq!(
        medications
            .prescribe(
                &Prescribe {
                    dosage: String::new(),
                    ..request(3, "Paracetamol")
                },
                &quiet,
                &BasicChecks,
                false
            )
            .await,
        Err(MedicationError::Refused(PrescriptionRefused::Incomplete))
    );
}

#[tokio::test]
async fn a_prescription_changes_status() {
    let store = store().await;
    let medications = Medications::new(store.clone());
    let (written, _) = medications
        .prescribe(
            &request(1, "Paracetamol"),
            &health_medications::Patient::default(),
            &BasicChecks,
            false,
        )
        .await
        .expect("prescribed");
    let stopped = medications
        .transition(
            &written.id,
            PrescriptionStatus::Stopped,
            at("2026-11-06T00:00:00Z"),
        )
        .await
        .expect("stopped");
    assert_eq!(stopped.status, PrescriptionStatus::Stopped);
    assert_eq!(
        stored(&store, &written.id).await.expect("written")["status"],
        "stopped"
    );
    assert!(matches!(
        medications
            .transition(
                &written.id,
                PrescriptionStatus::Active,
                at("2026-11-07T00:00:00Z")
            )
            .await,
        Err(MedicationError::Refused(
            PrescriptionRefused::Transition { .. }
        ))
    ));
    let unknown = "0190f1a2-7c3d-7e4f-8a5b-000000000999";
    assert_eq!(
        medications
            .transition(
                unknown,
                PrescriptionStatus::Stopped,
                at("2026-11-07T00:00:00Z")
            )
            .await,
        Err(MedicationError::NotFound(unknown.into()))
    );
}

#[test]
fn the_worklist_and_the_patients_record_come_from_fhir() {
    let open = health_medications::prescribe(&request(1, "Paracetamol"))
        .expect("prescribed")
        .0;
    let done = health_medications::prescribe(&request(2, "Iron"))
        .expect("prescribed")
        .0
        .transition(PrescriptionStatus::Completed, at("2026-11-05T11:00:00Z"))
        .expect("done")
        .0;
    let resources = vec![
        fhir(&open),
        fhir(&done),
        json!({ "resourceType": "AllergyIntolerance", "code": { "text": "Sulfa" }, "clinicalStatus": { "coding": [{ "code": "active" }] } }),
        json!({ "resourceType": "AllergyIntolerance", "code": { "text": "Peanut" }, "clinicalStatus": { "coding": [{ "code": "resolved" }] } }),
        json!({ "resourceType": "AllergyIntolerance", "code": { "coding": [{ "display": "Latex" }] }, "verificationStatus": { "coding": [{ "code": "refuted" }] } }),
    ];
    let known = patient(&resources);
    assert_eq!(known.prescriptions, vec![open.clone()]);
    assert_eq!(known.allergies, ["Sulfa"]);
    assert_eq!(worklist(&resources, "KAM013"), vec![open]);
    assert_eq!(worklist(&resources, "MBR001"), vec![]);
}

//! Patient requests over SurrealDB 3.3.5 in memory (#114): proposals only for a linked patient,
//! a refill only of their own prescription at its facility, and withdrawal, decline and
//! acceptance only while pending.

use std::sync::Arc;

use health_client_registry::person::PortalAccess;
use health_identifiers::{OpId, ResourceId};
use health_patient_engagement::requests::{RequestKind, RequestRefused, RequestState};
use health_patient_engagement_app::account_person;
use health_patient_engagement_app::requests::{
    Asked, PatientRequests, RequestError, Submission, view,
};
use health_persistence::{Precondition, RawJson, ResourceType, Store, Transaction, WriteRequest};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::SystemClock;
use serde_json::{Value, json};

const ACCOUNT: &str = "phone-account-1";

fn id(n: u32) -> String {
    format!("0190f1a2-7c3d-7e4f-8a5b-{n:012}")
}

async fn write(store: &SurrealStore, resource: Value) {
    let kind: ResourceType = resource["resourceType"]
        .as_str()
        .expect("type")
        .parse()
        .expect("type");
    let rid: ResourceId = resource["id"].as_str().expect("id").parse().expect("id");
    let mut tx = store.begin().await.expect("begin");
    tx.write(WriteRequest {
        resource_type: kind,
        id: rid,
        json: RawJson::from_validated(resource.to_string()),
        op_id: OpId::new(),
        precondition: Precondition::MustNotExist,
    })
    .await
    .expect("write");
    tx.commit().await.expect("commit");
}

async fn store(assurance: &str) -> SurrealStore {
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: "requests".into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    let store = SurrealStore::new(connection, Arc::new(SystemClock));
    write(
        &store,
        json!({
            "resourceType": "Person", "id": account_person(ACCOUNT).to_string(),
            "identifier": [{ "system": "urn:health:login-account", "value": ACCOUNT }],
            "link": [{ "target": { "reference": "Patient/p-1" }, "assurance": assurance }]
        }),
    )
    .await;
    for (n, patient) in [(1, "Patient/p-1"), (2, "Patient/p-2")] {
        write(&store, json!({
            "resourceType": "MedicationRequest", "id": id(n), "status": "active", "intent": "order",
            "meta": { "tag": [{ "system": "urn:health:facility", "code": "KAM013" }] },
            "medication": { "concept": { "text": "Amlodipine 5 mg" } }, "dosageInstruction": [{ "text": "Once daily" }],
            "subject": { "reference": patient }
        }))
        .await;
    }
    store
}

fn submission(n: u32, asked: Asked) -> Submission {
    Submission {
        id: id(n),
        patient: "Patient/p-1".into(),
        asked,
        note: Some("  please ".into()),
        at: "2026-11-06T08:00:00Z".into(),
    }
}

#[tokio::test]
async fn proposals_for_a_linked_patient_and_their_own_prescription() {
    let store = store("level3").await;
    let requests = PatientRequests::new(store.clone());
    assert_eq!(
        requests.linked(ACCOUNT).await.expect("linked"),
        ["Patient/p-1"]
    );
    assert_eq!(
        requests
            .access(ACCOUNT, "Patient/p-1")
            .await
            .expect("access"),
        PortalAccess::Records
    );
    assert_eq!(
        requests
            .access(ACCOUNT, "Patient/p-2")
            .await
            .expect("access"),
        PortalAccess::None
    );
    assert_eq!(
        requests.linked("someone-else").await.expect("linked"),
        Vec::<String>::new()
    );

    let refill = requests
        .submit(
            ACCOUNT,
            submission(
                11,
                Asked::Refill {
                    prior: format!("MedicationRequest/{}", id(1)),
                },
            ),
        )
        .await
        .expect("refill");
    let request = view(&refill).expect("a request");
    assert_eq!(
        (
            request.kind,
            request.state,
            request.title.as_str(),
            request.facility.as_str()
        ),
        (
            RequestKind::Refill,
            RequestState::Pending,
            "Amlodipine 5 mg",
            "KAM013"
        )
    );
    assert_eq!(refill["intent"], "proposal");
    assert_eq!(refill["note"][0]["text"], "please");
    assert_eq!(
        refill["priorPrescription"]["reference"],
        format!("MedicationRequest/{}", id(1))
    );
    assert_eq!(
        requests
            .submit(
                ACCOUNT,
                submission(
                    12,
                    Asked::Refill {
                        prior: format!("MedicationRequest/{}", id(2))
                    }
                )
            )
            .await,
        Err(RequestError::NotAPrescription(format!(
            "MedicationRequest/{}",
            id(2)
        ))),
        "another patient's prescription"
    );
    assert_eq!(
        requests
            .submit(
                ACCOUNT,
                submission(
                    11,
                    Asked::Refill {
                        prior: format!("MedicationRequest/{}", id(1))
                    }
                )
            )
            .await,
        Err(RequestError::Exists(id(11)))
    );
    let mut theirs = submission(
        13,
        Asked::Care {
            facility: "KAM013".into(),
            what: "Test".into(),
        },
    );
    theirs.patient = "Patient/p-2".into();
    assert_eq!(
        requests.submit(ACCOUNT, theirs).await,
        Err(RequestError::NotLinked)
    );
    assert_eq!(
        requests
            .submit(
                ACCOUNT,
                submission(
                    14,
                    Asked::Appointment {
                        facility: "KAM013".into(),
                        reason: " ".into(),
                        preferred: None
                    }
                )
            )
            .await,
        Err(RequestError::Refused(RequestRefused::Missing(
            "what it is for and where"
        )))
    );
    let appointment = requests
        .submit(
            ACCOUNT,
            submission(
                15,
                Asked::Appointment {
                    facility: "KAM013".into(),
                    reason: "Review".into(),
                    preferred: Some("2026-11-20".into()),
                },
            ),
        )
        .await
        .expect("appointment");
    assert_eq!(
        (
            appointment["status"].as_str(),
            appointment["requestedPeriod"][0]["start"].as_str()
        ),
        (Some("proposed"), Some("2026-11-20"))
    );
    assert!(
        view(&json!({ "resourceType": "MedicationRequest", "intent": "order" })).is_none(),
        "an order is no request"
    );
}

#[tokio::test]
async fn only_a_pending_request_is_withdrawn_declined_or_accepted() {
    let store = store("level1").await;
    let requests = PatientRequests::new(store.clone());
    assert_eq!(
        requests
            .access(ACCOUNT, "Patient/p-1")
            .await
            .expect("access"),
        PortalAccess::SelfSubmitted
    );
    let care = |n: u32| {
        submission(
            n,
            Asked::Care {
                facility: "KAM013".into(),
                what: "Blood sugar test".into(),
            },
        )
    };
    for n in [21, 22, 23] {
        requests.submit(ACCOUNT, care(n)).await.expect("submitted");
    }
    let reference = |n: u32| format!("ServiceRequest/{}", id(n));

    let withdrawn = requests
        .withdraw(ACCOUNT, &reference(21))
        .await
        .expect("withdrawn");
    assert_eq!(
        (
            withdrawn["status"].as_str(),
            withdrawn["statusReason"]["text"].as_str()
        ),
        (Some("revoked"), Some("withdrawn by the patient"))
    );
    assert_eq!(
        requests.withdraw(ACCOUNT, &reference(21)).await,
        Err(RequestError::Refused(RequestRefused::NotPending))
    );
    assert_eq!(
        requests.withdraw("someone-else", &reference(22)).await,
        Err(RequestError::NotFound(reference(22)))
    );

    assert_eq!(
        requests.decline(&reference(22), "KAM013", " ").await,
        Err(RequestError::Refused(RequestRefused::NoReason))
    );
    assert_eq!(
        requests.decline(&reference(22), "MBR001", "No").await,
        Err(RequestError::NotFound(reference(22)))
    );
    let declined = requests
        .decline(&reference(22), "KAM013", "Tested last week")
        .await
        .expect("declined");
    assert_eq!(declined["statusReason"]["text"], "Tested last week");

    let (pending, _) = requests
        .pending(&reference(23), "KAM013")
        .await
        .expect("pending");
    assert_eq!(pending.state, RequestState::Pending);
    let accepted = requests
        .accepted(&reference(23), "KAM013", |p| {
            p["note"] = json!([{ "text": "ordered" }])
        })
        .await
        .expect("accepted");
    assert_eq!(
        (
            accepted["status"].as_str(),
            accepted["note"][0]["text"].as_str()
        ),
        (Some("completed"), Some("ordered"))
    );
    assert!(matches!(
        requests.pending(&reference(23), "KAM013").await,
        Err(RequestError::Refused(RequestRefused::NotPending))
    ));
    assert_eq!(
        requests
            .pending("ServiceRequest/nope", "KAM013")
            .await
            .map(|_| ()),
        Err(RequestError::NotFound("ServiceRequest/nope".into()))
    );
}

//! Care plans over SurrealDB 3.3.5 in memory (#110): a plan drafted, activated and linked to the
//! patient's own Condition and requests, as a valid R5 CarePlan; another patient's or another
//! facility's records are refused.

use std::sync::Arc;

use health_care_plans::{Draft, PlanRefused, PlanStatus};
use health_care_plans_app::{CarePlanError, CarePlans, care_plan, fhir};
use health_fhir_validation::validate_structure;
use health_identifiers::{OpId, ResourceId};
use health_persistence::{Precondition, RawJson, ResourceType, Store, Transaction, WriteRequest};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::{SystemClock, Timestamp};
use serde_json::{Value, json};

const PLAN: &str = "0190f1a2-7c3d-7e4f-8a5b-000000000f01";

fn at(text: &str) -> Timestamp {
    text.parse().expect("timestamp")
}

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

fn tagged(mut resource: Value, facility: &str) -> Value {
    resource["meta"] = json!({ "tag": [{ "system": "urn:health:facility", "code": facility }] });
    resource
}

async fn store() -> SurrealStore {
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: "care-plans".into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    let store = SurrealStore::new(connection, Arc::new(SystemClock));
    write(&store, tagged(json!({ "resourceType": "Condition", "id": id(1), "subject": { "reference": "Patient/p-1" }, "code": { "text": "Hypertension" } }), "KAM013")).await;
    write(&store, tagged(json!({ "resourceType": "MedicationRequest", "id": id(2), "status": "active", "intent": "order", "subject": { "reference": "Patient/p-1" }, "medication": { "concept": { "text": "Amlodipine 5 mg" } } }), "KAM013")).await;
    write(&store, tagged(json!({ "resourceType": "MedicationRequest", "id": id(3), "status": "active", "intent": "order", "subject": { "reference": "Patient/p-2" }, "medication": { "concept": { "text": "Iron" } } }), "KAM013")).await;
    write(&store, tagged(json!({ "resourceType": "Condition", "id": id(4), "subject": { "reference": "Patient/p-1" }, "code": { "text": "Asthma" } }), "MBR001")).await;
    store
}

fn request() -> Draft {
    Draft {
        id: PLAN.into(),
        patient: "Patient/p-1".into(),
        facility: "KAM013".into(),
        title: "Hypertension follow-up".into(),
        description: Some("Blood pressure under 140/90 within three months".into()),
        author: "clinician-1".into(),
        at: at("2026-11-05T10:00:00Z"),
    }
}

#[tokio::test]
async fn a_plan_links_the_patients_own_condition_and_requests() {
    let store = store().await;
    let plans = CarePlans::new(store.clone());
    plans.draft(request()).await.expect("drafted");
    assert_eq!(
        plans.draft(request()).await,
        Err(CarePlanError::Exists(PLAN.into()))
    );
    plans
        .transition(PLAN, PlanStatus::Active, at("2026-11-05T10:05:00Z"))
        .await
        .expect("active");
    plans
        .address(PLAN, &format!("Condition/{}", id(1)))
        .await
        .expect("addressed");
    let plan = plans
        .plan(PLAN, &format!("MedicationRequest/{}", id(2)))
        .await
        .expect("planned");
    assert_eq!(plan.activities, [format!("MedicationRequest/{}", id(2))]);

    let kind: ResourceType = "CarePlan".parse().expect("type");
    let mut tx = store.begin().await.expect("begin");
    let head = tx
        .read(&kind, PLAN.parse().expect("id"))
        .await
        .expect("read")
        .expect("written");
    let issues = validate_structure(head.json.as_str());
    assert!(issues.is_empty(), "{issues:?}");
    let stored: Value = serde_json::from_str(head.json.as_str()).expect("json");
    assert_eq!(
        stored["activity"][0]["plannedActivityReference"]["reference"],
        format!("MedicationRequest/{}", id(2))
    );
    assert_eq!(
        stored["addresses"][0]["reference"]["reference"],
        format!("Condition/{}", id(1))
    );
    assert_eq!(care_plan(&stored).as_ref(), Some(&plan));
    assert_eq!(fhir(&plan)["intent"], "plan");
    assert_eq!(plans.care_plan(PLAN).await.expect("plan"), plan);

    // Another patient's request, another facility's condition, or nothing at all: refused.
    for refused in [
        format!("MedicationRequest/{}", id(3)),
        format!("MedicationRequest/{}", id(99)),
    ] {
        assert_eq!(
            plans.plan(PLAN, &refused).await,
            Err(CarePlanError::NotThePatients(refused.clone()))
        );
    }
    let elsewhere = format!("Condition/{}", id(4));
    assert_eq!(
        plans.address(PLAN, &elsewhere).await,
        Err(CarePlanError::NotThePatients(elsewhere.clone()))
    );
    assert_eq!(
        plans.plan(PLAN, "Observation/x").await,
        Err(CarePlanError::Refused(PlanRefused::NotARequest(
            "Observation/x".into()
        )))
    );

    let done = plans
        .transition(PLAN, PlanStatus::Completed, at("2027-02-01T00:00:00Z"))
        .await
        .expect("completed");
    assert_eq!(done.ended, Some(at("2027-02-01T00:00:00Z")));
    assert_eq!(
        plans
            .plan(PLAN, &format!("MedicationRequest/{}", id(2)))
            .await,
        Err(CarePlanError::Refused(PlanRefused::Ended))
    );
    let unknown = id(98);
    assert_eq!(
        plans.care_plan(&unknown).await,
        Err(CarePlanError::NotFound(unknown.clone()))
    );
}

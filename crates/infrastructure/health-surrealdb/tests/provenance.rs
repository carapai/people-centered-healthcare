//! Provenance in one table, each row owned by its target's Owner (#196; resource-ownership.md
//! rule 7): the owner is stamped from the target, read in the same transaction.

use std::sync::Arc;

use health_identifiers::{OpId, ResourceId};
use health_persistence::{Precondition, RawJson, Store, Transaction, WriteRequest};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::SystemClock;
use serde_json::{Value, json};

async fn store() -> SurrealStore {
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: "provenance".into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    SurrealStore::new(connection, Arc::new(SystemClock))
}

fn request(resource: &str, id: ResourceId, json: &Value) -> WriteRequest {
    WriteRequest {
        resource_type: resource.parse().expect("type"),
        id,
        json: RawJson::from_validated(json.to_string()),
        op_id: OpId::new(),
        precondition: Precondition::MustNotExist,
    }
}

fn provenance(target: &str) -> Value {
    json!({ "resourceType": "Provenance", "target": [{ "reference": target }], "recorded": "2026-11-01T00:00:00Z", "agent": [{ "who": { "display": "registry" } }] })
}

/// The Owner the store gave Provenance `id`, by the owner query sync uses.
async fn owner(store: &SurrealStore, id: ResourceId) -> Option<String> {
    let mut tx = store.begin().await.expect("tx");
    for owner in ["client-registry", "laboratory", "clinical-data"] {
        if tx
            .provenance_of_owner(owner)
            .await
            .expect("query")
            .contains(&id.to_string())
        {
            return Some(owner.to_owned());
        }
    }
    None
}

#[tokio::test]
async fn a_provenance_is_owned_by_its_targets_owner() {
    let store = store().await;
    let (patient, lab, prov_patient, prov_lab) = (
        ResourceId::new(),
        ResourceId::new(),
        ResourceId::new(),
        ResourceId::new(),
    );
    let mut tx = store.begin().await.expect("tx");
    tx.write(request(
        "Patient",
        patient,
        &json!({ "resourceType": "Patient" }),
    ))
    .await
    .expect("patient");
    let lab_json = json!({ "resourceType": "Observation", "status": "final", "code": { "text": "Hb" },
        "category": [{ "coding": [{ "system": "http://terminology.hl7.org/CodeSystem/observation-category", "code": "laboratory" }] }] });
    tx.write(request("Observation", lab, &lab_json))
        .await
        .expect("lab");
    // Written in the same transaction as their targets.
    tx.write(request(
        "Provenance",
        prov_patient,
        &provenance(&format!("Patient/{patient}")),
    ))
    .await
    .expect("provenance");
    tx.write(request(
        "Provenance",
        prov_lab,
        &provenance(&format!("Observation/{lab}/_history/1")),
    ))
    .await
    .expect("provenance");
    tx.commit().await.expect("commit");
    assert_eq!(
        owner(&store, prov_patient).await.as_deref(),
        Some("client-registry")
    );
    assert_eq!(
        owner(&store, prov_lab).await.as_deref(),
        Some("laboratory"),
        "the target's category decides"
    );
}

#[tokio::test]
async fn a_provenance_without_a_stored_target_is_refused_and_one_rolled_back_is_gone() {
    let store = store().await;
    let mut tx = store.begin().await.expect("tx");
    for json in [
        provenance(&format!("Patient/{}", ResourceId::new())),
        provenance("Basic/whatever"),
        json!({ "resourceType": "Provenance", "recorded": "2026-11-01T00:00:00Z" }),
    ] {
        assert!(
            tx.write(request("Provenance", ResourceId::new(), &json))
                .await
                .is_err(),
            "{json}"
        );
    }
    drop(tx);

    // A Provenance commits only with its target's write: rolled back together.
    let (patient, prov) = (ResourceId::new(), ResourceId::new());
    let mut tx = store.begin().await.expect("tx");
    tx.write(request(
        "Patient",
        patient,
        &json!({ "resourceType": "Patient" }),
    ))
    .await
    .expect("patient");
    tx.write(request(
        "Provenance",
        prov,
        &provenance(&format!("Patient/{patient}")),
    ))
    .await
    .expect("provenance");
    drop(tx);
    assert_eq!(owner(&store, prov).await, None);
}

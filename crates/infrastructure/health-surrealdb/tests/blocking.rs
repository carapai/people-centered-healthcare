//! The blocking index: rows written with each live version, replaced by the next, removed by a
//! tombstone, and counted in the background (ADR 0055).

use std::sync::Arc;

use health_fhir_search::Registry;
use health_identifiers::{OpId, ResourceId};
use health_persistence::{
    BlockingIndex, DerivedKeys, Precondition, RawJson, Store, Transaction, WriteRequest,
};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::SystemClock;
use serde_json::{Value, json};

/// Blocks Patients by family name and district.
struct ByFamily;

impl DerivedKeys for ByFamily {
    fn keys(&self, resource_type: &str, resource: &Value) -> Vec<(String, String)> {
        if resource_type != "Patient" {
            return Vec::new();
        }
        let mut keys = Vec::new();
        if let Some(family) = resource["name"][0]["family"].as_str() {
            keys.push(("family".to_owned(), family.to_uppercase()));
        }
        if let Some(district) = resource["address"][0]["district"].as_str() {
            keys.push(("district".to_owned(), district.to_uppercase()));
        }
        keys
    }
}

async fn store() -> SurrealStore {
    let settings = Settings {
        target: Target::Memory,
        namespace: "blocking".into(),
        database: "main".into(),
    };
    let connection = Connection::open(&settings).await.expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    SurrealStore::new(connection, Arc::new(SystemClock))
        .with_registry(Registry::new(jiff::tz::Offset::UTC))
        .with_blocking(Arc::new(ByFamily))
}

async fn write(
    store: &SurrealStore,
    id: ResourceId,
    json: Value,
    precondition: Precondition,
) -> OpId {
    let mut tx = store.begin().await.expect("tx");
    let version = tx
        .write(WriteRequest {
            resource_type: "Patient".parse().expect("type"),
            id,
            json: RawJson::from_validated(json.to_string()),
            op_id: OpId::new(),
            precondition,
        })
        .await
        .expect("write");
    tx.commit().await.expect("commit");
    version.version_id
}

async fn members(store: &SurrealStore, kind: &str, value: &str) -> Vec<ResourceId> {
    let mut tx = store.begin().await.expect("tx");
    tx.members(kind, value, 100).await.expect("members")
}

#[tokio::test]
async fn blocks_follow_the_current_version() {
    let store = store().await;
    let (a, b) = (ResourceId::new(), ResourceId::new());
    let patient = |family: &str| json!({ "resourceType": "Patient", "name": [{ "family": family }], "address": [{ "district": "Gulu" }] });
    let va = write(&store, a, patient("Okello"), Precondition::MustNotExist).await;
    write(&store, b, patient("Okello"), Precondition::MustNotExist).await;
    let mut both = vec![a, b];
    both.sort();
    assert_eq!(members(&store, "family", "OKELLO").await, both);
    assert_eq!(members(&store, "district", "GULU").await.len(), 2);

    // A new version replaces its keys.
    write(&store, a, patient("Akello"), Precondition::IfMatch(va)).await;
    assert_eq!(members(&store, "family", "OKELLO").await, vec![b]);
    assert_eq!(members(&store, "family", "AKELLO").await, vec![a]);

    // Counts are only what the last recount saw.
    let mut tx = store.begin().await.expect("tx");
    assert_eq!(
        tx.counts(&[("family".into(), "OKELLO".into())])
            .await
            .expect("counts"),
        vec![None]
    );
    drop(tx);
    assert_eq!(store.recount_blocks().await.expect("recount"), 3);
    let mut tx = store.begin().await.expect("tx");
    let counted = tx
        .counts(&[
            ("family".into(), "OKELLO".into()),
            ("district".into(), "GULU".into()),
            ("family".into(), "NOBODY".into()),
        ])
        .await
        .expect("counts");
    assert_eq!(counted, vec![Some(1), Some(2), None]);
    drop(tx);

    // A tombstone leaves every block.
    let mut tx = store.begin().await.expect("tx");
    tx.delete(&"Patient".parse().expect("type"), b, OpId::new(), None)
        .await
        .expect("delete");
    tx.commit().await.expect("commit");
    assert!(members(&store, "family", "OKELLO").await.is_empty());
    assert_eq!(members(&store, "district", "GULU").await, vec![a]);

    // Other types have no blocks.
    let mut tx = store.begin().await.expect("tx");
    tx.write(WriteRequest {
        resource_type: "Organization".parse().expect("type"),
        id: ResourceId::new(),
        json: RawJson::from_validated(
            json!({ "resourceType": "Organization", "name": "Okello" }).to_string(),
        ),
        op_id: OpId::new(),
        precondition: Precondition::MustNotExist,
    })
    .await
    .expect("write");
    tx.commit().await.expect("commit");
    assert_eq!(store.recount_blocks().await.expect("recount"), 2);
}

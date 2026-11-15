//! Equipment as a FHIR Device (ADR 0064): a valid Device that reads back the same, owned by the
//! equipment module, and registered once per unit over SurrealDB (memory engine).

use std::sync::Arc;

use health_equipment::{Equipment, Status};
use health_equipment_app::{RegisterError, device, equipment, register};
use health_fhir_validation::validate_structure;
use health_persistence::{ResourceType, Store, Transaction};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::SystemClock;

fn bp(serial: &str) -> Equipment {
    Equipment::received(
        "bp-machine",
        "BP machine, digital",
        serial,
        "KLA01",
        "Location/opd",
        "grn-7",
    )
    .expect("valid")
}

#[test]
fn a_unit_is_a_valid_device_and_reads_back_the_same() {
    let unit = bp("BP-0042");
    let resource = device(&unit);
    assert_eq!(
        validate_structure(&resource.to_string()),
        vec![],
        "{resource}"
    );
    assert_eq!(equipment(&resource), Some(unit.clone()));
    assert_eq!(
        health_surrealdb::ownership::owner_of("Device", &resource),
        Some("equipment")
    );
    let retired = Equipment {
        status: Status::Retired,
        received: None,
        ..unit
    };
    let resource = device(&retired);
    assert_eq!(resource["status"], "inactive");
    assert_eq!(equipment(&resource), Some(retired));
    let implant = serde_json::json!({
        "resourceType": "Device",
        "category": [{"coding": [{"system": "urn:health:device-category", "code": "patient-assigned"}]}]
    });
    assert_eq!(equipment(&implant), None, "a patient-assigned Device");
}

#[tokio::test]
async fn a_unit_is_registered_once() {
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: "equipment".into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    let store = SurrealStore::new(connection, Arc::new(SystemClock));
    let unit = bp("BP-0042");

    let mut tx = store.begin().await.expect("begin");
    register(&mut tx, &unit).await.expect("registered");
    tx.commit().await.expect("commit");

    let mut tx = store.begin().await.expect("begin");
    let devices: ResourceType = "Device".parse().expect("type");
    let id = unit.id.to_string().parse().expect("id");
    let head = tx.read(&devices, id).await.expect("read").expect("there");
    let stored: serde_json::Value = serde_json::from_str(head.json.as_str()).expect("json");
    assert_eq!(equipment(&stored), Some(unit.clone()));
    assert_eq!(
        register(&mut tx, &unit).await,
        Err(RegisterError::Registered {
            product: "bp-machine".into(),
            serial: "BP-0042".into()
        })
    );
}

//! Equipment as a FHIR Device (ADR 0064): a valid Device that reads back the same, owned by the
//! equipment module, and registered once per unit over SurrealDB (memory engine).

use std::sync::Arc;

use health_equipment::{Change, Equipment, EquipmentRefused, Status};
use health_equipment_app::{
    EquipmentError, Equipments, RegisterError, device, equipment, equipment_postings, handed_over,
    register,
};
use health_fhir_validation::validate_structure;
use health_financial_ledger::{Currency, EQUIPMENT, EQUIPMENT_WRITE_OFF, PAYABLE, Side};
use health_persistence::{ResourceType, Store, Transaction};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::{SystemClock, Timestamp};
use rust_decimal::Decimal;

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
        change: Some(Change {
            note: "beyond repair".into(),
            by: "tech-1".into(),
            at: Timestamp::from_second(1_800_000_000).expect("time"),
        }),
        ..unit
    };
    let resource = device(&retired);
    assert_eq!(resource["status"], "inactive");
    assert_eq!(equipment(&resource), Some(retired));
    let mut dispatched = bp("BP-0043");
    dispatched
        .dispatch(
            "GUL02",
            "store-1",
            Timestamp::from_second(1_800_000_000).expect("time"),
        )
        .expect("dispatched");
    let resource = device(&dispatched);
    assert_eq!(
        validate_structure(&resource.to_string()),
        vec![],
        "{resource}"
    );
    assert_eq!(equipment(&resource), Some(dispatched.clone()));
    let handed = handed_over(&resource).expect("handed over");
    assert_eq!(handed["meta"]["tag"][0]["code"], "GUL02");
    assert_eq!(handed_over(&handed), None, "handed over once");
    assert_eq!(handed_over(&device(&bp("BP-0044"))), None, "not in transit");
    let implant = serde_json::json!({
        "resourceType": "Device",
        "category": [{"coding": [{"system": "urn:health:device-category", "code": "patient-assigned"}]}]
    });
    assert_eq!(equipment(&implant), None, "a patient-assigned Device");
}

async fn store() -> SurrealStore {
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: "equipment".into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    SurrealStore::new(connection, Arc::new(SystemClock))
}

#[tokio::test]
async fn a_unit_is_registered_once() {
    let store = store().await;
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

#[tokio::test]
async fn its_facility_changes_a_unit_and_each_change_is_a_version() {
    let store = store().await;
    let unit = bp("BP-0042");
    let mut tx = store.begin().await.expect("begin");
    register(&mut tx, &unit).await.expect("registered");
    tx.commit().await.expect("commit");
    let id = unit.id.to_string().parse().expect("id");
    let equipments = Equipments::new(store.clone());
    let at = Timestamp::from_second(1_800_000_000).expect("time");

    let faulty = equipments
        .change(id, "KLA01", |u| {
            u.set_status(Status::Faulty, "cuff leaks", "nurse-1", at)
        })
        .await
        .expect("changed");
    assert_eq!(faulty.status, Status::Faulty);
    let moved = equipments
        .change(id, "KLA01", |u| {
            u.move_to("Location/workshop", "tech-1", at)
        })
        .await
        .expect("moved");
    assert_eq!(equipments.get(id).await.expect("read"), Some(moved.clone()));
    assert_eq!(moved.status, Status::Faulty);
    assert_eq!(moved.location, "Location/workshop");

    assert_eq!(
        equipments
            .change(id, "GUL02", |u| u.set_status(
                Status::Available,
                "",
                "x",
                at
            ))
            .await,
        Err(EquipmentError::OtherFacility("BP-0042".into()))
    );
    assert_eq!(
        equipments
            .change(id, "KLA01", |u| u.set_status(
                Status::Faulty,
                "again",
                "x",
                at
            ))
            .await,
        Err(EquipmentError::Refused(EquipmentRefused::Unchanged(
            "BP-0042".into(),
            "faulty".into()
        )))
    );
    assert!(matches!(
        equipments
            .change(health_identifiers::ResourceId::new(), "KLA01", |_| Ok(()))
            .await,
        Err(EquipmentError::NotFound(_))
    ));
    // Registered, made faulty, moved: three versions, the maintenance history.
    let mut tx = store.begin().await.expect("begin");
    let devices: ResourceType = "Device".parse().expect("type");
    let head = tx.read(&devices, id).await.expect("read").expect("there");
    assert_eq!(head.version.version_seq, 3);
}

#[tokio::test]
async fn central_hands_a_dispatched_unit_to_the_facility_that_receives_it() {
    let store = store().await;
    let unit = bp("BP-0050");
    let mut tx = store.begin().await.expect("begin");
    register(&mut tx, &unit).await.expect("registered");
    tx.commit().await.expect("commit");
    let id = unit.id.to_string().parse().expect("id");
    let equipments = Equipments::new(store.clone());
    let at = Timestamp::from_second(1_800_000_000).expect("time");

    assert_eq!(
        equipments.dispatch(id, "GUL02", "MBR03", "x", at).await,
        Err(EquipmentError::OtherFacility("BP-0050".into()))
    );
    let arriving = equipments
        .dispatch(id, "KLA01", "GUL02", "store-1", at)
        .await
        .expect("dispatched");
    assert_eq!(arriving.facility, "GUL02");
    assert!(arriving.in_transit());
    // The sender has let it go; the receiver changes it only once received.
    assert_eq!(
        equipments
            .change(id, "KLA01", |u| u.move_to("Location/ward", "x", at))
            .await,
        Err(EquipmentError::OtherFacility("BP-0050".into()))
    );
    assert_eq!(
        equipments
            .change(id, "GUL02", |u| u.set_status(
                Status::Faulty,
                "bent",
                "x",
                at
            ))
            .await,
        Err(EquipmentError::Refused(EquipmentRefused::InTransit(
            "BP-0050".into(),
            "GUL02".into()
        )))
    );
    let received = equipments
        .change(id, "GUL02", |u| u.receive("Location/lab", "store-2", at))
        .await
        .expect("received");
    assert_eq!(received.location, "Location/lab");
    assert!(!received.in_transit());
    // Registered, dispatched, handed over, received.
    let mut tx = store.begin().await.expect("begin");
    let devices: ResourceType = "Device".parse().expect("type");
    let head = tx.read(&devices, id).await.expect("read").expect("there");
    assert_eq!(head.version.version_seq, 4);
}

#[test]
fn a_unit_keeps_its_cost_and_posts_its_receipt_retirement_and_hand_over_at_cost() {
    let at = Timestamp::from_second(1_800_000_000).expect("time");
    let unit = bp("BP-0060").with_acquisition(Decimal::new(125_000_070, 2), at);
    let resource = device(&unit);
    assert_eq!(
        validate_structure(&resource.to_string()),
        vec![],
        "{resource}"
    );
    assert_eq!(
        equipment(&resource),
        Some(unit.clone()),
        "the cost reads back exactly"
    );

    let ugx = Currency::from_code("UGX").expect("currency");
    let accounts = |posting: &health_financial_ledger::Posting| {
        posting
            .lines
            .iter()
            .map(|l| (l.account.clone(), l.side, l.dimensions.facility.clone()))
            .collect::<Vec<_>>()
    };
    let received = equipment_postings(None, &unit, ugx, at).expect("received");
    assert_eq!(
        accounts(&received),
        vec![
            (EQUIPMENT.to_owned(), Side::Debit, "KLA01".to_owned()),
            (PAYABLE.to_owned(), Side::Credit, "KLA01".to_owned()),
        ]
    );
    assert_eq!(
        received.lines[0].amount.amount(),
        Decimal::new(1_250_001, 0),
        "UGX has no cents"
    );

    // Moving it within the facility moves no value.
    let mut moved = unit.clone();
    moved.move_to("Location/ward", "x", at).expect("moved");
    assert_eq!(equipment_postings(Some(&unit), &moved, ugx, at), None);

    let mut sent = unit.clone();
    sent.dispatch("GUL02", "x", at).expect("dispatched");
    assert_eq!(
        equipment_postings(Some(&unit), &sent, ugx, at),
        None,
        "not yet handed over"
    );
    let mut handed = sent.clone();
    handed.hand_over();
    assert_eq!(
        accounts(&equipment_postings(Some(&sent), &handed, ugx, at).expect("handed over")),
        vec![
            (EQUIPMENT.to_owned(), Side::Debit, "GUL02".to_owned()),
            (EQUIPMENT.to_owned(), Side::Credit, "KLA01".to_owned()),
        ]
    );

    let mut retired = unit.clone();
    retired
        .set_status(Status::Retired, "beyond repair", "x", at)
        .expect("retired");
    assert_eq!(
        accounts(&equipment_postings(Some(&unit), &retired, ugx, at).expect("retired")),
        vec![
            (
                EQUIPMENT_WRITE_OFF.to_owned(),
                Side::Debit,
                "KLA01".to_owned()
            ),
            (EQUIPMENT.to_owned(), Side::Credit, "KLA01".to_owned()),
        ]
    );
    // A unit whose cost is not known posts nothing.
    assert_eq!(equipment_postings(None, &bp("BP-0061"), ugx, at), None);
}

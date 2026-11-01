//! Receiving, requisitions and reorder over SurrealDB (memory engine).

use std::sync::Arc;

use health_inventory::receiving::{
    GoodsReceived, ReceivedLine, ReorderPolicy, RequestedLine, Requisition, RequisitionStatus,
};
use health_inventory::transfers::{TransferLine, TransferStatus};
use health_inventory::{
    Movement, MovementKind, Origin, Ownership, Policy, Status, StockStore, StockUnit,
};
use health_inventory_app::{Inventory, InventoryError, Sender, supply_requests};
use health_persistence::Store;
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::{SignedDuration, SystemClock, Timestamp};
use rust_decimal::Decimal;

async fn setup() -> (Inventory<SurrealStore>, SurrealStore) {
    let settings = Settings {
        target: Target::Memory,
        namespace: "receiving".into(),
        database: "main".into(),
    };
    let connection = Connection::open(&settings).await.expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    let store = SurrealStore::new(connection, Arc::new(SystemClock));
    (Inventory::new(store.clone(), Policy::default()), store)
}

fn now() -> Timestamp {
    Timestamp::from_second(1_800_000_000).expect("time")
}

fn sender() -> Sender {
    Sender {
        actor: "pharm-1".into(),
        node: "E1".into(),
    }
}

fn unit(store: &str) -> StockUnit {
    StockUnit {
        store: store.into(),
        product: "amoxicillin-500".into(),
        batch: "B42".into(),
        expiry: "2027-06-30".into(),
        funding_source: "government".into(),
        ownership: Ownership::Owned,
        status: Status::Usable,
    }
}

fn note() -> GoodsReceived {
    GoodsReceived {
        id: "grn-1".into(),
        store: "Location/pharmacy".into(),
        supplier: "NMS".into(),
        delivery: "INV-2026-0042".into(),
        lines: vec![ReceivedLine {
            product: "amoxicillin-500".into(),
            batch: "B42".into(),
            expiry: "2027-06-30".into(),
            funding_source: "government".into(),
            ownership: Ownership::Owned,
            quantity: 1000,
            pack_size: 100,
            unit_cost: Decimal::new(1_50, 2),
        }],
    }
}

#[tokio::test]
async fn a_receipt_increases_the_balance_with_its_cost_recorded() {
    let (inventory, store) = setup().await;
    inventory
        .receive_goods(&note(), &sender(), now())
        .await
        .expect("received");
    let key = unit("Location/pharmacy").key();
    assert_eq!(
        inventory.balance(&key).await.expect("balance").on_hand,
        1000
    );
    let mut tx = store.begin().await.expect("tx");
    let movements = tx.movements(&key).await.expect("movements");
    assert_eq!(movements.len(), 1);
    assert_eq!(movements[0].kind, MovementKind::Receipt);
    assert_eq!(movements[0].unit_cost, Some(Decimal::new(1_50, 2)));
    assert_eq!(movements[0].pack_size, 100);
    assert_eq!(movements[0].source_document, "GoodsReceived/grn-1");
    drop(tx);
    // A note is received once.
    assert!(matches!(
        inventory.receive_goods(&note(), &sender(), now()).await,
        Err(InventoryError::Receiving(_))
    ));
    assert_eq!(
        inventory.balance(&key).await.expect("balance").on_hand,
        1000
    );
    let mut empty = note();
    empty.id = "grn-2".into();
    empty.lines.clear();
    assert!(matches!(
        inventory.receive_goods(&empty, &sender(), now()).await,
        Err(InventoryError::Receiving(_))
    ));
}

#[tokio::test]
async fn a_ward_requisition_is_approved_and_fulfilled_by_a_transfer() {
    let (inventory, _) = setup().await;
    inventory
        .receive_goods(&note(), &sender(), now())
        .await
        .expect("received");
    let requisition = Requisition::new(
        "req-1",
        "Location/ward",
        "Location/pharmacy",
        vec![RequestedLine {
            product: "amoxicillin-500".into(),
            quantity: 200,
        }],
        "nurse-1",
    )
    .expect("requisition");
    inventory.request(&requisition).await.expect("requested");
    let fhir = supply_requests(&requisition);
    assert_eq!(fhir[0]["resourceType"], "SupplyRequest");
    assert_eq!(fhir[0]["deliverTo"]["reference"], "Location/ward");
    let line = TransferLine {
        product: "amoxicillin-500".into(),
        batch: "B42".into(),
        expiry: "2027-06-30".into(),
        funding_source: "government".into(),
        ownership: Ownership::Owned,
        quantity: 200,
    };
    // Not before it is approved.
    assert!(matches!(
        inventory
            .fulfil_requisition("req-1", "t-req-1", vec![line.clone()], &sender(), now())
            .await,
        Err(InventoryError::Receiving(_))
    ));
    inventory
        .decide_requisition("req-1", true, "pharm-1")
        .await
        .expect("approved");
    let (fulfilled, transfer) = inventory
        .fulfil_requisition("req-1", "t-req-1", vec![line], &sender(), now())
        .await
        .expect("fulfilled");
    assert_eq!(fulfilled.status, RequisitionStatus::Fulfilled);
    assert_eq!(transfer.status, TransferStatus::InTransit);
    assert_eq!(
        inventory
            .requisition("req-1")
            .await
            .expect("req")
            .transfer
            .as_deref(),
        Some("t-req-1")
    );
    assert_eq!(
        inventory
            .balance(&unit("Location/pharmacy").key())
            .await
            .expect("b")
            .on_hand,
        800
    );
}

#[tokio::test]
async fn reorder_suggestions_follow_the_last_months_consumption() {
    let (inventory, _) = setup().await;
    inventory
        .receive_goods(
            &note(),
            &sender(),
            now() - SignedDuration::from_hours(24 * 60),
        )
        .await
        .expect("received");
    // 300 issued in the last 30 days, and 500 before then (not counted).
    for (id, quantity, days_ago) in [("old", -500, 45), ("a", -100, 20), ("b", -200, 5)] {
        let at = now() - SignedDuration::from_hours(24 * days_ago);
        let movement = Movement {
            id: id.into(),
            unit: unit("Location/pharmacy"),
            kind: MovementKind::Issue,
            quantity,
            pack_size: 1,
            unit_cost: None,
            occurred_at: at,
            recorded_at: at,
            source_document: "doc".into(),
            actor: "u1".into(),
            node: "E1".into(),
            op_id: id.into(),
            reason: None,
            reverses: None,
            emergency: false,
        };
        inventory
            .record(
                &movement,
                Origin::Authority {
                    may_emergency_dispense: false,
                },
            )
            .await
            .expect("issued");
    }
    let suggestions = inventory
        .reorder("Location/pharmacy", ReorderPolicy::default(), 30, now())
        .await
        .expect("suggestions");
    assert_eq!(suggestions.len(), 1);
    let s = &suggestions[0];
    assert_eq!((s.on_hand, s.min, s.max, s.order), (200, 300, 900, 700));
}

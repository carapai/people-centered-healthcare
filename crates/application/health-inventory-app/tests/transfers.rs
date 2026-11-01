//! Transfers and Reconciliation Records over SurrealDB (memory engine): each step one
//! transaction, discrepancies raising records, and a transfer closing once they are resolved.

use std::sync::Arc;

use health_inventory::reconciliation::{Discrepancy, RootCause};
use health_inventory::transfers::{Counted, Transfer, TransferLine, TransferStatus};
use health_inventory::{
    Movement, MovementKind, Origin, Ownership, Policy, Refused, Status, StockUnit,
};
use health_inventory_app::{Inventory, InventoryError, Sender};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::{SystemClock, Timestamp};

const AUTHORITY: Origin = Origin::Authority {
    may_emergency_dispense: false,
};

async fn inventory() -> Inventory<SurrealStore> {
    let settings = Settings {
        target: Target::Memory,
        namespace: "transfers".into(),
        database: "main".into(),
    };
    let connection = Connection::open(&settings).await.expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    Inventory::new(
        SurrealStore::new(connection, Arc::new(SystemClock)),
        Policy::default(),
    )
}

fn at() -> Timestamp {
    Timestamp::from_second(1_800_000_000).expect("time")
}

fn unit(store: &str, status: Status) -> StockUnit {
    StockUnit {
        store: store.into(),
        product: "amoxicillin-500".into(),
        batch: "B42".into(),
        expiry: "2027-06-30".into(),
        funding_source: "government".into(),
        ownership: Ownership::Owned,
        status,
    }
}

fn movement(id: &str, kind: MovementKind, quantity: i64) -> Movement {
    Movement {
        id: id.into(),
        unit: unit("Location/pharmacy", Status::Usable),
        kind,
        quantity,
        pack_size: 10,
        unit_cost: None,
        occurred_at: at(),
        recorded_at: at(),
        source_document: "doc".into(),
        actor: "u1".into(),
        node: "E1".into(),
        op_id: id.into(),
        reason: None,
        reverses: None,
        emergency: false,
    }
}

fn line(quantity: i64) -> TransferLine {
    TransferLine {
        product: "amoxicillin-500".into(),
        batch: "B42".into(),
        expiry: "2027-06-30".into(),
        funding_source: "government".into(),
        ownership: Ownership::Owned,
        quantity,
    }
}

fn sender(actor: &str) -> Sender {
    Sender {
        actor: actor.into(),
        node: "E1".into(),
    }
}

async fn on_hand(inventory: &Inventory<SurrealStore>, store: &str, status: Status) -> i64 {
    inventory
        .balance(&unit(store, status).key())
        .await
        .expect("balance")
        .on_hand
}

#[tokio::test]
async fn a_short_and_damaged_receipt_reconciles_and_then_closes() {
    let inventory = inventory().await;
    inventory
        .record(&movement("receipt", MovementKind::Receipt, 10), AUTHORITY)
        .await
        .expect("receipt");
    let transfer = Transfer::new(
        "t1",
        "Location/pharmacy",
        "Location/ward",
        None,
        vec![line(6)],
    )
    .expect("transfer");
    let sent = inventory
        .dispatch(transfer, &sender("u1"), at())
        .await
        .expect("dispatch");
    assert_eq!(sent.status, TransferStatus::InTransit);
    assert_eq!(
        on_hand(&inventory, "Location/pharmacy", Status::Usable).await,
        4
    );
    assert_eq!(
        on_hand(&inventory, "Location/ward", Status::Usable).await,
        0
    );

    let (received, records) = inventory
        .receive(
            "t1",
            vec![Counted {
                line: 0,
                good: 4,
                damaged: 1,
            }],
            &sender("u2"),
            at(),
        )
        .await
        .expect("receive");
    assert_eq!(received.status, TransferStatus::Reconciling);
    assert_eq!(
        on_hand(&inventory, "Location/ward", Status::Usable).await,
        4
    );
    assert_eq!(
        on_hand(&inventory, "Location/ward", Status::Quarantine).await,
        1
    );
    assert_eq!(
        records
            .iter()
            .map(|r| (r.discrepancy, r.quantity))
            .collect::<Vec<_>>(),
        [(Discrepancy::Damaged, -1), (Discrepancy::Short, -1)]
    );
    assert_eq!(
        inventory.open_reconciliations().await.expect("open").len(),
        2
    );

    inventory
        .resolve(
            &records[0].id,
            RootCause::TransitLoss,
            "carton crushed",
            "u2",
            at(),
        )
        .await
        .expect("resolved");
    assert_eq!(
        inventory.transfer("t1").await.expect("t1").status,
        TransferStatus::Reconciling
    );
    inventory
        .resolve(
            &records[1].id,
            RootCause::RecordingError,
            "picked 5, not 6",
            "u1",
            at(),
        )
        .await
        .expect("resolved");
    assert_eq!(
        inventory.transfer("t1").await.expect("t1").status,
        TransferStatus::Closed
    );
    assert!(
        inventory
            .open_reconciliations()
            .await
            .expect("open")
            .is_empty()
    );
    assert!(matches!(
        inventory
            .resolve(&records[1].id, RootCause::Theft, "", "u3", at())
            .await,
        Err(InventoryError::Resolve(_))
    ));
    assert!(inventory.nightly_check().await.expect("check").is_empty());
}

#[tokio::test]
async fn a_dispatch_beyond_what_is_available_moves_nothing() {
    let inventory = inventory().await;
    inventory
        .record(&movement("receipt", MovementKind::Receipt, 3), AUTHORITY)
        .await
        .expect("receipt");
    // Two lines of the same stock: 2 then 2, more than the 3 there are.
    let transfer = Transfer::new(
        "t2",
        "Location/pharmacy",
        "Location/ward",
        None,
        vec![line(2), line(2)],
    )
    .expect("transfer");
    assert!(matches!(
        inventory.dispatch(transfer, &sender("u1"), at()).await,
        Err(InventoryError::Refused(Refused::InsufficientStock { .. }))
    ));
    assert_eq!(
        on_hand(&inventory, "Location/pharmacy", Status::Usable).await,
        3
    );
    assert!(matches!(
        inventory.transfer("t2").await,
        Err(InventoryError::NotFound(_))
    ));
}

#[tokio::test]
async fn a_direct_transfer_moves_both_sides_at_once() {
    let inventory = inventory().await;
    inventory
        .record(&movement("receipt", MovementKind::Receipt, 5), AUTHORITY)
        .await
        .expect("receipt");
    let transfer = Transfer::new(
        "t3",
        "Location/pharmacy",
        "Location/ward",
        None,
        vec![line(5)],
    )
    .expect("transfer");
    let moved = inventory
        .direct_transfer(transfer, &sender("u1"), at())
        .await
        .expect("direct");
    assert!(moved.direct);
    assert_eq!(
        on_hand(&inventory, "Location/pharmacy", Status::Usable).await,
        0
    );
    assert_eq!(
        on_hand(&inventory, "Location/ward", Status::Usable).await,
        5
    );
    assert!(inventory.nightly_check().await.expect("check").is_empty());
}

#[tokio::test]
async fn a_late_operation_driving_a_balance_negative_opens_a_record() {
    let inventory = inventory().await;
    inventory
        .record(&movement("receipt", MovementKind::Receipt, 1), AUTHORITY)
        .await
        .expect("receipt");
    inventory
        .record(&movement("late", MovementKind::Dispense, -3), Origin::Late)
        .await
        .expect("appended");
    let open = inventory.open_reconciliations().await.expect("open");
    assert_eq!(open.len(), 1);
    assert_eq!(
        (open[0].discrepancy, open[0].quantity),
        (Discrepancy::Negative, -2)
    );
    assert_eq!(open[0].source, "Movement/late");
    assert!(matches!(
        inventory
            .resolve(&open[0].id, RootCause::Other, "", "u1", at())
            .await,
        Err(InventoryError::Resolve(_))
    ));
    inventory
        .resolve(
            &open[0].id,
            RootCause::UnrecordedIssue,
            "two dispensed during an outage",
            "u1",
            at(),
        )
        .await
        .expect("resolved");
    assert!(
        inventory
            .open_reconciliations()
            .await
            .expect("open")
            .is_empty()
    );
}

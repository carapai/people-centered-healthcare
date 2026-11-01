//! Stock counts over SurrealDB (memory engine): no freeze, variances posted on approval with
//! Reconciliation Records, a flagged negative balance settled with its root cause, and the
//! InventoryReport export.

use std::sync::Arc;

use health_inventory::counts::{CountMode, CountRefused, CountStatus};
use health_inventory::reconciliation::{Discrepancy, RootCause};
use health_inventory::{Movement, MovementKind, Origin, Ownership, Policy, Status, StockUnit};
use health_inventory_app::{Cause, Inventory, InventoryError, Sender, inventory_report};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::{SystemClock, Timestamp};

const AUTHORITY: Origin = Origin::Authority {
    may_emergency_dispense: false,
};

async fn inventory() -> Inventory<SurrealStore> {
    let settings = Settings {
        target: Target::Memory,
        namespace: "counts".into(),
        database: "main".into(),
    };
    let connection = Connection::open(&settings).await.expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    Inventory::new(
        SurrealStore::new(connection, Arc::new(SystemClock)),
        Policy::default(),
    )
}

fn at(seconds: i64) -> Timestamp {
    Timestamp::from_second(1_800_000_000 + seconds).expect("time")
}

fn unit() -> StockUnit {
    StockUnit {
        store: "Location/pharmacy".into(),
        product: "amoxicillin-500".into(),
        batch: "B42".into(),
        expiry: "2027-06-30".into(),
        funding_source: "government".into(),
        ownership: Ownership::Owned,
        status: Status::Usable,
    }
}

fn movement(id: &str, kind: MovementKind, quantity: i64, recorded: i64) -> Movement {
    Movement {
        id: id.into(),
        unit: unit(),
        kind,
        quantity,
        pack_size: 1,
        unit_cost: None,
        occurred_at: at(recorded),
        recorded_at: at(recorded),
        source_document: "doc".into(),
        actor: "u1".into(),
        node: "E1".into(),
        op_id: id.into(),
        reason: None,
        reverses: None,
        emergency: false,
    }
}

fn sender(actor: &str) -> Sender {
    Sender {
        actor: actor.into(),
        node: "E1".into(),
    }
}

#[tokio::test]
async fn a_count_while_issuing_posts_only_the_true_variance() {
    let inventory = inventory().await;
    inventory
        .record(
            &movement("receipt", MovementKind::Receipt, 10, 0),
            AUTHORITY,
        )
        .await
        .expect("receipt");
    let count = inventory
        .start_count("c1", "Location/pharmacy", CountMode::Full, None, at(100))
        .await
        .expect("started");
    assert_eq!(count.lines.len(), 1);
    assert_eq!(count.lines[0].at_start, 10);
    // Issued while counting: not a variance.
    inventory
        .record(&movement("issue", MovementKind::Issue, -2, 200), AUTHORITY)
        .await
        .expect("issue");
    inventory
        .record_count("c1", 0, 7, Some("breakage"))
        .await
        .expect("counted");
    inventory.submit_count("c1", "u1").await.expect("submitted");
    let (approved, variances) = inventory
        .approve_count("c1", &sender("u2"), false, 5, &[], at(300))
        .await
        .expect("approved");
    assert_eq!(approved.status, CountStatus::Approved);
    assert_eq!(
        variances.iter().map(|v| v.variance).collect::<Vec<_>>(),
        [-1]
    );
    assert_eq!(
        inventory
            .balance(&unit().key())
            .await
            .expect("balance")
            .on_hand,
        7
    );
    let open = inventory.open_reconciliations().await.expect("open");
    assert_eq!(
        open.iter()
            .map(|r| (r.discrepancy, r.quantity, r.source.as_str()))
            .collect::<Vec<_>>(),
        [(Discrepancy::CountVariance, -1, "Count/c1")]
    );
    assert!(inventory.nightly_check().await.expect("check").is_empty());

    let report = inventory_report(&approved);
    assert_eq!(report["resourceType"], "InventoryReport");
    assert_eq!(report["countType"], "snapshot");
    assert_eq!(
        report["inventoryListing"][0]["item"][0]["quantity"]["value"],
        7
    );
}

#[tokio::test]
async fn a_large_variance_waits_for_a_supervisor() {
    let inventory = inventory().await;
    inventory
        .record(
            &movement("receipt", MovementKind::Receipt, 50, 0),
            AUTHORITY,
        )
        .await
        .expect("receipt");
    inventory
        .start_count("c2", "Location/pharmacy", CountMode::Blind, None, at(100))
        .await
        .expect("started");
    inventory
        .record_count("c2", 0, 30, Some("recount"))
        .await
        .expect("counted");
    inventory.submit_count("c2", "u1").await.expect("submitted");
    assert!(matches!(
        inventory
            .approve_count("c2", &sender("u1"), false, 5, &[], at(200))
            .await,
        Err(InventoryError::Count(CountRefused::SupervisorRequired(-20)))
    ));
    assert_eq!(
        inventory
            .balance(&unit().key())
            .await
            .expect("balance")
            .on_hand,
        50
    );
    assert_eq!(
        inventory.count("c2").await.expect("count").status,
        CountStatus::Submitted
    );
    inventory
        .approve_count("c2", &sender("supervisor"), true, 5, &[], at(200))
        .await
        .expect("approved");
    assert_eq!(
        inventory
            .balance(&unit().key())
            .await
            .expect("balance")
            .on_hand,
        30
    );
}

#[tokio::test]
async fn a_count_settles_a_flagged_negative_balance_only_with_its_root_cause() {
    let inventory = inventory().await;
    inventory
        .record(&movement("receipt", MovementKind::Receipt, 1, 0), AUTHORITY)
        .await
        .expect("receipt");
    inventory
        .record(
            &movement("late", MovementKind::Dispense, -3, 10),
            Origin::Late,
        )
        .await
        .expect("appended");
    let negative = inventory
        .open_reconciliations()
        .await
        .expect("open")
        .remove(0);
    assert!(
        inventory
            .balance(&unit().key())
            .await
            .expect("balance")
            .flagged
    );

    inventory
        .start_count("c3", "Location/pharmacy", CountMode::Full, None, at(100))
        .await
        .expect("started");
    inventory
        .record_count("c3", 0, 4, Some("unrecorded receipt found"))
        .await
        .expect("counted");
    inventory.submit_count("c3", "u1").await.expect("submitted");
    assert_eq!(
        inventory
            .approve_count("c3", &sender("s1"), true, 5, &[], at(200))
            .await,
        Err(InventoryError::CauseRequired(negative.id.clone()))
    );
    let cause = Cause {
        record: negative.id.clone(),
        cause: RootCause::RecordingError,
        note: "a delivery of 6 was never entered".into(),
    };
    inventory
        .approve_count("c3", &sender("s1"), true, 10, &[cause], at(200))
        .await
        .expect("approved");
    let balance = inventory.balance(&unit().key()).await.expect("balance");
    assert_eq!(balance.on_hand, 4);
    assert!(!balance.flagged);
    let open = inventory.open_reconciliations().await.expect("open");
    assert!(
        open.iter()
            .all(|r| r.discrepancy == Discrepancy::CountVariance),
        "{open:?}"
    );
    assert!(inventory.nightly_check().await.expect("check").is_empty());
}

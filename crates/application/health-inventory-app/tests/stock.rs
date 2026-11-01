//! Recording movements over SurrealDB (memory engine): concurrent issues of the last pack, status
//! changes and the nightly drift check.

use std::sync::Arc;

use health_inventory::{
    Movement, MovementKind, Origin, Ownership, Policy, Refused, Status, StockUnit,
};
use health_inventory_app::{Inventory, InventoryError};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::{SystemClock, Timestamp};

const AUTHORITY: Origin = Origin::Authority {
    may_emergency_dispense: false,
};

async fn inventory() -> Inventory<SurrealStore> {
    let settings = Settings {
        target: Target::Memory,
        namespace: "inventory".into(),
        database: "main".into(),
    };
    let connection = Connection::open(&settings).await.expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    Inventory::new(
        SurrealStore::new(connection, Arc::new(SystemClock)),
        Policy::default(),
    )
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

fn movement(id: &str, kind: MovementKind, quantity: i64) -> Movement {
    let at = Timestamp::from_second(1_800_000_000).expect("time");
    Movement {
        id: id.into(),
        unit: unit(),
        kind,
        quantity,
        pack_size: 10,
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
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_issues_of_the_last_pack_exactly_one_succeeds() {
    let inventory = Arc::new(inventory().await);
    inventory
        .record(&movement("receipt", MovementKind::Receipt, 1), AUTHORITY)
        .await
        .expect("receipt");

    let mut issues = tokio::task::JoinSet::new();
    for i in 0..6 {
        let inventory = Arc::clone(&inventory);
        issues.spawn(async move {
            inventory
                .record(
                    &movement(&format!("issue-{i}"), MovementKind::Issue, -1),
                    AUTHORITY,
                )
                .await
        });
    }
    let outcomes = issues.join_all().await;
    let succeeded = outcomes.iter().filter(|outcome| outcome.is_ok()).count();
    assert_eq!(succeeded, 1, "{outcomes:?}");
    for outcome in outcomes.iter().filter(|outcome| outcome.is_err()) {
        assert_eq!(
            outcome,
            &Err(InventoryError::Refused(Refused::InsufficientStock {
                available: 0
            }))
        );
    }
    let balance = inventory.balance(&unit().key()).await.expect("balance");
    assert_eq!(balance.on_hand, 0);
    assert!(!balance.flagged);
    assert!(inventory.nightly_check().await.expect("check").is_empty());
}

#[tokio::test]
async fn a_status_change_moves_stock_between_units_in_one_transaction() {
    let inventory = inventory().await;
    inventory
        .record(&movement("receipt", MovementKind::Receipt, 30), AUTHORITY)
        .await
        .expect("receipt");
    let mut hold = movement("hold", MovementKind::StatusChange, 12);
    hold.reason = Some("suspected cold-chain break".into());
    inventory
        .change_status(&hold, Status::Quarantine, AUTHORITY)
        .await
        .expect("quarantined");
    let quarantined = StockUnit {
        status: Status::Quarantine,
        ..unit()
    };
    assert_eq!(
        inventory
            .balance(&unit().key())
            .await
            .expect("usable")
            .on_hand,
        18
    );
    assert_eq!(
        inventory
            .balance(&quarantined.key())
            .await
            .expect("quarantine")
            .on_hand,
        12
    );
    // More than is usable: refused, and neither unit changes.
    let mut too_much = movement("hold-2", MovementKind::StatusChange, 19);
    too_much.reason = Some("recall".into());
    assert!(matches!(
        inventory
            .change_status(&too_much, Status::Quarantine, AUTHORITY)
            .await,
        Err(InventoryError::Refused(Refused::InsufficientStock {
            available: 18
        }))
    ));
    assert_eq!(
        inventory
            .balance(&unit().key())
            .await
            .expect("usable")
            .on_hand,
        18
    );
    assert!(inventory.nightly_check().await.expect("check").is_empty());
}

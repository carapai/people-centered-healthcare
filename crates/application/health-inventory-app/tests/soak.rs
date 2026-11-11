//! The stock soak (#141; testing.md § Stock concurrency): many writers issue, dispense and receive
//! on one Stock Unit at once. Under the authority's policy no movement takes the balance below
//! zero, every movement that was refused or lost left nothing behind, and the balance record
//! equals the ledger rebuilt from the movements, so the nightly check finds no drift.
//! `HEALTH_STOCK_SOAK_WRITERS` and `HEALTH_STOCK_SOAK_OPERATIONS` set the load: 4 writers and 200
//! operations by default, 16 and 2000 nightly. They take turns per store, so none is ever `Busy`.

use std::sync::Arc;
use std::sync::atomic::{AtomicI64, AtomicUsize, Ordering};

use health_inventory::{
    Movement, MovementKind, Origin, Ownership, Policy, Refused, Status, StockStore, StockUnit,
};
use health_inventory_app::{Inventory, InventoryError};
use health_persistence::Store;
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::{SystemClock, Timestamp};
use rust_decimal::Decimal;

const OPENING: i64 = 100;

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
    let at = Timestamp::now();
    Movement {
        id: id.into(),
        unit: unit(),
        kind,
        quantity,
        pack_size: 1,
        unit_cost: (kind == MovementKind::Receipt).then(|| Decimal::new(250, 2)),
        occurred_at: at,
        recorded_at: at,
        source_document: format!("soak/{id}"),
        actor: "pharm-1".into(),
        node: "C1".into(),
        op_id: id.into(),
        reason: None,
        reverses: None,
        emergency: false,
    }
}

const AUTHORITY: Origin = Origin::Authority {
    may_emergency_dispense: false,
};

#[derive(Default)]
struct Tally {
    taken: AtomicI64,
    received: AtomicI64,
    recorded: AtomicUsize,
    refused: AtomicUsize,
    busy: AtomicUsize,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_movements_on_one_unit_never_go_negative_and_match_the_ledger() {
    let operations: usize = std::env::var("HEALTH_STOCK_SOAK_OPERATIONS")
        .ok()
        .and_then(|n| n.parse().ok())
        .unwrap_or(200);
    let writers: usize = std::env::var("HEALTH_STOCK_SOAK_WRITERS")
        .ok()
        .and_then(|n| n.parse().ok())
        .unwrap_or(4);
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: "soak".into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    let store = SurrealStore::new(connection, Arc::new(SystemClock));
    let inventory = Arc::new(Inventory::new(store.clone(), Policy::default()));
    inventory
        .record(
            &movement("opening", MovementKind::Receipt, OPENING),
            AUTHORITY,
        )
        .await
        .expect("opening stock");

    let tally = Arc::new(Tally::default());
    let mut tasks = Vec::new();
    for writer in 0..writers {
        let inventory = Arc::clone(&inventory);
        let tally = Arc::clone(&tally);
        tasks.push(tokio::spawn(async move {
            for n in (writer..operations).step_by(writers) {
                // Mostly taking stock out, one or two at a time; every seventh a delivery, so the
                // unit runs dry and refills while the writers race.
                let (kind, quantity) = match n % 7 {
                    0 => (MovementKind::Receipt, 5),
                    1 | 3 | 5 => (MovementKind::Dispense, -1),
                    _ => (MovementKind::Issue, -2),
                };
                let id = format!("m{n}");
                match inventory
                    .record(&movement(&id, kind, quantity), AUTHORITY)
                    .await
                {
                    Ok(decision) => {
                        assert!(
                            decision.balance.on_hand >= 0,
                            "{id} left {} on hand",
                            decision.balance.on_hand
                        );
                        assert!(!decision.emergency, "{id} was flagged as an emergency");
                        tally.recorded.fetch_add(1, Ordering::SeqCst);
                        if quantity > 0 {
                            tally.received.fetch_add(quantity, Ordering::SeqCst);
                        } else {
                            tally.taken.fetch_add(-quantity, Ordering::SeqCst);
                        }
                    }
                    Err(InventoryError::Refused(Refused::InsufficientStock { available })) => {
                        assert!(
                            available < -quantity,
                            "{id} refused with {available} available"
                        );
                        tally.refused.fetch_add(1, Ordering::SeqCst);
                    }
                    Err(InventoryError::Busy) => {
                        tally.busy.fetch_add(1, Ordering::SeqCst);
                    }
                    Err(other) => panic!("{id}: {other}"),
                }
            }
        }));
    }
    for task in tasks {
        task.await.expect("writer");
    }

    let recorded = tally.recorded.load(Ordering::SeqCst);
    let refused = tally.refused.load(Ordering::SeqCst);
    let busy = tally.busy.load(Ordering::SeqCst);
    let expected =
        OPENING + tally.received.load(Ordering::SeqCst) - tally.taken.load(Ordering::SeqCst);
    eprintln!(
        "stock soak: {operations} operations by {writers} writers: {recorded} recorded, \
         {refused} refused for stock, {busy} busy; {expected} on hand"
    );
    assert_eq!(recorded + refused + busy, operations);
    // Writers on one node take turns per store, so none runs out of attempts.
    assert_eq!(busy, 0, "writers on one node collided");
    // The stock ran out and was refused, so the race was real.
    assert!(
        refused > 0,
        "no movement was ever refused: the unit never ran dry"
    );
    // The balance record is what the recorded movements add up to, and the ledger agrees.
    let balance = inventory.balance(&unit().key()).await.expect("balance");
    assert_eq!(balance.on_hand, expected, "the balance record");
    assert!(balance.on_hand >= 0);
    let mut tx = store.begin().await.expect("begin");
    let movements = tx.movements(&unit().key()).await.expect("movements");
    drop(tx);
    assert_eq!(
        movements.len(),
        recorded + 1,
        "a refused or lost movement left a trace, or a recorded one is missing"
    );
    assert_eq!(
        health_inventory::rebuild(&movements),
        expected,
        "the ledger"
    );
    // The stock card ends at the balance. It orders by `occurred_at`, which each writer sets
    // before its reruns, so a dispense made a moment before a delivery, that could only commit
    // after it, may dip below zero part way: occurrence order is not commit order. The guard is
    // the balance record, checked above.
    assert_eq!(
        health_inventory::stock_card(&movements)
            .last()
            .map(|(_, running)| *running),
        Some(expected),
        "the stock card's closing balance"
    );
    assert!(
        inventory.nightly_check().await.expect("check").is_empty(),
        "the nightly check finds drift"
    );
}

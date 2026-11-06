//! Dispensing over SurrealDB (memory engine; #128): stock moves only on a completed dispense,
//! preparations reserve by first expiry for eligible patients, concurrent preparations of the
//! last packs, partial dispenses, cancellation, lapsed reservations, and returns to quarantine.

use std::collections::BTreeMap;
use std::sync::Arc;

use health_inventory::dispensing::{BatchRule, Eligibility, Need, Pick, SelectRefused};
use health_inventory::{Movement, MovementKind, Origin, Ownership, Policy, Status, StockUnit};
use health_inventory_app::{Completion, Inventory, InventoryError, PatientReturn, Preparation};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::{SignedDuration, SystemClock, Timestamp};
use jiff::civil::date;

const AUTHORITY: Origin = Origin::Authority {
    may_emergency_dispense: false,
};
const STORE: &str = "Location/pharmacy";
const RULE: BatchRule = BatchRule { buffer_days: 30 };

async fn inventory(namespace: &str) -> Inventory<SurrealStore> {
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: namespace.into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    Inventory::new(
        SurrealStore::new(connection, Arc::new(SystemClock)),
        Policy::default(),
    )
}

fn unit(batch: &str, expiry: &str, funding: &str) -> StockUnit {
    StockUnit {
        store: STORE.into(),
        product: "amoxicillin-500".into(),
        batch: batch.into(),
        expiry: expiry.into(),
        funding_source: funding.into(),
        ownership: Ownership::Owned,
        status: Status::Usable,
    }
}

fn now() -> Timestamp {
    "2026-11-06T08:00:00Z".parse().expect("time")
}

async fn receive(inventory: &Inventory<SurrealStore>, unit: &StockUnit, quantity: i64) {
    let at = now();
    let id = format!("receipt-{}", unit.batch);
    inventory
        .record(
            &Movement {
                id: id.clone(),
                unit: unit.clone(),
                kind: MovementKind::Receipt,
                quantity,
                pack_size: 1,
                unit_cost: None,
                occurred_at: at,
                recorded_at: at,
                source_document: "GRN/1".into(),
                actor: "storekeeper".into(),
                node: "E1".into(),
                op_id: id,
                reason: None,
                reverses: None,
                emergency: false,
            },
            AUTHORITY,
        )
        .await
        .expect("receipt");
}

fn preparation(dispense: &str, quantity: i64, programs: &[&str], at: Timestamp) -> Preparation {
    Preparation {
        dispense: format!("MedicationDispense/{dispense}"),
        store: STORE.into(),
        need: Need {
            product: "amoxicillin-500".into(),
            quantity,
            course_ends: date(2026, 11, 13),
            programs: programs.iter().map(|p| (*p).to_owned()).collect(),
        },
        choice: None,
        at,
    }
}

fn completion(dispense: &str, given: &[(&str, i64)], at: Timestamp) -> Completion {
    Completion {
        dispense: format!("MedicationDispense/{dispense}"),
        patient: "Patient/p1".into(),
        given: given
            .iter()
            .map(|(id, q)| ((*id).to_owned(), *q))
            .collect::<BTreeMap<_, _>>(),
        may_emergency_dispense: false,
        actor: "pharmacist".into(),
        node: "E1".into(),
        at,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stock_moves_only_when_a_dispense_is_completed() {
    let inventory = inventory("dispense-moves").await;
    let soon = unit("SOON", "2027-01-31", "gou");
    let late = unit("LATE", "2027-12-31", "gou");
    receive(&inventory, &soon, 10).await;
    receive(&inventory, &late, 100).await;
    let at = now();

    // Preparing reserves by first expiry, across batches: available falls, on-hand does not.
    let held = inventory
        .prepare(
            &preparation("d1", 14, &[], at),
            &Eligibility::default(),
            RULE,
        )
        .await
        .expect("prepared");
    let picks: Vec<(&str, i64)> = held
        .iter()
        .map(|r| (r.unit.batch.as_str(), r.quantity))
        .collect();
    assert_eq!(picks, [("SOON", 10), ("LATE", 4)]);
    assert_eq!(
        inventory
            .available(&soon.key(), at)
            .await
            .expect("available"),
        0
    );
    assert_eq!(
        inventory
            .balance(&soon.key())
            .await
            .expect("balance")
            .on_hand,
        10,
        "nothing moved"
    );
    // Preparing it again changes nothing.
    let again = inventory
        .prepare(
            &preparation("d1", 14, &[], at),
            &Eligibility::default(),
            RULE,
        )
        .await
        .expect("again");
    assert_eq!(again, held);

    // Completing it, in part: 10 from SOON, 2 of the 4 from LATE; the other 2 go back.
    let ids: Vec<String> = held.iter().map(|r| r.id.clone()).collect();
    let moved = inventory
        .complete_dispense(&completion("d1", &[(&ids[0], 10), (&ids[1], 2)], at))
        .await
        .expect("completed");
    assert_eq!(
        moved
            .iter()
            .map(|m| (m.kind, m.unit.batch.as_str(), m.quantity))
            .collect::<Vec<_>>(),
        [
            (MovementKind::Dispense, "SOON", -10),
            (MovementKind::Dispense, "LATE", -2)
        ]
    );
    assert_eq!(
        inventory
            .balance(&soon.key())
            .await
            .expect("balance")
            .on_hand,
        0
    );
    assert_eq!(
        inventory
            .balance(&late.key())
            .await
            .expect("balance")
            .on_hand,
        98
    );
    assert_eq!(
        inventory
            .available(&late.key(), at)
            .await
            .expect("available"),
        98,
        "the rest is released"
    );
    // Completed once: its reservations are consumed.
    assert!(matches!(
        inventory
            .complete_dispense(&completion("d1", &[(&ids[0], 1)], at))
            .await,
        Err(InventoryError::NotFound(_))
    ));
    assert!(
        inventory.nightly_check().await.expect("check").is_empty(),
        "no drift"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_cancelled_or_lapsed_preparation_moves_nothing_and_frees_its_stock() {
    let inventory = inventory("dispense-cancel").await;
    let soon = unit("SOON", "2027-01-31", "gou");
    receive(&inventory, &soon, 10).await;
    let at = now();
    inventory
        .prepare(
            &preparation("d1", 6, &[], at),
            &Eligibility::default(),
            RULE,
        )
        .await
        .expect("prepared");
    assert_eq!(
        inventory
            .cancel_dispense("MedicationDispense/d1", at)
            .await
            .expect("cancelled"),
        1
    );
    assert_eq!(
        inventory
            .available(&soon.key(), at)
            .await
            .expect("available"),
        10
    );
    assert_eq!(
        inventory
            .balance(&soon.key())
            .await
            .expect("balance")
            .on_hand,
        10
    );

    // An abandoned preparation lapses after 4 hours: its stock is free without anyone releasing it.
    let held = inventory
        .prepare(
            &preparation("d2", 6, &[], at),
            &Eligibility::default(),
            RULE,
        )
        .await
        .expect("prepared");
    let later = at
        .checked_add(SignedDuration::from_hours(5))
        .expect("later");
    assert_eq!(
        inventory
            .available(&soon.key(), later)
            .await
            .expect("available"),
        10
    );
    assert_eq!(
        inventory
            .complete_dispense(&completion("d2", &[(&held[0].id, 6)], later))
            .await,
        Err(InventoryError::Lapsed(held[0].id.clone())),
        "it must be prepared again"
    );
    // Another preparation may now take that stock.
    inventory
        .prepare(
            &preparation("d3", 10, &[], later),
            &Eligibility::default(),
            RULE,
        )
        .await
        .expect("prepared");
    assert_eq!(
        inventory
            .balance(&soon.key())
            .await
            .expect("balance")
            .on_hand,
        10,
        "still nothing moved"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_preparations_of_the_last_packs_never_promise_them_twice() {
    let inventory = Arc::new(inventory("dispense-race").await);
    let soon = unit("SOON", "2027-01-31", "gou");
    receive(&inventory, &soon, 3).await;
    let at = now();
    let mut preparations = tokio::task::JoinSet::new();
    for i in 0..6 {
        let inventory = Arc::clone(&inventory);
        preparations.spawn(async move {
            inventory
                .prepare(
                    &preparation(&format!("d{i}"), 2, &[], at),
                    &Eligibility::default(),
                    RULE,
                )
                .await
        });
    }
    let outcomes = preparations.join_all().await;
    let reserved: i64 = outcomes
        .iter()
        .filter_map(|o| o.as_ref().ok())
        .flatten()
        .map(|r| r.quantity)
        .sum();
    assert_eq!(
        outcomes.iter().filter(|o| o.is_ok()).count(),
        1,
        "{outcomes:?}"
    );
    assert_eq!(reserved, 2);
    for refused in outcomes.iter().filter(|o| o.is_err()) {
        assert_eq!(
            refused,
            &Err(InventoryError::Select(SelectRefused::NotEnough {
                usable: 1
            }))
        );
    }
    assert_eq!(
        inventory
            .available(&soon.key(), at)
            .await
            .expect("available"),
        1
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn donor_stock_is_reserved_only_for_eligible_patients() {
    let inventory = inventory("dispense-donor").await;
    let donor = unit("DONOR", "2027-01-31", "pepfar");
    receive(&inventory, &donor, 50).await;
    let rules = Eligibility::new([("pepfar", "hiv")]);
    let at = now();
    assert_eq!(
        inventory
            .prepare(&preparation("d1", 5, &[], at), &rules, RULE)
            .await,
        Err(InventoryError::Select(SelectRefused::NotEnough {
            usable: 0
        }))
    );
    let mut asked = preparation("d2", 5, &[], at);
    asked.choice = Some((
        vec![Pick {
            unit: donor.clone(),
            quantity: 5,
        }],
        "nothing else in stock".into(),
    ));
    assert!(matches!(
        inventory.prepare(&asked, &rules, RULE).await,
        Err(InventoryError::Select(SelectRefused::NotEligible(_)))
    ));
    let held = inventory
        .prepare(&preparation("d3", 5, &["hiv"], at), &rules, RULE)
        .await
        .expect("enrolled");
    assert_eq!(held[0].unit.batch, "DONOR");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reserved_stock_is_not_issued_and_a_return_goes_to_quarantine() {
    let inventory = inventory("dispense-return").await;
    let soon = unit("SOON", "2027-01-31", "gou");
    receive(&inventory, &soon, 4).await;
    let at = now();
    inventory
        .prepare(
            &preparation("d1", 3, &[], at),
            &Eligibility::default(),
            RULE,
        )
        .await
        .expect("prepared");
    // An issue elsewhere sees only what is not reserved.
    let issue = Movement {
        id: "issue-1".into(),
        unit: soon.clone(),
        kind: MovementKind::Issue,
        quantity: -2,
        pack_size: 1,
        unit_cost: None,
        occurred_at: at,
        recorded_at: at,
        source_document: "Requisition/1".into(),
        actor: "storekeeper".into(),
        node: "E1".into(),
        op_id: "issue-1".into(),
        reason: None,
        reverses: None,
        emergency: false,
    };
    assert!(
        inventory.record(&issue, AUTHORITY).await.is_err(),
        "3 of the 4 are reserved"
    );

    let back = inventory
        .return_to_quarantine(&PatientReturn {
            id: "return-1".into(),
            unit: soon.clone(),
            quantity: 1,
            dispense: "MedicationDispense/d0".into(),
            actor: "pharmacist".into(),
            node: "E1".into(),
            at,
        })
        .await
        .expect("returned");
    assert_eq!(back.unit.status, Status::Quarantine);
    let quarantined = StockUnit {
        status: Status::Quarantine,
        ..soon.clone()
    };
    assert_eq!(
        inventory
            .balance(&quarantined.key())
            .await
            .expect("balance")
            .on_hand,
        1
    );
    assert_eq!(
        inventory
            .balance(&soon.key())
            .await
            .expect("balance")
            .on_hand,
        4,
        "not back into usable stock"
    );
}

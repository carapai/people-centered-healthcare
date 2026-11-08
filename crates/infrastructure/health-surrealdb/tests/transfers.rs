//! Inter-facility transfers through central (#247): the source dispatches at its own edge, the
//! transfer reaches the destination's edge through central, the destination receives short, and
//! central, matching the source's dispatch with the destination's receipt, raises a
//! Reconciliation Record.

use std::sync::Arc;

use health_inventory::reconciliation::Discrepancy;
use health_inventory::transfers::{Counted, Transfer, TransferLine, TransferStatus};
use health_inventory::{
    Movement, MovementKind, Origin, Ownership, Policy, Status, StockStore as _, StockUnit,
};
use health_inventory_app::{Inventory, Sender};
use health_persistence::Store as _;
use health_surrealdb::{
    Connection, MIGRATIONS, Settings, SurrealStore, Target, TransferMatch, TransferRefused, migrate,
};
use health_time::{SystemClock, Timestamp};

async fn store(namespace: &str) -> SurrealStore {
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: namespace.into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    SurrealStore::new(connection, Arc::new(SystemClock))
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

fn sender(node: &str) -> Sender {
    Sender {
        actor: "storekeeper".into(),
        node: node.into(),
    }
}

#[tokio::test]
async fn a_short_receipt_matched_against_the_dispatch_at_central_raises_a_reconciliation() {
    let (source, destination, central) = (
        store("kla").await,
        store("gul").await,
        store("central").await,
    );
    source.add_upstream("central").await.expect("upstream");
    destination.add_upstream("central").await.expect("upstream");
    let at_source = Inventory::new(source.clone(), Policy::default());
    let authority = Origin::Authority {
        may_emergency_dispense: false,
    };
    at_source
        .record(
            &Movement {
                id: "grn-1".into(),
                unit: unit("Location/kla-store", Status::Usable),
                kind: MovementKind::Receipt,
                quantity: 100,
                pack_size: 10,
                unit_cost: None,
                occurred_at: at(),
                recorded_at: at(),
                source_document: "grn".into(),
                actor: "storekeeper".into(),
                node: "KLA-E1".into(),
                op_id: "grn-1".into(),
                reason: None,
                reverses: None,
                emergency: false,
            },
            authority,
        )
        .await
        .expect("receipt");

    // KLA01 dispatches 40 to GUL02.
    let transfer = Transfer::new(
        "t-kla-gul",
        "Location/kla-store",
        "Location/gul-store",
        Some(("KLA01".into(), "GUL02".into())),
        vec![line(40)],
    )
    .expect("transfer");
    at_source
        .dispatch(transfer, &sender("KLA-E1"), at())
        .await
        .expect("dispatch");

    // Up from the source to central; another facility may not send it.
    let queued = source.queued_transfers("central").await.expect("queue");
    assert_eq!(queued.len(), 1);
    assert!(matches!(
        central.receive_transfer("MBR03", queued[0].clone()).await,
        Err(TransferRefused::NotTheirs(_))
    ));
    assert_eq!(
        central
            .receive_transfer("KLA01", queued[0].clone())
            .await
            .expect("receive"),
        TransferMatch::Pending
    );
    source
        .transfers_sent("central", &["t-kla-gul".into()])
        .await
        .expect("sent");

    // Down to the destination, which receives 35 of the 40.
    let inbound = central.inbound_transfers("GUL02").await.expect("inbound");
    assert_eq!(inbound.len(), 1);
    assert!(
        central
            .inbound_transfers("KLA01")
            .await
            .expect("none")
            .is_empty()
    );
    assert!(
        destination
            .accept_transfer(&inbound[0])
            .await
            .expect("accept")
    );
    assert!(
        !destination
            .accept_transfer(&inbound[0])
            .await
            .expect("again"),
        "kept once"
    );
    let at_destination = Inventory::new(destination.clone(), Policy::default());
    let (received, local) = at_destination
        .receive(
            "t-kla-gul",
            vec![Counted {
                line: 0,
                good: 35,
                damaged: 0,
            }],
            &sender("GUL-E1"),
            at(),
        )
        .await
        .expect("receive");
    assert_eq!(received.status, TransferStatus::Reconciling);
    assert_eq!(local.len(), 1, "the destination reconciles its own receipt");

    // The receipt goes up; central matches it against the source's dispatch. The destination's
    // copy cannot change what was sent: central keeps the source's lines.
    let mut queued = destination
        .queued_transfers("central")
        .await
        .expect("queue");
    assert_eq!(queued.len(), 1);
    queued[0].lines[0].quantity = 35;
    let matched = central
        .receive_transfer("GUL02", queued[0].clone())
        .await
        .expect("match");
    let TransferMatch::Mismatch(records) = matched else {
        panic!("a short receipt is a mismatch: {matched:?}")
    };
    assert_eq!(records.len(), 1);
    let mut tx = central.begin().await.expect("begin");
    let record = tx
        .reconciliation(&records[0])
        .await
        .expect("read")
        .expect("record");
    assert_eq!(record.discrepancy, Discrepancy::Short);
    assert_eq!(record.quantity, -5);
    assert_eq!(
        record.unit,
        unit("Location/gul-store", Status::Usable).key()
    );
    assert!(
        central
            .inbound_transfers("GUL02")
            .await
            .expect("none")
            .is_empty(),
        "received"
    );
}

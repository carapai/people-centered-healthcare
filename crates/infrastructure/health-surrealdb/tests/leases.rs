//! Authority Leases on the stock ledger (#135; write-authority.md § Hand-over rules): while an
//! edge has leased an outreach store to a desktop, the edge's own stock movements there are
//! refused and it takes the store's operations from that desktop alone; the desktop moves the
//! store's stock only while its lease is active; once returned, the store is the edge's again.

use std::collections::BTreeSet;
use std::sync::Arc;

use health_authority_leases::Lease;
use health_identifiers::NodeId;
use health_inventory::{
    Movement, MovementKind, Origin, Ownership, Policy, Refused, Status, StockUnit,
};
use health_inventory_app::{Inventory, InventoryError};
use health_surrealdb::{
    Connection, LeaseRole, MIGRATIONS, Settings, SurrealAuthority, SurrealStore, SyncScope, Target,
    migrate,
};
use health_sync_protocol::{Outcome, receive};
use health_time::{FixedClock, SignedDuration, Timestamp};

const OUTREACH: &str = "Location/outreach-bag";
const PHARMACY: &str = "Location/main-pharmacy";

struct Node {
    id: NodeId,
    store: SurrealStore,
}

async fn node() -> Node {
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: "test".into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    Node {
        id: NodeId::new(),
        store: SurrealStore::new(connection, Arc::new(FixedClock::new(at(0)))),
    }
}

fn at(hours: i64) -> Timestamp {
    "2026-11-12T06:00:00Z"
        .parse::<Timestamp>()
        .expect("time")
        .checked_add(SignedDuration::from_hours(hours))
        .expect("time")
}

fn receipt(id: &str, store: &str, hours: i64) -> Movement {
    Movement {
        id: id.into(),
        unit: StockUnit {
            store: store.into(),
            product: "amoxicillin-500".into(),
            batch: "B42".into(),
            expiry: "2027-06-30".into(),
            funding_source: "government".into(),
            ownership: Ownership::Owned,
            status: Status::Usable,
        },
        kind: MovementKind::Receipt,
        quantity: 10,
        pack_size: 10,
        unit_cost: None,
        occurred_at: at(hours),
        recorded_at: at(hours),
        source_document: "grn".into(),
        actor: "storekeeper".into(),
        node: "KLA01".into(),
        op_id: id.into(),
        reason: None,
        reverses: None,
        emergency: false,
    }
}

async fn record(node: &Node, movement: &Movement) -> Result<(), InventoryError> {
    Inventory::new(node.store.clone(), Policy::default())
        .record(
            movement,
            Origin::Authority {
                may_emergency_dispense: false,
            },
        )
        .await
        .map(|_| ())
}

/// The outcomes of `from`'s queued operations at `edge`.
async fn push(from: &Node, edge: &Node) -> Vec<Outcome> {
    let sent = from
        .store
        .outbox(&edge.id.to_string(), from.id, 50)
        .await
        .expect("outbox");
    let authority =
        SurrealAuthority::new(&edge.store, from.id, SyncScope::Facility("KLA01".into()));
    let mut outcomes = Vec::new();
    for outgoing in &sent {
        outcomes.push(
            receive(&authority, &outgoing.group)
                .await
                .expect("receive")
                .outcome,
        );
    }
    outcomes
}

#[tokio::test]
async fn a_leased_outreach_store_is_the_holders_alone_until_it_is_returned() {
    let edge = node().await;
    let holder = node().await;
    let other = node().await;
    for desktop in [&holder, &other] {
        desktop
            .store
            .add_upstream(&edge.id.to_string())
            .await
            .expect("upstream");
    }
    let mut lease = Lease::grant(
        uuid::Uuid::now_v7(),
        holder.id.to_string(),
        BTreeSet::from([OUTREACH.to_owned()]),
        "admin",
        at(0),
        SignedDuration::from_hours(8),
        &[],
    )
    .expect("grant");
    edge.store
        .put_lease(LeaseRole::Granted, &lease)
        .await
        .expect("granted");
    holder
        .store
        .put_lease(LeaseRole::Held, &lease)
        .await
        .expect("held");

    // The edge cannot move the outreach store's stock, only its own pharmacy's.
    assert!(matches!(
        record(&edge, &receipt("edge-grn", OUTREACH, 1)).await,
        Err(InventoryError::Refused(Refused::Leased(store))) if store == OUTREACH
    ));
    record(&edge, &receipt("edge-pharmacy", PHARMACY, 1))
        .await
        .expect("the pharmacy is the edge's");

    // The holder moves it while the lease is active; past its expiry, no more.
    record(&holder, &receipt("outreach-grn", OUTREACH, 2))
        .await
        .expect("the holder's");
    assert!(matches!(
        record(&holder, &receipt("outreach-late", OUTREACH, 9)).await,
        Err(InventoryError::Refused(Refused::Leased(_)))
    ));
    // Another desktop records a candidate on it, which the edge refuses on arrival.
    record(&other, &receipt("other-grn", OUTREACH, 2))
        .await
        .expect("a local candidate");
    assert_eq!(push(&holder, &edge).await, vec![Outcome::Accepted]);
    let refused = push(&other, &edge).await;
    assert!(
        matches!(&refused[..], [Outcome::Rejected { detail, .. }] if detail.contains("is leased to")),
        "{refused:?}"
    );

    // Returned once drained: the store is the edge's again.
    lease
        .request_return(&holder.id.to_string(), at(3))
        .expect("return");
    lease.complete_return(true, at(4)).expect("returned");
    edge.store
        .put_lease(LeaseRole::Granted, &lease)
        .await
        .expect("returned");
    record(&edge, &receipt("edge-after", OUTREACH, 5))
        .await
        .expect("the edge's again");
    assert_eq!(
        edge.store.leases(LeaseRole::Granted).await.expect("leases"),
        vec![lease]
    );
}

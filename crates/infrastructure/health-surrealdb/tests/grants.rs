//! Break-glass grants travel up (#224): a grant made at an edge, offline or not, is queued for
//! central, kept there with its review due 72 hours after it arrives, and its acknowledgement
//! follows it.

use std::sync::Arc;

use health_persistence::Store as _;
use health_persistence::Transaction as _;
use health_surrealdb::{
    BreakGlassRow, Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate,
};
use health_time::{FixedClock, SignedDuration, Timestamp};

async fn store(namespace: &str) -> SurrealStore {
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: namespace.into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    let at: Timestamp = "2026-11-09T08:00:00Z".parse().expect("time");
    SurrealStore::new(connection, Arc::new(FixedClock::new(at)))
}

#[tokio::test]
async fn a_grant_made_offline_is_reviewed_72_hours_after_it_reaches_central() {
    let (edge, central) = (store("edge").await, store("central").await);
    edge.add_upstream("central").await.expect("upstream");
    let made: Timestamp = "2026-11-02T10:00:00Z".parse().expect("time");
    let mut tx = edge.begin().await.expect("begin");
    tx.put_break_glass(&BreakGlassRow {
        id: "bg-1".into(),
        session: "s-1".into(),
        account: "a-nurse".into(),
        facility: "KLA01".into(),
        patient: "Patient/p-1".into(),
        reason: "BTG".into(),
        justification: "unconscious, no consent possible".into(),
        at: made,
        expires_at: made + SignedDuration::from_hours(1),
        review_due: made + SignedDuration::from_hours(72),
        acknowledged: false,
        review: None,
        escalated_at: None,
    })
    .await
    .expect("grant");
    tx.commit().await.expect("commit");

    // A week later the edge reaches central.
    let queued = edge.queued_grants("central").await.expect("queue");
    assert_eq!(queued.len(), 1);
    let arrived: Timestamp = "2026-11-09T08:00:00Z".parse().expect("time");
    assert!(
        central
            .receive_grant(&queued[0], arrived)
            .await
            .expect("receive")
    );
    assert!(
        !central
            .receive_grant(&queued[0], arrived)
            .await
            .expect("again"),
        "kept once"
    );
    edge.grants_sent("central", &["bg-1".into()])
        .await
        .expect("sent");
    assert!(
        edge.queued_grants("central")
            .await
            .expect("queue")
            .is_empty()
    );

    let mut tx = central.begin().await.expect("begin");
    let reviews = tx.break_glass_reviews(10).await.expect("reviews");
    drop(tx);
    assert_eq!(reviews.len(), 1);
    assert_eq!(
        reviews[0].review_due,
        arrived + SignedDuration::from_hours(72)
    );
    assert_eq!(reviews[0].at, made, "the event keeps its own time");
    assert!(!reviews[0].acknowledged);

    // The in-charge acknowledges at the facility; the acknowledgement follows the grant up.
    let mut tx = edge.begin().await.expect("begin");
    assert!(
        tx.acknowledge_break_glass("bg-1", "KLA01")
            .await
            .expect("ack")
    );
    tx.commit().await.expect("commit");
    let queued = edge.queued_grants("central").await.expect("queue");
    assert!(queued[0].acknowledged);
    central
        .receive_grant(&queued[0], arrived + SignedDuration::from_hours(1))
        .await
        .expect("receive");
    let mut tx = central.begin().await.expect("begin");
    let reviews = tx.break_glass_reviews(10).await.expect("reviews");
    assert!(reviews[0].acknowledged);
    assert_eq!(
        reviews[0].review_due,
        arrived + SignedDuration::from_hours(72),
        "the clock started at first arrival"
    );
}

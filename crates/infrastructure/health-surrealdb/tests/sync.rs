//! Sync between two stores through the `health-sync-protocol` ports (#81; synchronization.md):
//! convergence after a partition, idempotent retries, conflicts kept as candidates, and the Sync
//! Scope on push and pull. In-memory engine; the round is driven in process, as the edge's sync
//! task drives it over HTTP.

use std::sync::Arc;

use health_identifiers::{NodeId, OpId, ResourceId};
use health_persistence::{
    Precondition, RawJson, ResourceType, Store, Transaction, VersionMeta, WriteRequest,
};
use health_surrealdb::{
    Connection, MIGRATIONS, Settings, SurrealAuthority, SurrealReplica, SurrealStore, SyncScope,
    Target, migrate,
};
use health_sync_protocol::{Lane, Outcome, PullRequest, PullResponse, Reason, apply_pull, receive};
use health_time::{FixedClock, Timestamp};
use serde_json::{Value, json};

struct Node {
    id: NodeId,
    store: SurrealStore,
}

async fn node(at: &str) -> Node {
    let settings = Settings {
        target: Target::Memory,
        namespace: "test".into(),
        database: "main".into(),
    };
    let connection = Connection::open(&settings).await.expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    let at: Timestamp = at.parse().expect("timestamp");
    Node {
        id: NodeId::new(),
        store: SurrealStore::new(connection, Arc::new(FixedClock::new(at))),
    }
}

/// Central and one edge of facility `KLA01`, the edge with central as its upstream.
async fn pair() -> (Node, Node) {
    let central = node("2026-11-08T08:00:00Z").await;
    let edge = node("2026-11-08T09:00:00Z").await;
    edge.store
        .add_upstream(&central.id.to_string())
        .await
        .expect("upstream");
    (central, edge)
}

fn scope() -> SyncScope {
    SyncScope::Facility("KLA01".into())
}

fn observation(facility: &str, value: i64) -> Value {
    json!({
        "resourceType": "Observation",
        "status": "final",
        "meta": {"tag": [{"system": "urn:health:facility", "code": facility}]},
        "valueInteger": value
    })
}

fn patient(name: &str) -> Value {
    json!({"resourceType": "Patient", "name": [{"text": name}]})
}

async fn write(
    node: &Node,
    id: ResourceId,
    body: &Value,
    precondition: Precondition,
) -> VersionMeta {
    let resource_type: ResourceType = body["resourceType"]
        .as_str()
        .expect("type")
        .parse()
        .expect("type");
    let mut tx = node.store.begin().await.expect("begin");
    let meta = tx
        .write(WriteRequest {
            resource_type,
            id,
            json: RawJson::from_validated(body.to_string()),
            op_id: OpId::new(),
            precondition,
        })
        .await
        .expect("write");
    tx.commit().await.expect("commit");
    meta
}

async fn head(node: &Node, resource_type: &str, id: ResourceId) -> Option<(OpId, Value)> {
    let mut tx = node.store.begin().await.expect("begin");
    let head = tx
        .read(&resource_type.parse().expect("type"), id)
        .await
        .expect("read")?;
    Some((
        head.version.version_id,
        serde_json::from_str(head.json.as_str()).expect("json"),
    ))
}

/// What one sync round moved.
#[derive(Debug, Default, PartialEq, Eq)]
struct Round {
    pushed: usize,
    outcomes: Vec<Outcome>,
    pulled: usize,
}

/// One sync round of `edge` with `central`: push its outbox, then pull until caught up.
async fn sync(edge: &Node, central: &Node) -> Round {
    let upstream = central.id.to_string();
    let mut round = Round::default();
    loop {
        let sent = edge
            .store
            .outbox(&upstream, edge.id, 50)
            .await
            .expect("outbox");
        if sent.is_empty() {
            break;
        }
        let authority = SurrealAuthority::new(&central.store, edge.id, scope());
        let mut answers = Vec::new();
        for outgoing in &sent {
            round.pushed += outgoing.group.items.len();
            answers.push(
                receive(&authority, &outgoing.group)
                    .await
                    .expect("receive")
                    .outcome,
            );
        }
        edge.store
            .record_push(&upstream, &sent, &answers)
            .await
            .expect("record");
        round.outcomes.extend(answers);
    }
    loop {
        let request = PullRequest {
            cursor: edge.store.checkpoint(central.id).await.expect("checkpoint"),
            lane: Lane::Clinical,
            limit: 3,
        };
        let response = central
            .store
            .pull(&scope(), central.id, edge.id, &request)
            .await
            .expect("pull");
        let more = matches!(response, PullResponse::Items { more: true, .. });
        if let PullResponse::Items { items, .. } = &response {
            round.pulled += items.len();
        }
        apply_pull(&SurrealReplica::new(&edge.store, central.id), response)
            .await
            .expect("apply");
        if !more {
            break;
        }
    }
    round
}

#[tokio::test]
async fn edge_and_central_converge_after_a_partition() {
    let (central, edge) = pair().await;
    // Before the partition: a patient registered at central reaches the edge.
    let shared = ResourceId::new();
    write(
        &central,
        shared,
        &patient("Amina"),
        Precondition::MustNotExist,
    )
    .await;
    sync(&edge, &central).await;
    let (base, _) = head(&edge, "Patient", shared).await.expect("synced");

    // Partitioned: each side writes on its own, and the edge updates the shared patient.
    let mut at_edge = Vec::new();
    for value in 0..5 {
        let id = ResourceId::new();
        write(
            &edge,
            id,
            &observation("KLA01", value),
            Precondition::MustNotExist,
        )
        .await;
        at_edge.push(id);
    }
    write(
        &edge,
        shared,
        &patient("Amina N."),
        Precondition::IfMatch(base),
    )
    .await;
    let at_central = ResourceId::new();
    write(
        &central,
        at_central,
        &patient("Brian"),
        Precondition::MustNotExist,
    )
    .await;

    let round = sync(&edge, &central).await;
    assert_eq!(round.pushed, 6);
    assert!(round.outcomes.iter().all(|o| *o == Outcome::Accepted));
    assert_eq!(round.pulled, 1, "only what central wrote comes back");

    for (resource_type, id) in at_edge
        .iter()
        .map(|id| ("Observation", *id))
        .chain([("Patient", shared), ("Patient", at_central)])
    {
        assert_eq!(
            head(&edge, resource_type, id).await,
            head(&central, resource_type, id).await,
            "{resource_type}/{id} is the same version, with the same body, on both"
        );
    }
    let (_, amina) = head(&central, "Patient", shared).await.expect("at central");
    assert_eq!(amina["name"][0]["text"], "Amina N.");

    // Nothing echoes: a second round moves nothing either way.
    assert_eq!(sync(&edge, &central).await, Round::default());
}

#[tokio::test]
async fn a_retried_push_is_applied_once() {
    let (central, edge) = pair().await;
    write(
        &edge,
        ResourceId::new(),
        &observation("KLA01", 1),
        Precondition::MustNotExist,
    )
    .await;
    let upstream = central.id.to_string();
    let sent = edge
        .store
        .outbox(&upstream, edge.id, 50)
        .await
        .expect("outbox");
    let authority = SurrealAuthority::new(&central.store, edge.id, scope());
    // The response is lost: the edge records nothing and sends the same group again.
    let first = receive(&authority, &sent[0].group).await.expect("first");
    assert!(!first.replayed);
    let before = central.store.cursor().await.expect("cursor").seq;
    let retry = receive(&authority, &sent[0].group).await.expect("retry");
    assert!(retry.replayed);
    assert_eq!(retry.outcome, Outcome::Accepted);
    assert_eq!(central.store.cursor().await.expect("cursor").seq, before);
}

#[tokio::test]
async fn concurrent_edits_open_a_conflict_and_the_edge_keeps_its_candidate() {
    let (central, edge) = pair().await;
    let id = ResourceId::new();
    write(&central, id, &patient("Chris"), Precondition::MustNotExist).await;
    sync(&edge, &central).await;
    let (base, _) = head(&edge, "Patient", id).await.expect("synced");

    // Both edit the same version while partitioned.
    let theirs = write(
        &central,
        id,
        &patient("Chris (central)"),
        Precondition::IfMatch(base),
    )
    .await;
    let ours = write(
        &edge,
        id,
        &patient("Chris (edge)"),
        Precondition::IfMatch(base),
    )
    .await;

    let round = sync(&edge, &central).await;
    assert!(
        matches!(round.outcomes.as_slice(), [Outcome::Conflict { .. }]),
        "{:?}",
        round.outcomes
    );
    assert_eq!(central.store.conflicts().await.expect("conflicts").len(), 1);
    let (central_head, _) = head(&central, "Patient", id).await.expect("head");
    assert_eq!(
        central_head, theirs.version_id,
        "central's version stays the head"
    );
    let (edge_head, _) = head(&edge, "Patient", id).await.expect("head");
    assert_eq!(
        edge_head, theirs.version_id,
        "the edge takes central's version as its head"
    );

    // The edge's candidate is kept, not lost: still in its history, still unaccepted.
    let mut tx = edge.store.begin().await.expect("begin");
    let kept = tx
        .version(&"Patient".parse().expect("type"), id, ours.version_id)
        .await
        .expect("version");
    assert!(kept.is_some());
    drop(tx);
    // Nothing more moves until the conflict is resolved at central.
    assert_eq!(sync(&edge, &central).await, Round::default());
}

#[tokio::test]
async fn the_sync_scope_holds_on_push_and_pull() {
    let (central, edge) = pair().await;
    // Central holds another facility's observation and its own: only the edge's comes down.
    let elsewhere = ResourceId::new();
    write(
        &central,
        elsewhere,
        &observation("GUL02", 1),
        Precondition::MustNotExist,
    )
    .await;
    let here = ResourceId::new();
    write(
        &central,
        here,
        &observation("KLA01", 2),
        Precondition::MustNotExist,
    )
    .await;
    // The edge pushes an observation tagged with another facility: refused.
    let smuggled = ResourceId::new();
    write(
        &edge,
        smuggled,
        &observation("GUL02", 3),
        Precondition::MustNotExist,
    )
    .await;

    let round = sync(&edge, &central).await;
    assert!(
        matches!(
            round.outcomes.as_slice(),
            [Outcome::Rejected {
                reason: Reason::Unauthorized,
                ..
            }]
        ),
        "{:?}",
        round.outcomes
    );
    assert_eq!(round.pulled, 1);
    assert!(head(&edge, "Observation", here).await.is_some());
    assert!(head(&edge, "Observation", elsewhere).await.is_none());
    assert!(head(&central, "Observation", smuggled).await.is_none());
}

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
    Connection, DOWN_GLOBAL, MIGRATIONS, Resolution, Settings, SurrealAuthority, SurrealReplica,
    SurrealStore, SyncScope, Target, UP_GLOBAL, migrate,
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
    seeded: usize,
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
    for lane in Lane::PULLED {
        loop {
            let request = PullRequest {
                cursor: edge
                    .store
                    .checkpoint(central.id, lane)
                    .await
                    .expect("checkpoint"),
                lane,
                limit: 3,
            };
            let response = central
                .store
                .pull(&scope(), central.id, edge.id, &request)
                .await
                .expect("pull");
            if response == PullResponse::ReseedRequired {
                // Beyond the horizon: re-seed from central's snapshot, then carry on pulling.
                let (items, cursor) = central
                    .store
                    .snapshot(&scope(), central.id)
                    .await
                    .expect("snapshot");
                round.seeded += edge
                    .store
                    .seed(central.id, &items, &cursor)
                    .await
                    .expect("seed");
                assert!(round.seeded < 1000, "a re-seed must move the checkpoint on");
                continue;
            }
            let more = matches!(response, PullResponse::Items { more: true, .. });
            if let PullResponse::Items { items, .. } = &response {
                round.pulled += items.len();
            }
            apply_pull(
                &SurrealReplica::new(&edge.store, central.id, lane),
                response,
            )
            .await
            .expect("apply");
            if !more {
                break;
            }
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

    // The Conflict Record keeps both branches, with its class and policy.
    let open = central.store.open_conflicts(None).await.expect("queue");
    assert_eq!(open.len(), 1);
    let record = &open[0];
    assert_eq!(record.class, "identity");
    assert!(record.policy.contains("never auto-merged"));
    assert!(record.candidates.iter().any(
        |item| item.op_id == ours.version_id && item.body["name"][0]["text"] == "Chris (edge)"
    ));
    // Aged more than 7 days, it would raise the alert; it is not yet.
    let week_ago = Timestamp::now() - health_time::SignedDuration::from_hours(24 * 7);
    assert!(
        central
            .store
            .open_conflicts(Some(week_ago))
            .await
            .expect("aged")
            .is_empty()
    );

    // Registry staff resolve it with a corrected version on top of the head.
    let resolved = central
        .store
        .resolve_conflict(
            &record.id,
            "registry:okello",
            Resolution::Write {
                resource_type: "Patient".parse().expect("type"),
                id,
                json: RawJson::from_validated(patient("Chris Okello").to_string()),
            },
            None,
        )
        .await
        .expect("resolve");
    assert!(
        central
            .store
            .open_conflicts(None)
            .await
            .expect("queue")
            .is_empty()
    );
    let record = central
        .store
        .conflict(&record.id)
        .await
        .expect("read")
        .expect("record");
    assert_eq!(record.status, "resolved");
    assert_eq!(record.resolved_by.as_deref(), Some("registry:okello"));
    assert_eq!(record.resolution, Some(resolved.to_string()));
    // The resolution syncs down like any other version.
    sync(&edge, &central).await;
    let (head_now, body) = head(&edge, "Patient", id).await.expect("head");
    assert_eq!(head_now, resolved);
    assert_eq!(body["name"][0]["text"], "Chris Okello");
}

fn note(status: &str, text: &str) -> Value {
    json!({
        "resourceType": "Composition",
        "status": status,
        "meta": {"tag": [{"system": "urn:health:facility", "code": "KLA01"}]},
        "title": "Consultation note",
        "section": [{"text": {"status": "generated", "div": text}}]
    })
}

#[tokio::test]
async fn a_signed_note_is_never_edited_in_place_by_sync() {
    let (central, edge) = pair().await;
    let id = ResourceId::new();
    let signed = write(
        &edge,
        id,
        &note("final", "<div>BP 120/80</div>"),
        Precondition::MustNotExist,
    )
    .await;
    sync(&edge, &central).await;

    // Edited in place at the edge: refused at central, which keeps the signed version.
    write(
        &edge,
        id,
        &note("final", "<div>BP 140/90</div>"),
        Precondition::IfMatch(signed.version_id),
    )
    .await;
    let round = sync(&edge, &central).await;
    assert!(
        matches!(
            round.outcomes.as_slice(),
            [Outcome::Rejected {
                reason: Reason::SignedVersionEdited,
                ..
            }]
        ),
        "{:?}",
        round.outcomes
    );
    let (at_central, _) = head(&central, "Composition", id).await.expect("head");
    assert_eq!(at_central, signed.version_id);

    // Marked entered-in-error as it was: accepted.
    let other = ResourceId::new();
    let first = write(
        &edge,
        other,
        &note("final", "<div>wrong patient</div>"),
        Precondition::MustNotExist,
    )
    .await;
    sync(&edge, &central).await;
    write(
        &edge,
        other,
        &note("entered-in-error", "<div>wrong patient</div>"),
        Precondition::IfMatch(first.version_id),
    )
    .await;
    let round = sync(&edge, &central).await;
    assert_eq!(round.outcomes, vec![Outcome::Accepted]);
}

#[test]
fn the_scope_lists_are_sorted_for_lookup() {
    for list in [UP_GLOBAL, DOWN_GLOBAL] {
        assert!(list.windows(2).all(|w| w[0] < w[1]), "{list:?}");
    }
}

#[tokio::test]
async fn central_audit_never_goes_down_and_registries_never_go_up() {
    let (central, edge) = pair().await;
    let audit = ResourceId::new();
    let event = json!({"resourceType": "AuditEvent", "recorded": "2026-11-08T08:00:00Z"});
    write(&central, audit, &event, Precondition::MustNotExist).await;
    let practitioner = ResourceId::new();
    let staff = json!({"resourceType": "Practitioner", "name": [{"text": "Dr Okello"}]});
    write(&central, practitioner, &staff, Precondition::MustNotExist).await;
    let forged = ResourceId::new();
    let fake = json!({"resourceType": "Organization", "name": "Not central's"});
    write(&edge, forged, &fake, Precondition::MustNotExist).await;

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
    assert!(head(&edge, "Practitioner", practitioner).await.is_some());
    assert!(head(&edge, "AuditEvent", audit).await.is_none());
    assert!(head(&central, "Organization", forged).await.is_none());
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

#[tokio::test]
async fn a_node_offline_beyond_the_horizon_reseeds_without_losing_data() {
    let (central, edge) = pair().await;
    let shared = ResourceId::new();
    write(
        &central,
        shared,
        &patient("Esther"),
        Precondition::MustNotExist,
    )
    .await;
    sync(&edge, &central).await;
    let epoch = edge.store.cursor().await.expect("cursor").epoch;

    // The edge is offline for longer than central keeps its log: it records an observation, and
    // central registers a patient and prunes its log past the edge's cursor.
    let offline = ResourceId::new();
    write(
        &edge,
        offline,
        &observation("KLA01", 7),
        Precondition::MustNotExist,
    )
    .await;
    let later = ResourceId::new();
    write(
        &central,
        later,
        &patient("Faith"),
        Precondition::MustNotExist,
    )
    .await;
    let horizon = central
        .store
        .prune_log(Timestamp::now() + health_time::SignedDuration::from_mins(1))
        .await
        .expect("prune");
    assert!(horizon >= 2, "pruned through {horizon}");
    // The log never hands out a seq again, even pruned to empty.
    assert!(central.store.cursor().await.expect("cursor").seq >= horizon);

    let round = sync(&edge, &central).await;
    assert_eq!(
        round.outcomes,
        vec![Outcome::Accepted],
        "the outbox survives and goes up"
    );
    assert!(round.seeded >= 1, "{round:?}");
    for (resource_type, id) in [
        ("Patient", shared),
        ("Patient", later),
        ("Observation", offline),
    ] {
        assert_eq!(
            head(&edge, resource_type, id).await,
            head(&central, resource_type, id).await,
            "{resource_type}/{id}"
        );
    }
    assert_ne!(
        edge.store.cursor().await.expect("cursor").epoch,
        epoch,
        "a re-seeded node takes a new epoch"
    );
    // Caught up again: nothing moves.
    assert_eq!(sync(&edge, &central).await, Round::default());
}

#[tokio::test]
async fn packages_arrive_before_the_clinical_data_written_before_them() {
    let (central, edge) = pair().await;
    let reading = ResourceId::new();
    write(
        &central,
        reading,
        &observation("KLA01", 5),
        Precondition::MustNotExist,
    )
    .await;
    let value_set = ResourceId::new();
    write(
        &central,
        value_set,
        &json!({"resourceType": "ValueSet", "status": "active", "url": "urn:test:vs"}),
        Precondition::MustNotExist,
    )
    .await;

    sync(&edge, &central).await;
    // The edge applied the packages lane first: the ValueSet sits earlier in its own log.
    let applied = edge.store.changes_after(0, 10).await.expect("log");
    let order: Vec<String> = applied
        .iter()
        .map(|c| c.resource_type.to_string())
        .collect();
    assert_eq!(order, vec!["ValueSet".to_owned(), "Observation".to_owned()]);
    // Each lane has its own cursor, at the end of central's log.
    let end = central.store.cursor().await.expect("cursor").seq;
    for lane in Lane::PULLED {
        let cursor = edge
            .store
            .checkpoint(central.id, lane)
            .await
            .expect("checkpoint");
        assert_eq!(cursor.map(|c| c.seq), Some(end), "{lane:?}");
    }
}

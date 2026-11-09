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

/// A visit at `facility` for `patient`, written at `node`: what ties a patient to a facility, so
/// their golden record goes down to it (golden-record narrowing).
async fn visit(node: &Node, patient: ResourceId, facility: &str) {
    let body = json!({
        "resourceType": "Encounter", "status": "planned",
        "meta": {"tag": [{"system": "urn:health:facility", "code": facility}]},
        "subject": {"reference": format!("Patient/{patient}")}
    });
    write(node, ResourceId::new(), &body, Precondition::MustNotExist).await;
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
    visit(&central, shared, "KLA01").await;
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
    visit(&central, at_central, "KLA01").await;

    let round = sync(&edge, &central).await;
    assert_eq!(round.pushed, 6);
    assert!(round.outcomes.iter().all(|o| *o == Outcome::Accepted));
    assert_eq!(
        round.pulled, 2,
        "only what central wrote comes back: a patient and their visit"
    );

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
    visit(&central, id, "KLA01").await;
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
    assert_eq!(
        edge.store.unaccepted().await.expect("states"),
        vec![(ours.version_id.to_string(), "conflict".to_owned())],
        "the user sees the record as not accepted"
    );

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
    // The resolution syncs down like any other version, and the edge stops holding its
    // candidate: the record is accepted as resolved (synchronization.md § Item states).
    sync(&edge, &central).await;
    let (head_now, body) = head(&edge, "Patient", id).await.expect("head");
    assert_eq!(head_now, resolved);
    assert_eq!(body["name"][0]["text"], "Chris Okello");
    assert!(edge.store.unaccepted().await.expect("states").is_empty());
    // Another node is never sent it.
    let another = NodeId::new();
    let PullResponse::Items { items, .. } = central
        .store
        .pull(
            &scope(),
            central.id,
            another,
            &PullRequest {
                cursor: None,
                lane: Lane::Clinical,
                limit: 500,
            },
        )
        .await
        .expect("pull")
    else {
        panic!("items")
    };
    assert!(items.iter().all(|i| i.item.resource_type != "resolution"));
    assert!(
        items.iter().any(|i| i.item.resource_id == id.to_string()),
        "while the record itself is"
    );
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
    visit(&central, shared, "KLA01").await;
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
    visit(&central, later, "KLA01").await;
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

#[tokio::test]
async fn the_facility_stock_ledger_goes_up_as_operations_applied_once() {
    use health_inventory::{
        Movement, MovementKind, Origin, Ownership, Policy, Status, StockStore as _, StockUnit,
    };
    use health_inventory_app::Inventory;
    let (central, edge) = pair().await;
    let unit = StockUnit {
        store: "Location/pharmacy".into(),
        product: "amoxicillin-500".into(),
        batch: "B42".into(),
        expiry: "2027-06-30".into(),
        funding_source: "government".into(),
        ownership: Ownership::Owned,
        status: Status::Usable,
    };
    let movement = |id: &str, kind: MovementKind, quantity: i64| {
        let at: Timestamp = "2026-11-09T09:00:00Z".parse().expect("time");
        Movement {
            id: id.into(),
            unit: unit.clone(),
            kind,
            quantity,
            pack_size: 10,
            unit_cost: None,
            occurred_at: at,
            recorded_at: at,
            source_document: "doc".into(),
            actor: "pharmacist".into(),
            node: "E1".into(),
            op_id: id.into(),
            reason: None,
            reverses: None,
            emergency: false,
        }
    };
    let at_edge = Inventory::new(edge.store.clone(), Policy::default());
    let authority = Origin::Authority {
        may_emergency_dispense: false,
    };
    at_edge
        .record(&movement("grn-1", MovementKind::Receipt, 100), authority)
        .await
        .expect("receipt");
    at_edge
        .record(&movement("disp-1", MovementKind::Dispense, -30), authority)
        .await
        .expect("dispense");

    // The pushed groups, kept to resend after a lost response.
    let sent = edge
        .store
        .outbox(&central.id.to_string(), edge.id, 50)
        .await
        .expect("outbox");
    assert!(sent.iter().all(|o| {
        o.group
            .items
            .iter()
            .all(|i| i.kind == health_sync_protocol::Kind::Op)
    }));
    let round = sync(&edge, &central).await;
    assert_eq!(round.outcomes, vec![Outcome::Accepted, Outcome::Accepted]);
    let authority_central = SurrealAuthority::new(&central.store, edge.id, scope());
    for outgoing in &sent {
        let again = receive(&authority_central, &outgoing.group)
            .await
            .expect("resend");
        assert!(again.replayed, "answered from the receipt");
    }

    // Central's copy of the facility's ledger: both movements, once, and the same balance.
    let mut tx = central.store.begin().await.expect("begin");
    let movements = tx.movements(&unit.key()).await.expect("movements");
    let balance = tx
        .stock_balance(&unit.key())
        .await
        .expect("balance")
        .expect("unit");
    drop(tx);
    assert_eq!(movements.len(), 2);
    assert_eq!(balance.on_hand, 70);
    assert!(!balance.flagged);
    // Operations never come back down.
    assert_eq!(sync(&edge, &central).await, Round::default());
}

#[tokio::test]
async fn charges_go_up_as_operations_and_central_follows_their_changes() {
    use health_billing::{Charge, ChargeStatus, ChargeStore as _};
    let (central, edge) = pair().await;
    let charge = Charge {
        id: "ch-1".into(),
        trigger: "consultation".into(),
        event: "Encounter/e-1".into(),
        event_item: "consult".into(),
        patient: "Patient/p-1".into(),
        item: "opd-consultation".into(),
        quantity: rust_decimal::Decimal::ONE,
        at: "2026-11-09T09:00:00Z".parse().expect("time"),
        account: "Account/a-1".into(),
        service_point: None,
        free_care: None,
        status: ChargeStatus::Billable,
        price: None,
        cancelled: None,
    };
    let mut tx = edge.store.begin().await.expect("begin");
    assert!(tx.add_charge(&charge).await.expect("add"));
    tx.commit().await.expect("commit");
    // Billed later at the facility: the change follows the charge up.
    let mut tx = edge.store.begin().await.expect("begin");
    tx.put_charge(&Charge {
        status: ChargeStatus::Billed,
        ..charge.clone()
    })
    .await
    .expect("bill");
    tx.commit().await.expect("commit");

    let round = sync(&edge, &central).await;
    assert_eq!(round.outcomes, vec![Outcome::Accepted, Outcome::Accepted]);
    let mut tx = central.store.begin().await.expect("begin");
    let at_central = tx.charges_of_account("Account/a-1").await.expect("charges");
    drop(tx);
    assert_eq!(at_central.len(), 1, "one charge, not two");
    assert_eq!(
        at_central[0].status,
        ChargeStatus::Billed,
        "in commit order"
    );
    assert_eq!(sync(&edge, &central).await, Round::default());
}

#[tokio::test]
async fn a_facility_cannot_send_operations_on_another_facility_s_ledgers() {
    use health_billing::{Charge, ChargeStatus, ChargeStore as _};
    use health_inventory::{Movement, MovementKind, Origin, Ownership, Policy, Status, StockUnit};
    use health_inventory_app::Inventory;
    let (central, kla) = pair().await;
    let gul = node("2026-11-09T09:00:00Z").await;
    gul.store
        .add_upstream(&central.id.to_string())
        .await
        .expect("upstream");
    let movement = |id: &str| Movement {
        id: id.into(),
        unit: StockUnit {
            store: "Location/kla-store".into(),
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
        occurred_at: "2026-11-09T09:00:00Z".parse().expect("time"),
        recorded_at: "2026-11-09T09:00:00Z".parse().expect("time"),
        source_document: "grn".into(),
        actor: "storekeeper".into(),
        node: "E1".into(),
        op_id: id.into(),
        reason: None,
        reverses: None,
        emergency: false,
    };
    let authority = Origin::Authority {
        may_emergency_dispense: false,
    };
    // KLA01's store and encounter reach central first.
    Inventory::new(kla.store.clone(), Policy::default())
        .record(&movement("kla-grn"), authority)
        .await
        .expect("kla receipt");
    let encounter = ResourceId::new();
    write(
        &kla,
        encounter,
        &json!({
            "resourceType": "Encounter", "status": "in-progress",
            "meta": {"tag": [{"system": "urn:health:facility", "code": "KLA01"}]}
        }),
        Precondition::MustNotExist,
    )
    .await;
    assert!(
        sync(&kla, &central)
            .await
            .outcomes
            .iter()
            .all(|o| *o == Outcome::Accepted)
    );

    // GUL02 sends a movement on KLA01's store and a charge on KLA01's encounter: both refused.
    Inventory::new(gul.store.clone(), Policy::default())
        .record(&movement("gul-grn"), authority)
        .await
        .expect("gul receipt");
    let mut tx = gul.store.begin().await.expect("begin");
    tx.add_charge(&Charge {
        id: "gul-charge".into(),
        trigger: "consultation".into(),
        event: format!("Encounter/{encounter}"),
        event_item: "consult".into(),
        patient: "Patient/p-1".into(),
        item: "opd-consultation".into(),
        quantity: rust_decimal::Decimal::ONE,
        at: "2026-11-09T09:00:00Z".parse().expect("time"),
        account: "Account/gul-a".into(),
        service_point: None,
        free_care: None,
        status: ChargeStatus::Billable,
        price: None,
        cancelled: None,
    })
    .await
    .expect("charge");
    tx.commit().await.expect("commit");
    let sent = gul
        .store
        .outbox(&central.id.to_string(), gul.id, 50)
        .await
        .expect("outbox");
    let authority_gul =
        SurrealAuthority::new(&central.store, gul.id, SyncScope::Facility("GUL02".into()));
    let mut refusals = Vec::new();
    for outgoing in &sent {
        let result = receive(&authority_gul, &outgoing.group)
            .await
            .expect("receive");
        if let Outcome::Rejected { detail, .. } = result.outcome {
            refusals.push(detail);
        }
    }
    assert_eq!(refusals.len(), 2, "{refusals:?}");
    assert!(
        refusals[0].contains("belongs to facility KLA01"),
        "{}",
        refusals[0]
    );
    assert!(
        refusals[1].contains("another facility's"),
        "{}",
        refusals[1]
    );
}

#[tokio::test]
async fn an_edge_receives_golden_records_only_for_patients_with_records_there() {
    let (central, edge) = pair().await;
    // Seen at this facility, and seen only elsewhere.
    let ours = ResourceId::new();
    write(
        &central,
        ours,
        &patient("Grace"),
        Precondition::MustNotExist,
    )
    .await;
    visit(&central, ours, "KLA01").await;
    let theirs = ResourceId::new();
    write(
        &central,
        theirs,
        &patient("Hope"),
        Precondition::MustNotExist,
    )
    .await;
    visit(&central, theirs, "GUL02").await;
    // Their relatives follow them.
    let relative = |patient: ResourceId| json!({"resourceType": "RelatedPerson", "patient": {"reference": format!("Patient/{patient}")}});
    let our_relative = ResourceId::new();
    write(
        &central,
        our_relative,
        &relative(ours),
        Precondition::MustNotExist,
    )
    .await;
    let their_relative = ResourceId::new();
    write(
        &central,
        their_relative,
        &relative(theirs),
        Precondition::MustNotExist,
    )
    .await;
    // A patient the edge registers itself comes back after central's golden-record work.
    let registered = ResourceId::new();
    write(
        &edge,
        registered,
        &patient("Ivy"),
        Precondition::MustNotExist,
    )
    .await;

    sync(&edge, &central).await;
    assert!(head(&edge, "Patient", ours).await.is_some());
    assert!(head(&edge, "RelatedPerson", our_relative).await.is_some());
    assert!(
        head(&edge, "Patient", theirs).await.is_none(),
        "another facility's patient"
    );
    assert!(head(&edge, "RelatedPerson", their_relative).await.is_none());

    // Central updates the edge's own registration: the update comes down.
    let (version, _) = head(&central, "Patient", registered)
        .await
        .expect("at central");
    write(
        &central,
        registered,
        &patient("Ivy Akello"),
        Precondition::IfMatch(version),
    )
    .await;
    sync(&edge, &central).await;
    let (_, body) = head(&edge, "Patient", registered)
        .await
        .expect("at the edge");
    assert_eq!(body["name"][0]["text"], "Ivy Akello");

    // A re-seed narrows the same way.
    let (items, _) = central
        .store
        .snapshot(&scope(), central.id)
        .await
        .expect("snapshot");
    let patients: Vec<&str> = items
        .iter()
        .filter(|i| i.resource_type == "Patient")
        .map(|i| i.resource_id.as_str())
        .collect();
    assert!(patients.contains(&ours.to_string().as_str()));
    assert!(!patients.contains(&theirs.to_string().as_str()));
}

#[tokio::test]
async fn central_checks_the_payments_and_postings_a_facility_sends() {
    use health_financial_ledger::{
        Currency, Dimensions, Line, Money, Posting, PostingStore as _, Side,
    };
    use health_payments::{Evidence, Method, Payment, PaymentStore as _, Purpose, Status};
    use rust_decimal::Decimal;
    let (central, kla) = pair().await;
    let ugx = |n: i64| Money::new(Decimal::from(n), Currency::from_code("UGX").expect("UGX"));
    let line = |account: &str, side: Side, amount: i64, facility: &str| Line {
        account: account.into(),
        side,
        amount: ugx(amount).expect("money"),
        dimensions: Dimensions {
            facility: facility.into(),
            ..Dimensions::default()
        },
        original: None,
    };
    let posting = |id: &str, lines: Vec<Line>, reverses: Option<&str>| Posting {
        id: id.into(),
        at: "2026-11-09T09:00:00Z".parse().expect("time"),
        description: id.into(),
        lines,
        reverses: reverses.map(Into::into),
    };
    let visit = |id: &str, facility: &str| {
        posting(
            id,
            vec![
                line("receivable/patient", Side::Debit, 5000, facility),
                line("revenue/consultation", Side::Credit, 5000, facility),
            ],
            None,
        )
    };
    let reversal = |id: &str, of: &str| {
        posting(
            id,
            vec![
                line("receivable/patient", Side::Credit, 5000, "KLA01"),
                line("revenue/consultation", Side::Debit, 5000, "KLA01"),
            ],
            Some(of),
        )
    };
    let payment = |id: &str, reference: &str, status: Status, evidence: Option<Evidence>| Payment {
        id: id.into(),
        method: Method::MtnMomo,
        purpose: Purpose::Settlement,
        amount: Decimal::from(5000),
        payer: "Account/a-1".into(),
        reference: Some(reference.into()),
        shift: None,
        status,
        recorded_at: "2026-11-09T09:00:00Z".parse().expect("time"),
        expires_at: None,
        evidence,
        reason: None,
        refunded: Decimal::ZERO,
        receipt: None,
    };
    // Written straight to the facility's store, past its own rules, as a faulty or tampered node
    // could; each in its own transaction, since a group is refused whole.
    enum Op {
        Post(Posting),
        Pay(Payment),
    }
    let mut unbalanced = visit("unbalanced", "KLA01");
    unbalanced.lines[1].amount = ugx(4000).expect("money");
    let callback = Some(Evidence::Callback);
    for op in [
        Op::Post(visit("good", "KLA01")),
        Op::Post(unbalanced),
        Op::Post(visit("elsewhere", "GUL02")),
        Op::Post(reversal("undo", "good")),
        Op::Post(reversal("undo-again", "good")),
        Op::Post(reversal("undo-nothing", "never-sent")),
        Op::Pay(payment("pay-1", "mtn-1", Status::Pending, None)),
        Op::Pay(payment("pay-2", "mtn-2", Status::Posted, None)),
        Op::Pay(payment("pay-3", "mtn-1", Status::Posted, callback)),
        // Confirmed later, then edited after posting.
        Op::Pay(payment("pay-1", "mtn-1", Status::Posted, callback)),
        Op::Pay(Payment {
            amount: Decimal::from(9000),
            ..payment("pay-1", "mtn-1", Status::Posted, callback)
        }),
    ] {
        let mut tx = kla.store.begin().await.expect("begin");
        match op {
            Op::Post(posting) => tx.append_posting(&posting).await.expect("post"),
            Op::Pay(payment) => tx.put_payment(&payment).await.expect("pay"),
        }
        tx.commit().await.expect("commit");
    }
    let sent = kla
        .store
        .outbox(&central.id.to_string(), kla.id, 50)
        .await
        .expect("outbox");
    let authority = SurrealAuthority::new(&central.store, kla.id, scope());
    let mut refusals = Vec::new();
    for outgoing in &sent {
        let result = receive(&authority, &outgoing.group).await.expect("receive");
        if let Outcome::Rejected { detail, .. } = result.outcome {
            refusals.push(detail);
        }
    }
    let refused = |text: &str| refusals.iter().any(|r| r.contains(text));
    assert!(refused("does not balance"), "{refusals:?}");
    assert!(
        refused("posting elsewhere has a line on facility GUL02"),
        "{refusals:?}"
    );
    assert!(refused("posting good is already reversed"), "{refusals:?}");
    assert!(
        refused("posting never-sent is not held here"),
        "{refusals:?}"
    );
    assert!(
        refused("payment pay-2: a posted MtnMomo payment names its provider evidence"),
        "{refusals:?}"
    );
    assert!(
        refused("provider reference mtn-1 is payment pay-1's already"),
        "{refusals:?}"
    );
    assert!(
        refused("payment pay-1: a payment is reversed, never edited: its amount changed"),
        "{refusals:?}"
    );
    assert_eq!(refusals.len(), 7, "{refusals:?}");

    let mut tx = central.store.begin().await.expect("begin");
    let postings: Vec<String> = tx
        .postings()
        .await
        .expect("postings")
        .into_iter()
        .map(|p| p.id)
        .collect();
    assert_eq!(postings, vec!["good".to_owned(), "undo".to_owned()]);
    let pay_1 = tx.payment("pay-1").await.expect("read").expect("held");
    assert_eq!(pay_1.status, Status::Posted, "confirmed");
    assert_eq!(pay_1.amount, Decimal::from(5000), "not edited");
    assert!(tx.payment("pay-2").await.expect("read").is_none());
    assert!(tx.payment("pay-3").await.expect("read").is_none());
}

#[tokio::test]
async fn activation_records_go_down_to_every_node_and_never_up() {
    let (central, edge) = pair().await;
    let record = |id: &str, scope: Value| {
        json!({
            "activation": {
                "id": id, "package": "ug.moh.anc", "version": "1.0.0", "scope": scope,
                "effectiveAt": "2026-11-09T09:00:00Z", "issuedAt": "2026-11-09T08:00:00Z",
                "by": "ops"
            },
            "digest": "ab".repeat(32),
            "signature": "header..sig"
        })
    };
    let mut tx = central.store.begin().await.expect("begin");
    for (id, scope) in [
        ("act-1", json!({"level": "national"})),
        // Another facility's: sent all the same; the node applies what its chain covers.
        ("act-2", json!({"level": "facility", "facility": "GUL02"})),
    ] {
        let body = record(id, scope);
        assert!(
            tx.put_package_activation(id, "ug.moh.anc", "2026-11-09T08:00:00Z", &body.to_string())
                .await
                .expect("put")
        );
    }
    // Kept once: a second put of the same record is a no-op.
    assert!(
        !tx.put_package_activation("act-1", "ug.moh.anc", "2026-11-09T08:00:00Z", "{}")
            .await
            .expect("again")
    );
    tx.commit().await.expect("commit");

    let round = sync(&edge, &central).await;
    assert_eq!(round.pulled, 2, "{round:?}");
    let mut tx = edge.store.begin().await.expect("begin");
    let held = tx.package_activations().await.expect("held");
    drop(tx);
    assert_eq!(held.len(), 2);
    let first: Value = serde_json::from_str(&held[0]).expect("json");
    assert_eq!(first, record("act-1", json!({"level": "national"})));
    // Received from central, so not queued back to it; a second round brings nothing.
    assert_eq!(sync(&edge, &central).await, Round::default());

    // An activation an edge makes is never pushed up.
    let mut tx = edge.store.begin().await.expect("begin");
    let forged = record("act-3", json!({"level": "national"}));
    tx.put_package_activation(
        "act-3",
        "ug.moh.anc",
        "2026-11-09T08:00:00Z",
        &forged.to_string(),
    )
    .await
    .expect("put");
    tx.commit().await.expect("commit");
    let round = sync(&edge, &central).await;
    assert!(
        matches!(round.outcomes.as_slice(), [Outcome::Rejected { .. }]),
        "{round:?}"
    );
    let mut tx = central.store.begin().await.expect("begin");
    assert_eq!(tx.package_activations().await.expect("held").len(), 2);
    drop(tx);

    // A re-seed carries them, and seeding a node that holds them already changes nothing.
    let (items, cursor) = central
        .store
        .snapshot(&scope(), central.id)
        .await
        .expect("snapshot");
    let activations: Vec<_> = items
        .iter()
        .filter(|i| i.resource_type == "activation")
        .collect();
    assert_eq!(activations.len(), 2);
    let fresh = node("2026-11-09T10:00:00Z").await;
    fresh
        .store
        .seed(central.id, &items, &cursor)
        .await
        .expect("seed");
    let mut tx = fresh.store.begin().await.expect("begin");
    assert_eq!(tx.package_activations().await.expect("seeded").len(), 2);
}

#[tokio::test]
async fn an_allowed_transition_commutes_with_a_concurrent_edit_and_a_competing_one_does_not() {
    let (central, edge) = pair().await;
    let encounter = |status: &str, note: Option<&str>, end: Option<&str>| {
        let mut body = json!({
            "resourceType": "Encounter", "status": status,
            "meta": {"tag": [{"system": "urn:health:facility", "code": "KLA01"}]},
            "actualPeriod": {"start": "2026-11-10T08:00:00Z"}
        });
        if let Some(note) = note {
            body["serviceType"] = json!([{ "concept": { "text": note } }]);
        }
        if let Some(end) = end {
            body["actualPeriod"]["end"] = json!(end);
        }
        body
    };
    let id = ResourceId::new();
    write(
        &edge,
        id,
        &encounter("in-progress", None, None),
        Precondition::MustNotExist,
    )
    .await;
    sync(&edge, &central).await;
    let (base, _) = head(&central, "Encounter", id).await.expect("synced");

    // Central records the service while the edge, offline, completes the visit.
    write(
        &central,
        id,
        &encounter("in-progress", Some("General OPD"), None),
        Precondition::IfMatch(base),
    )
    .await;
    write(
        &edge,
        id,
        &encounter("completed", None, Some("2026-11-10T09:30:00Z")),
        Precondition::IfMatch(base),
    )
    .await;
    let round = sync(&edge, &central).await;
    assert_eq!(
        round.outcomes,
        vec![Outcome::Accepted],
        "no Conflict Record"
    );
    assert!(
        central
            .store
            .conflicts()
            .await
            .expect("conflicts")
            .is_empty()
    );
    let (merged_id, merged) = head(&central, "Encounter", id).await.expect("head");
    assert_eq!(merged["status"], "completed");
    assert_eq!(merged["actualPeriod"]["end"], "2026-11-10T09:30:00Z");
    assert_eq!(
        merged["serviceType"][0]["concept"]["text"], "General OPD",
        "central's edit is kept"
    );
    // The merge comes down to the edge as its head.
    sync(&edge, &central).await;
    assert_eq!(
        head(&edge, "Encounter", id).await.expect("head").0,
        merged_id
    );
    assert!(edge.store.unaccepted().await.expect("states").is_empty());

    // Both moving the status from the same version is a Conflict Record, never a winner.
    let other = ResourceId::new();
    write(
        &edge,
        other,
        &encounter("in-progress", None, None),
        Precondition::MustNotExist,
    )
    .await;
    sync(&edge, &central).await;
    let (other_base, _) = head(&central, "Encounter", other).await.expect("synced");
    write(
        &central,
        other,
        &encounter("on-hold", None, None),
        Precondition::IfMatch(other_base),
    )
    .await;
    write(
        &edge,
        other,
        &encounter("discontinued", None, Some("2026-11-10T10:00:00Z")),
        Precondition::IfMatch(other_base),
    )
    .await;
    let round = sync(&edge, &central).await;
    assert!(
        matches!(round.outcomes.as_slice(), [Outcome::Conflict { .. }]),
        "{:?}",
        round.outcomes
    );
}

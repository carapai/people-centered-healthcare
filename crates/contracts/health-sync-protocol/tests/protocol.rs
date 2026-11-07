//! The sync/v1 contract: the envelope, the handshake, push grouping, the receiver's idempotency,
//! the pull rule and item states (synchronization.md).

use std::collections::{BTreeSet, HashMap};
use std::sync::Mutex;

use health_identifiers::{NodeId, OpId, ResourceId};
use health_sync_protocol::{
    Answer, ApplyDecision, Authority, Compatibility, ContentHash, Cursor, Event, Group, Hello, Hlc,
    Incompatibility, Item, ItemState, Kind, Local, MAX_BATCH_ITEMS, Outcome, PROTOCOL, PackageRef,
    PullError, PullResponse, Pulled, Reason, Receipt, Replica, SoftwareVersion, Verdict,
    apply_pull, batches, decide, expected_cursor, from_ndjson, group, receive, to_ndjson,
};
use health_time::Timestamp;
use serde_json::{Value, json};

fn version(tx_id: OpId, body: Value) -> Item {
    Item {
        kind: Kind::Version,
        op_id: OpId::new(),
        tx_id,
        resource_type: "Observation".into(),
        resource_id: ResourceId::new().to_string(),
        parent_version_id: None,
        origin_node: NodeId::new(),
        device_id: None,
        author: None,
        hlc: Hlc::default(),
        content_hash: ContentHash::of(&body),
        packages: Vec::new(),
        snapshot_version: 1,
        provisional: false,
        deleted: false,
        blobs: Vec::new(),
        body,
    }
}

fn op(tx_id: OpId) -> Item {
    let body = json!({"type": "dispense", "quantity": 2});
    Item {
        kind: Kind::Op,
        resource_type: "stock".into(),
        resource_id: "KLA01|pharmacy".into(),
        ..version(tx_id, body)
    }
}

fn group_of(items: Vec<Item>) -> Group {
    Group {
        tx_id: items[0].tx_id,
        items,
    }
}

#[test]
fn the_content_hash_ignores_key_order_and_whitespace_but_not_content() {
    let a: Value =
        serde_json::from_str(r#"{"b": [1, {"y": 2, "x": "é"}], "a": true}"#).expect("ok");
    let b: Value = serde_json::from_str(r#"{"a":true,"b":[1,{"x":"é","y":2}]}"#).expect("ok");
    let c = json!({"a": true, "b": [1, {"x": "é", "y": 3}]});
    assert_eq!(ContentHash::of(&a), ContentHash::of(&b));
    assert_ne!(ContentHash::of(&a), ContentHash::of(&c));

    let text = ContentHash::of(&a).to_string();
    assert_eq!(text.len(), 64);
    assert_eq!(
        text.parse::<ContentHash>().expect("ok"),
        ContentHash::of(&a)
    );
    assert!(text.to_uppercase().parse::<ContentHash>().is_err());
    assert_eq!(
        ContentHash::of_bytes(b"").to_string(),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

#[test]
fn items_round_trip_through_ndjson_with_their_kind() {
    let tx = OpId::new();
    let items = vec![version(tx, json!({"resourceType": "Observation"})), op(tx)];
    let body = to_ndjson(&items).expect("ok");
    let text = String::from_utf8(body.clone()).expect("ok");
    assert_eq!(text.lines().count(), 2);
    assert!(text.contains(r#""kind":"version""#) && text.contains(r#""kind":"op""#));
    assert!(!text.contains("provisional"), "false flags are left out");

    let mut padded = body;
    padded.extend_from_slice(b"\n  \n");
    let back: Vec<Item> = from_ndjson(&padded).expect("ok");
    assert_eq!(back, items);
    for item in &back {
        item.check().expect("ok");
    }
    let error = from_ndjson::<Item>(b"{}\nnot json\n").expect_err("an error");
    assert_eq!(error.line, 1);
}

#[test]
fn a_malformed_item_fails_its_check() {
    let tx = OpId::new();
    let mut tampered = version(tx, json!({"value": 1}));
    tampered.body = json!({"value": 2});
    assert!(tampered.check().is_err());

    let mut not_a_type = version(tx, json!({}));
    not_a_type.resource_type = "observation".into();
    assert!(not_a_type.check().is_err());

    let mut parented_op = op(tx);
    parented_op.parent_version_id = Some(OpId::new());
    assert!(parented_op.check().is_err());
}

fn hello(protocol_major: u16, software: &str, packages: &[(&str, &str)]) -> Hello {
    Hello {
        protocol: health_sync_protocol::ProtocolVersion {
            major: protocol_major,
            minor: 3,
        },
        software: software.parse().expect("ok"),
        node: NodeId::new(),
        epoch: "e1".into(),
        packages: packages
            .iter()
            .map(|(name, version)| PackageRef {
                name: (*name).into(),
                version: (*version).into(),
            })
            .collect(),
        snapshot_version: 4,
        cursor: None,
    }
}

#[test]
fn the_handshake_refuses_another_major_and_quarantines_an_out_of_range_peer() {
    let upstream = Compatibility {
        protocol: PROTOCOL,
        software: "1.4.0".parse().expect("ok"),
        approved: BTreeSet::from([PackageRef {
            name: "core".into(),
            version: "2.0.0".into(),
        }]),
    };
    assert_eq!(
        upstream.judge(&hello(1, "1.4.2", &[("core", "2.0.0")])),
        Verdict::Compatible
    );
    assert_eq!(upstream.judge(&hello(1, "1.3.9", &[])), Verdict::Compatible);
    assert!(matches!(
        upstream.judge(&hello(2, "1.4.0", &[])),
        Verdict::Refused {
            reason: Incompatibility::Protocol { .. }
        }
    ));
    let Verdict::Quarantined { reasons } = upstream.judge(&hello(1, "1.2.0", &[("core", "1.9.0")]))
    else {
        panic!("quarantined")
    };
    assert!(matches!(reasons[0], Incompatibility::Software { .. }));
    assert!(matches!(reasons[1], Incompatibility::Package { .. }));
    assert!(matches!(
        upstream.judge(&hello(1, "1.5.0", &[])),
        Verdict::Quarantined { .. }
    ));
    assert!("1.4".parse::<SoftwareVersion>().is_err());
}

#[test]
fn a_new_epoch_discards_the_saved_cursor() {
    let node = NodeId::new();
    let saved = Cursor {
        node,
        epoch: "e1".into(),
        seq: 40,
    };
    assert_eq!(
        expected_cursor(Some(saved.clone()), node, "e1"),
        Some(saved.clone())
    );
    assert_eq!(expected_cursor(Some(saved.clone()), node, "e2"), None);
    assert_eq!(expected_cursor(Some(saved), NodeId::new(), "e1"), None);
}

#[test]
fn push_groups_by_transaction_and_never_splits_a_group_across_batches() {
    let (t1, t2) = (OpId::new(), OpId::new());
    let items = vec![
        version(t1, json!({})),
        version(t1, json!({})),
        version(t2, json!({})),
    ];
    let groups = group(items.clone()).expect("ok");
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].items.len(), 2);

    let split = vec![items[0].clone(), items[2].clone(), items[1].clone()];
    assert!(group(split).is_err());

    let big = OpId::new();
    let mut many: Vec<Group> = vec![group_of(
        (0..MAX_BATCH_ITEMS - 1)
            .map(|_| version(big, json!({})))
            .collect(),
    )];
    let pair = OpId::new();
    many.push(group_of(vec![
        version(pair, json!({})),
        version(pair, json!({})),
    ]));
    let huge = OpId::new();
    many.push(group_of(
        (0..MAX_BATCH_ITEMS + 10)
            .map(|_| version(huge, json!({})))
            .collect(),
    ));
    let sizes: Vec<Vec<usize>> = batches(many)
        .iter()
        .map(|b| b.iter().map(|g| g.items.len()).collect())
        .collect();
    assert_eq!(
        sizes,
        vec![
            vec![MAX_BATCH_ITEMS - 1],
            vec![2],
            vec![MAX_BATCH_ITEMS + 10]
        ]
    );
}

/// An authority that accepts every new group and counts what it applies.
#[derive(Default)]
struct Central {
    receipts: Mutex<HashMap<OpId, Receipt>>,
    applied: Mutex<usize>,
}

impl Authority for Central {
    type Error = ();

    async fn receipts(&self, op_ids: &[OpId]) -> Result<Vec<Receipt>, ()> {
        let held = self.receipts.lock().expect("lock");
        Ok(op_ids
            .iter()
            .filter_map(|id| held.get(id).cloned())
            .collect())
    }

    async fn apply(&self, group: &Group) -> Result<Outcome, ()> {
        *self.applied.lock().expect("lock") += 1;
        let mut held = self.receipts.lock().expect("lock");
        for item in &group.items {
            held.insert(
                item.op_id,
                Receipt {
                    op_id: item.op_id,
                    content_hash: item.content_hash,
                    outcome: Outcome::Accepted,
                },
            );
        }
        Ok(Outcome::Accepted)
    }
}

#[test]
fn a_retried_group_is_answered_from_its_receipts_and_applied_once() {
    let central = Central::default();
    let tx = OpId::new();
    let sent = group_of(vec![version(tx, json!({"a": 1})), op(tx)]);

    let first = pollster::block_on(receive(&central, &sent)).expect("ok");
    assert_eq!(first.outcome, Outcome::Accepted);
    assert!(!first.replayed);
    let retry = pollster::block_on(receive(&central, &sent)).expect("ok");
    assert_eq!(retry.outcome, Outcome::Accepted);
    assert!(retry.replayed);
    assert_eq!(*central.applied.lock().expect("lock"), 1);
}

#[test]
fn an_op_id_reused_with_other_content_is_rejected() {
    let central = Central::default();
    let tx = OpId::new();
    let sent = group_of(vec![version(tx, json!({"a": 1}))]);
    pollster::block_on(receive(&central, &sent)).expect("ok");

    let mut reused = sent.clone();
    reused.items[0].body = json!({"a": 2});
    reused.items[0].content_hash = ContentHash::of(&reused.items[0].body);
    let result = pollster::block_on(receive(&central, &reused)).expect("ok");
    assert!(matches!(
        result.outcome,
        Outcome::Rejected {
            reason: Reason::OpIdReused,
            ..
        }
    ));
    assert_eq!(*central.applied.lock().expect("lock"), 1);
}

#[test]
fn a_malformed_or_inconsistent_group_is_rejected_without_applying() {
    let central = Central::default();
    let tx = OpId::new();
    let mut tampered = group_of(vec![version(tx, json!({"a": 1}))]);
    tampered.items[0].body = json!({"a": 9});
    let rejected = |group: &Group| match pollster::block_on(receive(&central, group))
        .expect("ok")
        .outcome
    {
        Outcome::Rejected { reason, .. } => Some(reason),
        _ => None,
    };
    assert_eq!(rejected(&tampered), Some(Reason::Malformed));

    let known = version(tx, json!({"b": 1}));
    pollster::block_on(receive(&central, &group_of(vec![known.clone()]))).expect("ok");
    let mixed = group_of(vec![known, version(tx, json!({"c": 1}))]);
    assert_eq!(rejected(&mixed), Some(Reason::InconsistentGroup));

    let foreign = group_of(vec![
        version(tx, json!({})),
        version(OpId::new(), json!({})),
    ]);
    assert_eq!(rejected(&foreign), Some(Reason::InconsistentGroup));
    assert_eq!(*central.applied.lock().expect("lock"), 1);
}

#[test]
fn pulled_data_never_overwrites_an_unaccepted_candidate_and_never_applies_twice() {
    let tx = OpId::new();
    let pulled = version(tx, json!({}));
    let candidate = OpId::new();
    assert_eq!(decide(Local::default(), &pulled), ApplyDecision::Head);
    assert_eq!(
        decide(
            Local {
                has_item: false,
                unaccepted_candidate: Some(candidate)
            },
            &pulled
        ),
        ApplyDecision::HeadKeepingCandidate(candidate)
    );
    let stored = Local {
        has_item: true,
        unaccepted_candidate: Some(candidate),
    };
    assert_eq!(decide(stored, &pulled), ApplyDecision::Skip);
    assert_eq!(decide(Local::default(), &op(tx)), ApplyDecision::Append);
}

/// A replica with one unaccepted candidate per resource id, recording each group's checkpoint.
#[derive(Default)]
struct Edge {
    candidates: HashMap<String, OpId>,
    checkpoints: Mutex<Vec<u64>>,
}

impl Replica for Edge {
    type Error = ();

    async fn local(&self, item: &Item) -> Result<Local, ()> {
        Ok(Local {
            has_item: false,
            unaccepted_candidate: self.candidates.get(&item.resource_id).copied(),
        })
    }

    async fn apply(&self, _: &Group, _: &[ApplyDecision], checkpoint: &Cursor) -> Result<(), ()> {
        self.checkpoints.lock().expect("lock").push(checkpoint.seq);
        Ok(())
    }

    async fn advance(&self, checkpoint: &Cursor) -> Result<(), ()> {
        self.checkpoints.lock().expect("lock").push(checkpoint.seq);
        Ok(())
    }
}

#[test]
fn a_pull_applies_one_group_at_a_time_with_its_checkpoint() {
    let (t1, t2) = (OpId::new(), OpId::new());
    let first = version(t1, json!({}));
    let candidate = OpId::new();
    let edge = Edge {
        candidates: HashMap::from([(first.resource_id.clone(), candidate)]),
        ..Edge::default()
    };
    let cursor = Cursor {
        node: NodeId::new(),
        epoch: "e1".into(),
        seq: 13,
    };
    let response = PullResponse::Items {
        items: vec![
            Pulled {
                seq: 11,
                item: first,
            },
            Pulled {
                seq: 12,
                item: version(t1, json!({})),
            },
            Pulled {
                seq: 13,
                item: version(t2, json!({})),
            },
        ],
        cursor: cursor.clone(),
        more: false,
    };
    let applied = pollster::block_on(apply_pull(&edge, response)).expect("ok");
    assert_eq!(applied.groups, 2);
    assert_eq!(applied.cursor, cursor);
    assert_eq!(applied.candidates_kept, vec![candidate]);
    assert_eq!(*edge.checkpoints.lock().expect("lock"), vec![12, 13]);

    // A response that read past items the node was not sent still moves its checkpoint.
    let skipped = PullResponse::Items {
        items: Vec::new(),
        cursor: Cursor {
            seq: 20,
            ..cursor.clone()
        },
        more: false,
    };
    pollster::block_on(apply_pull(&edge, skipped)).expect("ok");
    assert_eq!(*edge.checkpoints.lock().expect("lock"), vec![12, 13, 20]);

    let reseed = pollster::block_on(apply_pull(&edge, PullResponse::ReseedRequired));
    assert!(matches!(reseed, Err(PullError::ReseedRequired)));
}

#[test]
fn item_states_follow_the_protocol() {
    let rejected = Outcome::Rejected {
        reason: Reason::Invalid,
        detail: String::new(),
    };
    let state = ItemState::Pending
        .on(Event::BatchBuilt)
        .and_then(|s| s.on(Event::TransportFailed))
        .and_then(|s| s.on(Event::BatchBuilt))
        .and_then(|s| s.on(Event::from(&rejected)))
        .and_then(|s| s.on(Event::Corrected))
        .expect("ok");
    assert_eq!(state, ItemState::Pending);
    assert_eq!(
        ItemState::Sending
            .on(Event::Answered(Answer::Conflict))
            .and_then(|s| s.on(Event::Resolved)),
        Ok(ItemState::Accepted)
    );
    assert!(ItemState::Accepted.on(Event::BatchBuilt).is_err());
    assert!(ItemState::Held.is_unaccepted());
    assert!(!ItemState::Accepted.is_unaccepted());
}

#[test]
fn the_hlc_moves_forward_past_local_and_remote_clocks() {
    let at = |ms: i64| Timestamp::from_millisecond(ms).expect("ok");
    let start = Hlc::default().tick(at(1_000));
    assert_eq!(
        start,
        Hlc {
            wall_ms: 1_000,
            logical: 0
        }
    );
    let same_ms = start.tick(at(1_000));
    assert_eq!(same_ms.logical, 1);
    let behind = same_ms.tick(at(900));
    assert!(
        behind > same_ms,
        "a clock stepping back never goes backwards"
    );

    let ahead = Hlc {
        wall_ms: 5_000,
        logical: 7,
    };
    let observed = behind.observe(ahead, at(1_100));
    assert_eq!(
        observed,
        Hlc {
            wall_ms: 5_000,
            logical: 8
        }
    );
    assert!(observed.tick(at(1_200)) > observed);
}

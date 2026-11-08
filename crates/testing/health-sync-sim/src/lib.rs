//! A multi-node sync simulation (testing.md § Offline and sync; #85): central, edges and desktops
//! as in-memory stores, each on its own clock, joined by links that can be cut. A round on a link
//! is the real protocol: the downstream's outbox goes up through `receive` over the upstream's
//! `SurrealAuthority`, then it pulls through `apply_pull` over its `SurrealReplica`, re-seeding
//! from the upstream's snapshot when told to. Restoring a node gives it a new epoch, as a restore
//! from backup does.
//!
//! The harness only moves data; scenarios (the `tests/` here) write records, cut links, let time
//! pass, and assert what each node holds.

use std::sync::Arc;

use health_identifiers::{NodeId, OpId, ResourceId};
use health_persistence::{
    Precondition, RawJson, ResourceType, Store as _, Transaction as _, VersionMeta, WriteRequest,
};
use health_surrealdb::{
    Connection, MIGRATIONS, Settings, SurrealAuthority, SurrealReplica, SurrealStore, SyncScope,
    Target, migrate, new_epoch,
};
use health_sync_protocol::{Lane, Outcome, PullRequest, PullResponse, apply_pull, receive};
use health_time::{FixedClock, SignedDuration, Timestamp};
use serde_json::Value;

/// One node: its store, on its own clock.
pub struct Node {
    /// What scenarios call it.
    pub name: &'static str,
    /// Its id on the sync protocol.
    pub id: NodeId,
    /// Its store.
    pub store: SurrealStore,
    /// Its clock, which scenarios skew and advance.
    pub clock: Arc<FixedClock>,
    connection: Connection,
}

impl Node {
    /// A new, empty node whose clock reads `at`.
    pub async fn new(name: &'static str, at: Timestamp) -> Self {
        let connection = Connection::open(&Settings {
            target: Target::Memory,
            namespace: name.into(),
            database: "sim".into(),
        })
        .await
        .unwrap_or_else(|e| panic!("{name}: {e}"));
        migrate(&connection, MIGRATIONS)
            .await
            .unwrap_or_else(|e| panic!("{name}: {e:?}"));
        let clock = Arc::new(FixedClock::new(at));
        Self {
            name,
            id: NodeId::new(),
            store: SurrealStore::new(connection.clone(), clock.clone()),
            clock,
            connection,
        }
    }

    /// Writes `body` as a version of `id`, as a user of this node would.
    pub async fn write(
        &self,
        id: ResourceId,
        body: &Value,
        precondition: Precondition,
    ) -> VersionMeta {
        let resource_type: ResourceType = body["resourceType"]
            .as_str()
            .and_then(|t| t.parse().ok())
            .unwrap_or_else(|| panic!("{}: a body without a resource type", self.name));
        let mut tx = self.store.begin().await.expect("begin");
        let meta = tx
            .write(WriteRequest {
                resource_type,
                id,
                json: RawJson::from_validated(body.to_string()),
                op_id: OpId::new(),
                precondition,
            })
            .await
            .unwrap_or_else(|e| panic!("{}: writing {id}: {e:?}", self.name));
        tx.commit().await.expect("commit");
        meta
    }

    /// Creates a record and returns its id.
    pub async fn create(&self, body: &Value) -> ResourceId {
        let id = ResourceId::new();
        self.write(id, body, Precondition::MustNotExist).await;
        id
    }

    /// The head of `resource_type/id` here: its version and body.
    pub async fn head(&self, resource_type: &str, id: ResourceId) -> Option<(OpId, Value)> {
        let mut tx = self.store.begin().await.expect("begin");
        let head = tx
            .read(&resource_type.parse().expect("type"), id)
            .await
            .expect("read")?;
        Some((
            head.version.version_id,
            serde_json::from_str(head.json.as_str()).expect("json"),
        ))
    }

    /// What a restore from backup does to the node's log: a new epoch, so every peer holding a
    /// cursor into it re-seeds.
    pub async fn restore(&self) {
        new_epoch(&self.connection).await.expect("new epoch");
    }

    /// Moves the node's clock by `by`.
    pub fn advance(&self, by: SignedDuration) {
        self.clock.advance(by);
    }
}

/// A sync link from a downstream node to its upstream, which can be cut.
pub struct Link {
    /// The downstream node's index.
    pub downstream: usize,
    /// The upstream node's index.
    pub upstream: usize,
    /// The downstream's Sync Scope at the upstream.
    pub scope: SyncScope,
    /// Whether the link carries traffic.
    pub up: bool,
}

/// What a round on a link moved.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Round {
    /// Items pushed up.
    pub pushed: usize,
    /// The upstream's answers.
    pub outcomes: Vec<Outcome>,
    /// Items pulled down.
    pub pulled: usize,
    /// Heads applied from a re-seed.
    pub seeded: usize,
}

impl Round {
    /// Whether anything moved.
    pub fn moved(&self) -> bool {
        self.pushed + self.pulled + self.seeded > 0
    }
}

/// The simulated deployment.
#[derive(Default)]
pub struct Sim {
    /// Its nodes.
    pub nodes: Vec<Node>,
    /// Its links.
    pub links: Vec<Link>,
    /// Every answer an upstream gave a push, in order.
    pub outcomes: Vec<Outcome>,
    /// Re-seeds that happened, as `(downstream, heads applied)`.
    pub reseeds: Vec<(&'static str, usize)>,
}

impl Sim {
    /// Adds a node; its index.
    pub fn add(&mut self, node: Node) -> usize {
        self.nodes.push(node);
        self.nodes.len() - 1
    }

    /// Links `downstream` to `upstream` with `scope`; the link's index.
    pub async fn connect(&mut self, downstream: usize, upstream: usize, scope: SyncScope) -> usize {
        let peer = self.nodes[upstream].id.to_string();
        self.nodes[downstream]
            .store
            .add_upstream(&peer)
            .await
            .expect("add upstream");
        self.links.push(Link {
            downstream,
            upstream,
            scope,
            up: true,
        });
        self.links.len() - 1
    }

    /// Cuts or restores a link.
    pub fn set_link(&mut self, link: usize, up: bool) {
        self.links[link].up = up;
    }

    /// One round on `link`: push the outbox, then pull until caught up, re-seeding when told
    /// to. Nothing moves on a cut link.
    pub async fn round(&mut self, link: usize) -> Round {
        let mut round = Round::default();
        let Link {
            downstream,
            upstream,
            ref scope,
            up,
        } = self.links[link];
        if !up {
            return round;
        }
        let (down, up_node) = (&self.nodes[downstream], &self.nodes[upstream]);
        let peer = up_node.id.to_string();
        loop {
            let sent = down
                .store
                .outbox(&peer, down.id, 100)
                .await
                .expect("outbox");
            if sent.is_empty() {
                break;
            }
            let authority = SurrealAuthority::new(&up_node.store, down.id, scope.clone());
            let mut answers = Vec::with_capacity(sent.len());
            for outgoing in &sent {
                round.pushed += outgoing.group.items.len();
                answers.push(
                    receive(&authority, &outgoing.group)
                        .await
                        .expect("receive")
                        .outcome,
                );
            }
            down.store
                .record_push(&peer, &sent, &answers)
                .await
                .expect("record push");
            round.outcomes.extend(answers);
        }
        for lane in Lane::PULLED {
            let replica = SurrealReplica::new(&down.store, up_node.id, lane);
            loop {
                let request = PullRequest {
                    cursor: down
                        .store
                        .checkpoint(up_node.id, lane)
                        .await
                        .expect("checkpoint"),
                    lane,
                    limit: 50,
                };
                let response = up_node
                    .store
                    .pull(scope, up_node.id, down.id, &request)
                    .await
                    .expect("pull");
                if response == PullResponse::ReseedRequired {
                    let (items, cursor) = up_node
                        .store
                        .snapshot(scope, up_node.id)
                        .await
                        .expect("snapshot");
                    let applied = down
                        .store
                        .seed(up_node.id, &items, &cursor)
                        .await
                        .expect("seed");
                    round.seeded += applied;
                    self.reseeds.push((down.name, applied));
                    assert!(
                        self.reseeds.len() < 1000,
                        "{} keeps being told to re-seed by {}",
                        down.name,
                        up_node.name
                    );
                    continue;
                }
                let more = matches!(response, PullResponse::Items { more: true, .. });
                if let PullResponse::Items { items, .. } = &response {
                    round.pulled += items.len();
                }
                apply_pull(&replica, response).await.expect("apply pull");
                if !more {
                    break;
                }
            }
        }
        self.outcomes.extend(round.outcomes.iter().cloned());
        round
    }

    /// Rounds on every live link, bottom-up then top-down, until nothing moves: what a
    /// deployment reaches once its links have been up for a while. Panics if it never settles.
    pub async fn settle(&mut self) {
        for _ in 0..20 {
            let mut moved = false;
            for link in 0..self.links.len() {
                moved |= self.round(link).await.moved();
            }
            if !moved {
                return;
            }
        }
        panic!("the deployment did not settle in 20 passes");
    }

    /// The node at `index`.
    pub fn node(&self, index: usize) -> &Node {
        &self.nodes[index]
    }
}

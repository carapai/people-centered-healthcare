//! Long outages at volume (nightly; #141; testing.md § Offline and sync): an edge cut off from
//! central while both keep writing for the facility, then reconnected.
//!
//! - Within the horizon (30 days): the edge catches up page by page.
//! - Beyond it (200 days, with central's log pruned at 180): the edge is told to re-seed, takes
//!   central's snapshot, and still delivers everything it wrote while apart.
//!
//! Either way every record agrees at both nodes and another pass moves nothing.
//! `HEALTH_OUTAGE_RECORDS` sets how many records each side writes (200 by default). Run with
//! `cargo test -p health-sync-sim -- --ignored`.

use std::time::Instant;

use health_identifiers::ResourceId;
use health_surrealdb::SyncScope;
use health_sync_sim::{Node, Sim};
use health_time::{SignedDuration, Timestamp};
use serde_json::json;

const FACILITY: &str = "KLA01";

fn records() -> usize {
    std::env::var("HEALTH_OUTAGE_RECORDS")
        .ok()
        .and_then(|n| n.parse().ok())
        .unwrap_or(200)
}

/// A facility Observation, so it is in the edge's scope wherever it is written.
fn observation(n: usize, at: &str) -> serde_json::Value {
    json!({
        "resourceType": "Observation", "status": "final",
        "meta": {"tag": [{"system": "urn:health:facility", "code": FACILITY}]},
        "code": {"text": format!("written at {at}")},
        "valueInteger": n
    })
}

/// Cuts the edge off for `days`, with each side writing `records()` facility records spread over
/// them, prunes central's log at the horizon if `prune`, reconnects, and checks convergence.
async fn outage(days: i64, prune: bool) -> Sim {
    let start: Timestamp = "2026-01-05T07:00:00Z".parse().expect("time");
    let mut sim = Sim::default();
    let central = sim.add(Node::new("central", start).await);
    let edge = sim.add(Node::new("edge", start).await);
    let wan = sim
        .connect(edge, central, SyncScope::Facility(FACILITY.into()))
        .await;
    let mut written: Vec<ResourceId> = Vec::new();
    // Before the outage: the two agree.
    for n in 0..10 {
        written.push(sim.node(central).create(&observation(n, "central")).await);
    }
    sim.settle().await;

    sim.set_link(wan, false);
    let per_day = records()
        .div_ceil(usize::try_from(days).expect("days"))
        .max(1);
    let mut n = 0;
    // The change log is stamped by the database's clock, not the nodes' simulated ones, so the
    // horizon is taken in real time: halfway through the outage's writes.
    let mut horizon = None;
    for _ in 0..days {
        if horizon.is_none() && n >= records() / 2 {
            horizon = Some(Timestamp::now());
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        for _ in 0..per_day {
            if n >= records() {
                break;
            }
            written.push(sim.node(edge).create(&observation(n, "edge")).await);
            written.push(sim.node(central).create(&observation(n, "central")).await);
            n += 1;
        }
        for node in &sim.nodes {
            node.advance(SignedDuration::from_hours(24));
        }
    }
    if prune {
        // Central keeps 180 days of log (synchronization.md § Tombstones): what it wrote in the
        // outage's first months is gone from it before the edge returns.
        let horizon = sim
            .node(central)
            .store
            .prune_log(horizon.expect("the outage's midpoint"))
            .await
            .expect("prune");
        assert!(
            horizon > 0,
            "nothing was pruned: the outage is within the horizon"
        );
    }

    sim.set_link(wan, true);
    let started = Instant::now();
    let mut rounds = 0;
    loop {
        rounds += 1;
        assert!(rounds < 10_000, "no convergence");
        if !sim.round(wan).await.moved() {
            break;
        }
    }
    eprintln!(
        "outage of {days} days, {} records written apart, prune {prune}: converged in {rounds} \
         rounds, {:.1} s; re-seeds {:?}",
        2 * n,
        started.elapsed().as_secs_f64(),
        sim.reseeds
    );

    for id in &written {
        let at_central = sim.node(central).head("Observation", *id).await;
        assert!(
            at_central.is_some(),
            "Observation/{id} never reached central"
        );
        assert_eq!(
            sim.node(edge).head("Observation", *id).await,
            at_central,
            "Observation/{id} differs at the edge"
        );
    }
    assert!(
        !sim.round(wan).await.moved(),
        "another pass moved something"
    );
    sim
}

#[tokio::test]
#[ignore = "nightly: hundreds of records each way take minutes"]
async fn a_month_apart_catches_up_page_by_page() {
    let sim = outage(30, false).await;
    assert!(
        sim.reseeds.is_empty(),
        "re-seeded within the horizon: {:?}",
        sim.reseeds
    );
}

#[tokio::test]
#[ignore = "nightly: hundreds of records each way take minutes"]
async fn beyond_the_horizon_the_edge_reseeds_and_still_delivers_its_writes() {
    let sim = outage(200, true).await;
    assert!(
        sim.reseeds.iter().any(|(node, _)| *node == "edge"),
        "the edge was not re-seeded: {:?}",
        sim.reseeds
    );
}

//! Central restored from a backup an hour old (ADR 0046, ADR 0065; #138): what the facility pushed
//! after the backup is gone from central, and comes back from the edge, which central's new
//! epoch tells to push again. Central's own records written after the backup are lost, as the
//! RPO allows.

use health_persistence::Precondition;
use health_surrealdb::SyncScope;
use health_sync_protocol::Outcome;
use health_sync_sim::{Node, Sim};
use health_time::{SignedDuration, Timestamp};
use serde_json::{Value, json};

const FACILITY: &str = "KLA01";

fn tagged(mut body: Value) -> Value {
    body["meta"] = json!({"tag": [{"system": "urn:health:facility", "code": FACILITY}]});
    body
}

#[tokio::test]
async fn the_edge_pushes_again_what_a_restored_central_lost() {
    let start: Timestamp = "2026-11-14T08:00:00Z".parse().expect("time");
    let mut sim = Sim::default();
    let central = sim.add(Node::new("central", start).await);
    let edge = sim.add(Node::new("edge", start + SignedDuration::from_mins(2)).await);
    sim.connect(edge, central, SyncScope::Facility(FACILITY.into()))
        .await;

    // Before the backup: a patient, synced.
    let before = sim
        .node(edge)
        .create(&tagged(
            json!({"resourceType": "Patient", "name": [{"text": "Achieng"}]}),
        ))
        .await;
    sim.settle().await;
    let backup = sim.node(central).backup().await;

    // After it: an encounter, the patient's corrected name, and a central-only record.
    let encounter = sim
        .node(edge)
        .create(&tagged(json!({
            "resourceType": "Encounter",
            "status": "in-progress",
            "subject": {"reference": format!("Patient/{before}")},
        })))
        .await;
    sim.node(edge)
        .write(
            before,
            &tagged(json!({"resourceType": "Patient", "name": [{"text": "Achieng Okot"}]})),
            Precondition::None,
        )
        .await;
    sim.settle().await;
    assert!(
        sim.node(central)
            .head("Encounter", encounter)
            .await
            .is_some()
    );

    // Central is restored: the last hour is gone from it.
    sim.node_mut(central).restore_from(&backup).await;
    assert!(
        sim.node(central)
            .head("Encounter", encounter)
            .await
            .is_none()
    );
    let (_, patient) = sim
        .node(central)
        .head("Patient", before)
        .await
        .expect("the patient was in the backup");
    assert_eq!(patient["name"][0]["text"], "Achieng");

    // The edge sees the new epoch and pushes again what central no longer holds.
    sim.outcomes.clear();
    sim.settle().await;
    assert_eq!(sim.repushes.len(), 1, "{:?}", sim.repushes);
    let (_, requeued) = sim.repushes[0];
    assert!(requeued >= 2, "the encounter and the new name: {requeued}");
    assert_eq!(
        sim.node(central).head("Encounter", encounter).await,
        sim.node(edge).head("Encounter", encounter).await,
    );
    assert_eq!(
        sim.node(central).head("Patient", before).await,
        sim.node(edge).head("Patient", before).await,
    );
    assert!(
        sim.outcomes.iter().all(|o| *o == Outcome::Accepted),
        "nothing conflicted: {:?}",
        sim.outcomes
    );

    // Settled: a later handshake re-queues nothing.
    sim.settle().await;
    assert_eq!(sim.repushes.len(), 1);
}

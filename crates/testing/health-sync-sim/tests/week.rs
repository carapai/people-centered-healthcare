//! A week of clinics on a flaky link (nightly; testing.md § Offline and sync; #85): every day the
//! facility's internet and, some days, its LAN go down; patients are seen at the desktop and the
//! edge regardless; central's registry changes; and on two days the same patient is edited at the
//! edge and at central while apart. At the end every record agrees across the three nodes, the
//! two concurrent edits are Conflict Records rather than lost or overwritten, and nothing moves on
//! another pass. Run with `cargo test -p health-sync-sim -- --ignored`.

use health_identifiers::ResourceId;
use health_persistence::Precondition;
use health_surrealdb::SyncScope;
use health_sync_protocol::Outcome;
use health_sync_sim::{Node, Sim};
use health_time::{SignedDuration, Timestamp};
use serde_json::json;

const FACILITY: &str = "KLA01";
const DAYS: usize = 7;
const PATIENTS_PER_DAY: usize = 12;

#[tokio::test]
#[ignore = "nightly: a week of clinics takes minutes"]
async fn a_week_of_clinics_on_a_flaky_link_converges() {
    let start: Timestamp = "2026-11-09T07:00:00Z".parse().expect("time");
    let mut sim = Sim::default();
    let central = sim.add(Node::new("central", start).await);
    let edge = sim.add(Node::new("edge", start + SignedDuration::from_mins(2)).await);
    let desktop = sim.add(Node::new("desktop", start - SignedDuration::from_mins(5)).await);
    let lan = sim
        .connect(desktop, edge, SyncScope::Facility(FACILITY.into()))
        .await;
    let wan = sim
        .connect(edge, central, SyncScope::Facility(FACILITY.into()))
        .await;
    let mut records: Vec<(&str, ResourceId)> = Vec::new();
    let mut contested = Vec::new();

    for day in 0..DAYS {
        // Morning: the internet is down every day, the LAN on odd days.
        sim.set_link(wan, false);
        sim.set_link(lan, day % 2 == 0);
        for n in 0..PATIENTS_PER_DAY {
            let at = if n % 3 == 0 { edge } else { desktop };
            let node = sim.node(at);
            let patient = node
                .create(&json!({"resourceType": "Patient", "name": [{"text": format!("Day {day} patient {n}")}]}))
                .await;
            records.push(("Patient", patient));
            let observation = node
                .create(&json!({
                    "resourceType": "Observation", "status": "final",
                    "meta": {"tag": [{"system": "urn:health:facility", "code": FACILITY}]},
                    "subject": {"reference": format!("Patient/{patient}")},
                    "valueInteger": n
                }))
                .await;
            records.push(("Observation", observation));
            if n % 4 == 0 {
                sim.round(lan).await;
            }
        }
        // Central registers a patient for the facility; on days 2 and 5 that patient is then
        // corrected both at central and at the edge, while apart.
        let referred = sim
            .node(central)
            .create(
                &json!({"resourceType": "Patient", "name": [{"text": format!("Referred {day}")}]}),
            )
            .await;
        records.push(("Patient", referred));
        if day == 2 || day == 5 {
            sim.set_link(wan, true);
            sim.settle().await;
            sim.set_link(wan, false);
            let (base, _) = sim
                .node(edge)
                .head("Patient", referred)
                .await
                .expect("synced");
            sim.node(central)
                .write(referred, &json!({"resourceType": "Patient", "name": [{"text": format!("Referred {day} (central)")}]}), Precondition::IfMatch(base))
                .await;
            sim.node(edge)
                .write(referred, &json!({"resourceType": "Patient", "name": [{"text": format!("Referred {day} (edge)")}]}), Precondition::IfMatch(base))
                .await;
            contested.push(referred);
        }
        // Evening: everything reconnects.
        sim.set_link(lan, true);
        sim.set_link(wan, true);
        sim.settle().await;
        for node in &sim.nodes {
            node.advance(SignedDuration::from_hours(24));
        }
    }

    for (resource_type, id) in &records {
        let at_central = sim.node(central).head(resource_type, *id).await;
        assert!(
            at_central.is_some(),
            "{resource_type}/{id} never reached central"
        );
        for other in [edge, desktop] {
            assert_eq!(
                sim.node(other).head(resource_type, *id).await,
                at_central,
                "{resource_type}/{id} at {}",
                sim.node(other).name
            );
        }
    }
    let conflicts = sim
        .outcomes
        .iter()
        .filter(|o| matches!(o, Outcome::Conflict { .. }))
        .count();
    assert_eq!(conflicts, contested.len(), "{:?}", sim.outcomes);
    assert_eq!(
        sim.node(central)
            .store
            .open_conflicts(None)
            .await
            .expect("queue")
            .len(),
        contested.len()
    );
    for id in &contested {
        let (_, kept) = sim.node(central).head("Patient", *id).await.expect("head");
        assert!(
            kept["name"][0]["text"]
                .as_str()
                .is_some_and(|n| n.ends_with("(central)")),
            "central's version stays the head until reviewed: {kept}"
        );
    }
    for link in [lan, wan] {
        assert!(!sim.round(link).await.moved());
    }
}

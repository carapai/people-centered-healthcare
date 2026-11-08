//! The clinical acceptance scenario (§117) across an offline period, on central + edge + desktop
//! (testing.md § Offline and sync; #85).
//!
//! A patient is registered at a desktop, triaged and seen, a note is signed and medicine
//! prescribed, while the facility has no internet and, for a while, the desktop no LAN. Central's
//! registry changes meanwhile, the outage outlasts central's horizon, and later central is restored
//! from backup. Every node keeps its own clock, minutes apart. In the end every node holds the
//! same version of every record in its scope, nothing written offline is lost, and nothing is
//! applied twice.

use health_identifiers::ResourceId;
use health_persistence::Precondition;
use health_surrealdb::SyncScope;
use health_sync_protocol::Outcome;
use health_sync_sim::{Node, Sim};
use health_time::{Clock as _, SignedDuration, Timestamp};
use serde_json::{Value, json};

const FACILITY: &str = "KLA01";

fn tagged(mut body: Value) -> Value {
    body["meta"] = json!({"tag": [{"system": "urn:health:facility", "code": FACILITY}]});
    body
}

/// Records every node in the deployment must end up holding identically.
struct Expect(Vec<(&'static str, ResourceId)>);

impl Expect {
    fn add(&mut self, resource_type: &'static str, id: ResourceId) -> ResourceId {
        self.0.push((resource_type, id));
        id
    }
}

async fn assert_converged(sim: &Sim, nodes: &[usize], expect: &Expect) {
    for (resource_type, id) in &expect.0 {
        let at_first = sim.node(nodes[0]).head(resource_type, *id).await;
        assert!(
            at_first.is_some(),
            "{resource_type}/{id} is missing at {}",
            sim.node(nodes[0]).name
        );
        for &other in &nodes[1..] {
            assert_eq!(
                sim.node(other).head(resource_type, *id).await,
                at_first,
                "{resource_type}/{id} differs between {} and {}",
                sim.node(nodes[0]).name,
                sim.node(other).name
            );
        }
    }
}

#[tokio::test]
async fn the_clinical_scenario_survives_an_offline_period() {
    let start: Timestamp = "2026-11-09T07:00:00Z".parse().expect("time");
    let mut sim = Sim::default();
    // Clocks minutes apart: they order nothing that matters (synchronization.md § Envelope).
    let central = sim.add(Node::new("central", start).await);
    let edge = sim.add(Node::new("edge", start + SignedDuration::from_mins(3)).await);
    let desktop = sim.add(Node::new("desktop", start - SignedDuration::from_mins(7)).await);
    let lan = sim
        .connect(desktop, edge, SyncScope::Facility(FACILITY.into()))
        .await;
    let wan = sim
        .connect(edge, central, SyncScope::Facility(FACILITY.into()))
        .await;
    let mut expect = Expect(Vec::new());

    // Central's registry, synced to the facility before the outage.
    let clinician = expect.add(
        "Practitioner",
        sim.node(central)
            .create(&json!({"resourceType": "Practitioner", "name": [{"text": "Dr Nakato"}]}))
            .await,
    );
    sim.settle().await;
    assert!(
        sim.node(desktop)
            .head("Practitioner", clinician)
            .await
            .is_some()
    );

    // The internet goes down.
    sim.set_link(wan, false);
    let d = sim.node(desktop);
    let patient = expect.add(
        "Patient",
        d.create(&json!({"resourceType": "Patient", "name": [{"text": "Grace Auma"}], "gender": "female"}))
            .await,
    );
    let encounter = expect.add(
        "Encounter",
        d.create(&tagged(json!({
            "resourceType": "Encounter", "status": "in-progress",
            "subject": {"reference": format!("Patient/{patient}")}
        })))
        .await,
    );
    for (code, value) in [("8480-6", 128), ("8462-4", 84), ("8310-5", 38)] {
        expect.add(
            "Observation",
            d.create(&tagged(json!({
                "resourceType": "Observation", "status": "final",
                "code": {"coding": [{"system": "http://loinc.org", "code": code}]},
                "subject": {"reference": format!("Patient/{patient}")},
                "encounter": {"reference": format!("Encounter/{encounter}")},
                "valueInteger": value
            })))
            .await,
        );
    }
    expect.add(
        "Condition",
        d.create(&tagged(json!({
            "resourceType": "Condition",
            "code": {"text": "Malaria"},
            "subject": {"reference": format!("Patient/{patient}")}
        })))
        .await,
    );
    let note = expect.add(
        "Composition",
        d.create(&tagged(json!({
            "resourceType": "Composition", "status": "final", "title": "Consultation",
            "subject": [{"reference": format!("Patient/{patient}")}],
            "section": [{"text": {"status": "generated", "div": "<div>Fever 3 days; RDT positive</div>"}}]
        })))
        .await,
    );
    expect.add(
        "MedicationRequest",
        d.create(&tagged(json!({
            "resourceType": "MedicationRequest", "status": "active", "intent": "order",
            "medication": {"concept": {"text": "Artemether-lumefantrine"}},
            "subject": {"reference": format!("Patient/{patient}")}
        })))
        .await,
    );
    // The desktop syncs to the edge over the LAN; nothing reaches central.
    sim.round(lan).await;
    assert!(sim.node(edge).head("Composition", note).await.is_some());
    assert!(sim.node(central).head("Composition", note).await.is_none());

    // The LAN drops too: the desktop works on alone, the edge records a task, and central's
    // registry changes.
    sim.set_link(lan, false);
    let (clinician_version, _) = sim
        .node(central)
        .head("Practitioner", clinician)
        .await
        .expect("head");
    sim.node(central)
        .write(
            clinician,
            &json!({"resourceType": "Practitioner", "name": [{"text": "Dr Nakato Sarah"}]}),
            Precondition::IfMatch(clinician_version),
        )
        .await;
    expect.add(
        "Task",
        sim.node(edge)
            .create(&tagged(json!({
                "resourceType": "Task", "status": "requested", "intent": "order",
                "for": {"reference": format!("Patient/{patient}")}
            })))
            .await,
    );
    let (encounter_version, _) = sim
        .node(desktop)
        .head("Encounter", encounter)
        .await
        .expect("head");
    sim.node(desktop)
        .write(
            encounter,
            &tagged(json!({
                "resourceType": "Encounter", "status": "completed",
                "subject": {"reference": format!("Patient/{patient}")}
            })),
            Precondition::IfMatch(encounter_version),
        )
        .await;

    // The outage outlasts central's horizon: its log is pruned past the edge's cursor.
    for node in &sim.nodes {
        node.advance(SignedDuration::from_hours(24 * 200));
    }
    let horizon = sim.node(central).clock.now() - SignedDuration::from_hours(24 * 180);
    sim.node(central)
        .store
        .prune_log(horizon + SignedDuration::from_hours(24 * 365))
        .await
        .expect("prune");

    // Everything comes back.
    sim.set_link(lan, true);
    sim.set_link(wan, true);
    sim.settle().await;
    assert!(
        sim.reseeds.iter().any(|(name, _)| *name == "edge"),
        "the edge re-seeds after the horizon: {:?}",
        sim.reseeds
    );
    assert_converged(&sim, &[central, edge, desktop], &expect).await;
    let (_, registry) = sim
        .node(desktop)
        .head("Practitioner", clinician)
        .await
        .expect("head");
    assert_eq!(registry["name"][0]["text"], "Dr Nakato Sarah");
    let (_, visit) = sim
        .node(central)
        .head("Encounter", encounter)
        .await
        .expect("head");
    assert_eq!(visit["status"], "completed");
    assert!(
        sim.outcomes.iter().all(|o| *o == Outcome::Accepted),
        "nothing conflicted or was refused: {:?}",
        sim.outcomes
    );

    // Central is restored from backup: a new epoch, so everyone below re-seeds, and still nothing
    // is lost or duplicated.
    let reseeds_before = sim.reseeds.len();
    sim.node(central).restore().await;
    sim.settle().await;
    assert!(sim.reseeds.len() > reseeds_before, "{:?}", sim.reseeds);
    assert_converged(&sim, &[central, edge, desktop], &expect).await;

    // Settled: another pass moves nothing.
    for link in [lan, wan] {
        assert!(!sim.round(link).await.moved());
    }
}

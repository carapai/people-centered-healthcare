use crate::interpreter::{Event, Instance};
use crate::model::{CLAIM_TIMEOUT, Kind};
use crate::store::STARTED;

use super::*;

fn at(text: &str) -> Timestamp {
    text.parse().expect("timestamp")
}

fn task(role: &str, worklist: &str, priority: Priority) -> HumanTask {
    HumanTask {
        role: role.into(),
        form: Some("https://health.go.ug/fhir/Questionnaire/triage|2.0.0".into()),
        worklist: worklist.into(),
        priority,
        claim_timeout: CLAIM_TIMEOUT,
    }
}

/// Reception → triage (a nurse's task) → consultation (a clinician's task) → done.
fn chart() -> Statechart {
    let mut triage = State::new("triage", Kind::Atomic);
    triage.task = Some(task("nurse", "opd-triage", Priority::Urgent));
    let mut consultation = State::new("consultation", Kind::Atomic);
    consultation.task = Some(task("clinician", "opd-consult", Priority::Routine));
    Statechart {
        states: vec![
            State::new("reception", Kind::Atomic),
            triage,
            consultation,
            State::new("done", Kind::Final),
        ],
        initial: vec![],
    }
}

fn record(configuration: &[&str], data: &[(String, Value)]) -> InstanceRecord {
    InstanceRecord {
        id: "i-1".into(),
        definition: "urn:opd|1.0.0".into(),
        context: "Encounter/e-1".into(),
        authority: "KAM013".into(),
        state: Instance {
            configuration: configuration.iter().map(|s| (*s).to_owned()).collect(),
            data: data.iter().cloned().collect(),
            sequence: 1,
            invoked: vec![],
            running: !configuration.is_empty(),
        },
        version: 1,
    }
}

/// History moving through `steps`: (event, configuration after, time).
fn history(steps: &[(&str, &[&str], &str)]) -> Vec<HistoryEntry> {
    let mut from: Vec<String> = Vec::new();
    steps
        .iter()
        .zip(1..)
        .map(|((event, to, when), seq)| {
            let to: Vec<String> = to.iter().map(|s| (*s).to_owned()).collect();
            HistoryEntry {
                instance: "i-1".into(),
                seq,
                event: Event::named(event),
                from: std::mem::replace(&mut from, to.clone()),
                to,
                effects: vec![],
                actor: "p-1".into(),
                at: at(when),
                op_id: format!("op-{seq}"),
            }
        })
        .collect()
}

fn claimed(state: &str, owner: &str, when: &str) -> Vec<(String, Value)> {
    vec![
        (owner_field(state), Value::Text(owner.into())),
        (claimed_field(state), Value::Int(at(when).as_second())),
    ]
}

fn statuses(tasks: &[TaskView]) -> Vec<(&str, &str)> {
    tasks
        .iter()
        .map(|t| (t.state.as_str(), t.status.code()))
        .collect()
}

#[test]
fn an_open_task_is_ready_until_claimed_and_the_claim_lapses_when_idle() {
    let steps = history(&[
        (STARTED, &["reception"], "2026-11-04T08:00:00Z"),
        ("arrived", &["triage"], "2026-11-04T08:05:00Z"),
    ]);
    let now = at("2026-11-04T08:10:00Z");

    let ready = project(&chart(), &record(&["triage"], &[]), &steps, now);
    assert_eq!(statuses(&ready), [("triage", "ready")]);
    let triage = &ready[0];
    assert_eq!(triage.id, task_id("i-1", "triage", 2));
    assert_eq!(triage.role, "nurse");
    assert_eq!(triage.worklist, "opd-triage");
    assert_eq!(triage.priority, Priority::Urgent);
    assert_eq!(triage.context, "Encounter/e-1");
    assert_eq!(triage.authored_on, at("2026-11-04T08:05:00Z"));
    assert_eq!(triage.owner, None);

    let data = claimed("triage", "Practitioner/nurse-1", "2026-11-04T08:06:00Z");
    let claimed = project(&chart(), &record(&["triage"], &data), &steps, now);
    assert_eq!(statuses(&claimed), [("triage", "in-progress")]);
    assert_eq!(claimed[0].owner.as_deref(), Some("Practitioner/nurse-1"));
    assert_eq!(claimed[0].last_modified, at("2026-11-04T08:06:00Z"));
    assert_eq!(claimed[0].claim_lapses, Some(at("2026-11-04T08:21:00Z")));
    assert_eq!(claimed[0].as_at(at("2026-11-04T08:20:59Z")), claimed[0]);
    let lapsed = claimed[0].as_at(at("2026-11-04T08:21:00Z"));
    assert_eq!((lapsed.status, lapsed.owner), (TaskStatus::Ready, None));

    let idle = project(
        &chart(),
        &record(&["triage"], &data),
        &steps,
        at("2026-11-04T08:21:00Z"),
    );
    assert_eq!(
        statuses(&idle),
        [("triage", "ready")],
        "a claim idle for 15 minutes lapses"
    );
    assert_eq!(idle[0].owner, None);
}

#[test]
fn leaving_a_task_state_completes_it_and_cancelling_cancels_it() {
    let steps = history(&[
        (STARTED, &["reception"], "2026-11-04T08:00:00Z"),
        ("arrived", &["triage"], "2026-11-04T08:05:00Z"),
        ("triaged", &["consultation"], "2026-11-04T08:20:00Z"),
        (CANCELLED, &[], "2026-11-04T08:30:00Z"),
    ]);
    let tasks = project(
        &chart(),
        &record(&[], &[]),
        &steps,
        at("2026-11-04T09:00:00Z"),
    );
    assert_eq!(
        statuses(&tasks),
        [("triage", "completed"), ("consultation", "cancelled")]
    );
    assert_eq!(tasks[0].last_modified, at("2026-11-04T08:20:00Z"));
    assert_eq!(tasks[1].last_modified, at("2026-11-04T08:30:00Z"));
}

#[test]
fn each_entry_into_a_task_state_is_its_own_task() {
    let steps = history(&[
        (STARTED, &["triage"], "2026-11-04T08:00:00Z"),
        ("triaged", &["consultation"], "2026-11-04T08:10:00Z"),
        ("retriage", &["triage"], "2026-11-04T08:20:00Z"),
    ]);
    let tasks = project(
        &chart(),
        &record(&["triage"], &[]),
        &steps,
        at("2026-11-04T08:30:00Z"),
    );
    assert_eq!(
        statuses(&tasks),
        [
            ("triage", "completed"),
            ("consultation", "completed"),
            ("triage", "ready")
        ]
    );
    assert_ne!(tasks[0].id, tasks[2].id);
    assert_eq!(tasks[2].id, task_id("i-1", "triage", 3));
}

#[test]
fn projections_rebuild_identically_from_the_same_instance_state() {
    let steps = history(&[
        (STARTED, &["reception"], "2026-11-04T08:00:00Z"),
        ("arrived", &["triage"], "2026-11-04T08:05:00Z"),
    ]);
    let data = claimed("triage", "nurse-1", "2026-11-04T08:06:00Z");
    let now = at("2026-11-04T08:07:00Z");
    let record = record(&["triage"], &data);
    assert_eq!(
        project(&chart(), &record, &steps, now),
        project(&chart(), &record, &steps, now)
    );
}

#[test]
fn escalation_only_raises_priority() {
    let steps = history(&[(STARTED, &["triage"], "2026-11-04T08:00:00Z")]);
    let now = at("2026-11-04T08:01:00Z");
    let escalated = |code: &str| {
        let data = [(priority_field("triage"), Value::Text(code.into()))];
        project(&chart(), &record(&["triage"], &data), &steps, now)[0].priority
    };
    assert_eq!(escalated("stat"), Priority::Stat);
    assert_eq!(escalated("routine"), Priority::Urgent);
    assert_eq!(escalated("nonsense"), Priority::Urgent);
}

#[test]
fn a_worklist_is_open_tasks_most_urgent_first_then_by_arrival() {
    let view = |id: &str, worklist: &str, priority, arrived: &str, status| TaskView {
        id: id.into(),
        instance: id.into(),
        definition: "urn:opd|1.0.0".into(),
        context: "Encounter/e".into(),
        state: "triage".into(),
        status,
        role: "nurse".into(),
        form: None,
        worklist: worklist.into(),
        priority,
        owner: None,
        claim_lapses: None,
        authored_on: at(arrived),
        last_modified: at(arrived),
    };
    let tasks = [
        view(
            "a",
            "opd-triage",
            Priority::Routine,
            "2026-11-04T08:00:00Z",
            TaskStatus::Ready,
        ),
        view(
            "b",
            "opd-triage",
            Priority::Stat,
            "2026-11-04T08:30:00Z",
            TaskStatus::InProgress,
        ),
        view(
            "c",
            "opd-triage",
            Priority::Routine,
            "2026-11-04T07:00:00Z",
            TaskStatus::Ready,
        ),
        view(
            "d",
            "opd-triage",
            Priority::Urgent,
            "2026-11-04T08:00:00Z",
            TaskStatus::Completed,
        ),
        view(
            "e",
            "opd-consult",
            Priority::Stat,
            "2026-11-04T06:00:00Z",
            TaskStatus::Ready,
        ),
    ];
    let ids: Vec<&str> = worklist(&tasks, "opd-triage")
        .into_iter()
        .map(|t| t.id.as_str())
        .collect();
    assert_eq!(ids, ["b", "c", "a"]);
}

#[test]
fn all_twelve_task_codes_are_named() {
    let all = [
        TaskStatus::Draft,
        TaskStatus::Requested,
        TaskStatus::Received,
        TaskStatus::Accepted,
        TaskStatus::Rejected,
        TaskStatus::Ready,
        TaskStatus::Cancelled,
        TaskStatus::InProgress,
        TaskStatus::OnHold,
        TaskStatus::Failed,
        TaskStatus::Completed,
        TaskStatus::EnteredInError,
    ];
    let codes: std::collections::BTreeSet<&str> = all.iter().map(|s| s.code()).collect();
    assert_eq!(codes.len(), 12);
    assert!(is_task_field(&owner_field("triage")));
    assert!(!is_task_field("urgent"));
}

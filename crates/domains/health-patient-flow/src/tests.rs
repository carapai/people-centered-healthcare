use super::*;

fn at(text: &str) -> Timestamp {
    text.parse().expect("timestamp")
}

#[test]
fn a_replayed_arrival_has_the_same_id() {
    let first = arrive(
        "Encounter/e-1",
        "Patient/p-1",
        "triage",
        Priority::Routine,
        at("2026-11-05T08:00:00Z"),
    );
    let again = arrive(
        "Encounter/e-1",
        "Patient/p-1",
        "triage",
        Priority::Routine,
        at("2026-11-05T08:00:00Z"),
    );
    assert_eq!(first.id, again.id);
    let elsewhere = arrive(
        "Encounter/e-1",
        "Patient/p-1",
        "consultation",
        Priority::Routine,
        at("2026-11-05T08:00:00Z"),
    );
    assert_ne!(first.id, elsewhere.id);
    assert_eq!(first.status, QueueStatus::Waiting);
}

#[test]
fn the_queue_is_emergencies_first_then_by_arrival() {
    let entry =
        |enc: &str, priority, when: &str| arrive(enc, "Patient/p", "triage", priority, at(when));
    let mut seen = entry("e-0", Priority::Routine, "2026-11-05T07:00:00Z");
    seen.status = QueueStatus::Done;
    let entries = [
        entry("e-1", Priority::Routine, "2026-11-05T08:00:00Z"),
        entry("e-2", Priority::Emergency, "2026-11-05T08:30:00Z"),
        entry("e-3", Priority::Routine, "2026-11-05T07:30:00Z"),
        entry("e-4", Priority::Urgent, "2026-11-05T08:10:00Z"),
        seen,
        arrive(
            "e-5",
            "Patient/p",
            "lab",
            Priority::Emergency,
            at("2026-11-05T06:00:00Z"),
        ),
    ];
    let order: Vec<&str> = queue(&entries, "triage")
        .iter()
        .map(|e| e.encounter.as_str())
        .collect();
    assert_eq!(order, ["e-2", "e-4", "e-3", "e-1"]);
}

#[test]
fn a_patient_is_called_seen_or_leaves() {
    let now = at("2026-11-05T08:00:00Z");
    let waiting = arrive("e-1", "Patient/p-1", "triage", Priority::Routine, now);
    let called = waiting
        .step(
            &Step::Call {
                by: "nurse-1".into(),
            },
            now,
        )
        .expect("call");
    assert_eq!(called.called, Some(("nurse-1".to_owned(), now)));
    let back = called.step(&Step::Recall, now).expect("recall");
    assert_eq!(
        (back.status, back.called.clone()),
        (QueueStatus::Waiting, None)
    );
    let done = called
        .step(&Step::Serve, now)
        .and_then(|e| e.step(&Step::Finish, now))
        .expect("seen");
    assert_eq!((done.status, done.closed), (QueueStatus::Done, Some(now)));
    assert_eq!(
        done.step(&Step::Leave, now),
        Err(QueueRefused {
            status: QueueStatus::Done
        })
    );
    let left = waiting.step(&Step::Leave, now).expect("left");
    assert!(!left.status.is_open());
}

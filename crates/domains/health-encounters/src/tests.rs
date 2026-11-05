use super::*;

fn at(text: &str) -> Timestamp {
    text.parse().expect("timestamp")
}

fn request() -> CheckIn {
    CheckIn {
        id: "0190f1a2-7c3d-7e4f-8a5b-000000000001".into(),
        patient: "Patient/p-1".into(),
        facility: "KAM013".into(),
        service_point: "triage".into(),
        class: Class::Ambulatory,
        by: "reception-1".into(),
        at: at("2026-11-05T08:00:00Z"),
    }
}

#[test]
fn a_check_in_starts_an_encounter_at_the_service_point() {
    let (encounter, event) = check_in(&request()).expect("checked in");
    assert_eq!(encounter.status, EncounterStatus::InProgress);
    assert_eq!(encounter.service_point, "triage");
    assert_eq!(
        event,
        EncounterEvent::Started {
            encounter: encounter.id.clone(),
            patient: "Patient/p-1".into(),
            service_point: "triage".into(),
            class: Class::Ambulatory,
            at: at("2026-11-05T08:00:00Z"),
        }
    );
    assert_eq!(event.name(), "encounters.started.v1");
    assert_eq!(
        open_guard("Patient/p-1", "triage"),
        "open-encounter|Patient/p-1|triage"
    );
    let mut blank = request();
    blank.service_point = " ".into();
    assert_eq!(check_in(&blank), Err(EncounterRefused::Incomplete));
}

#[test]
fn the_status_follows_r5s_lifecycle_and_final_is_final() {
    let (started, _) = check_in(&request()).expect("checked in");
    let later = at("2026-11-05T09:00:00Z");
    let (held, event) = started
        .transition(EncounterStatus::OnHold, later)
        .expect("hold");
    assert_eq!(event.name(), "encounters.status-changed.v1");
    let (resumed, _) = held
        .transition(EncounterStatus::InProgress, later)
        .expect("resume");
    let (done, _) = resumed
        .transition(EncounterStatus::Completed, later)
        .expect("complete");
    assert_eq!(done.ended, Some(later));
    assert!(done.status.is_final());
    assert_eq!(
        done.transition(EncounterStatus::InProgress, later),
        Err(EncounterRefused::Transition {
            from: "completed",
            to: "in-progress"
        })
    );
    assert!(
        done.transition(EncounterStatus::EnteredInError, later)
            .is_err()
    );
    assert!(
        started
            .transition(EncounterStatus::EnteredInError, later)
            .is_ok()
    );
    assert!(
        started
            .transition(EncounterStatus::Cancelled, later)
            .is_err()
    );
}

#[test]
fn a_patient_moves_between_service_points_while_under_way() {
    let (started, _) = check_in(&request()).expect("checked in");
    let at_move = at("2026-11-05T08:30:00Z");
    let (moved, event) = started.move_to("consultation", at_move).expect("moved");
    assert_eq!(moved.service_point, "consultation");
    assert!(
        matches!(event, EncounterEvent::Moved { ref from, ref to, .. } if from == "triage" && to == "consultation")
    );
    assert_eq!(
        moved.move_to("consultation", at_move),
        Err(EncounterRefused::CannotMove)
    );
    let (held, _) = moved
        .transition(EncounterStatus::OnHold, at_move)
        .expect("hold");
    assert_eq!(
        held.move_to("lab", at_move),
        Err(EncounterRefused::CannotMove)
    );
}

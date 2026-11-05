use super::*;

fn at(text: &str) -> Timestamp {
    text.parse().expect("timestamp")
}

fn drafted() -> CarePlan {
    draft(Draft {
        id: "cp1".into(),
        patient: "Patient/p-1".into(),
        facility: "KAM013".into(),
        title: " Hypertension follow-up ".into(),
        description: Some("  ".into()),
        author: "clinician-1".into(),
        at: at("2026-11-05T10:00:00Z"),
    })
    .expect("drafted")
}

#[test]
fn a_plan_is_drafted_activated_and_ended() {
    let plan = drafted();
    assert_eq!(
        (
            plan.status,
            plan.title.as_str(),
            plan.description.as_deref()
        ),
        (PlanStatus::Draft, "Hypertension follow-up", None)
    );
    let active = plan
        .transition(PlanStatus::Active, at("2026-11-05T10:05:00Z"))
        .expect("active");
    let held = active
        .transition(PlanStatus::OnHold, at("2026-11-06T00:00:00Z"))
        .expect("held");
    let resumed = held
        .transition(PlanStatus::Active, at("2026-11-07T00:00:00Z"))
        .expect("resumed");
    let done = resumed
        .transition(PlanStatus::Completed, at("2027-01-01T00:00:00Z"))
        .expect("completed");
    assert_eq!(done.ended, Some(at("2027-01-01T00:00:00Z")));
    assert_eq!(
        done.transition(PlanStatus::Active, at("2027-01-02T00:00:00Z")),
        Err(PlanRefused::Transition {
            from: "completed",
            to: "active"
        })
    );
    assert!(
        done.transition(PlanStatus::EnteredInError, at("2027-01-02T00:00:00Z"))
            .is_err()
    );
    assert!(
        plan.transition(PlanStatus::Completed, at("2026-11-05T10:05:00Z"))
            .is_err(),
        "a draft is activated before it completes"
    );
    assert!(
        plan.transition(PlanStatus::EnteredInError, at("2026-11-05T10:05:00Z"))
            .is_ok()
    );
    assert!(
        draft(Draft {
            title: " ".into(),
            ..Draft {
                id: "x".into(),
                patient: "Patient/p".into(),
                facility: "F".into(),
                title: "t".into(),
                description: None,
                author: "a".into(),
                at: at("2026-11-05T10:00:00Z")
            }
        })
        .is_err()
    );
    for status in [
        PlanStatus::Draft,
        PlanStatus::Active,
        PlanStatus::OnHold,
        PlanStatus::Revoked,
        PlanStatus::Completed,
        PlanStatus::EnteredInError,
    ] {
        assert_eq!(PlanStatus::from_code(status.code()), Some(status));
    }
    assert_eq!(PlanStatus::from_code("cancelled"), None);
}

#[test]
fn activities_are_the_patients_requests_and_conditions_what_it_addresses() {
    let plan = drafted()
        .address("Condition/c-1")
        .expect("addressed")
        .address("Condition/c-1")
        .expect("once")
        .plan("MedicationRequest/rx-1")
        .expect("planned")
        .plan("ServiceRequest/sr-1")
        .expect("planned")
        .plan("MedicationRequest/rx-1")
        .expect("once");
    assert_eq!(plan.addresses, ["Condition/c-1"]);
    assert_eq!(
        plan.activities,
        ["MedicationRequest/rx-1", "ServiceRequest/sr-1"]
    );
    assert_eq!(
        plan.plan("Observation/o-1"),
        Err(PlanRefused::NotARequest("Observation/o-1".into()))
    );
    assert_eq!(
        plan.address("Observation/o-1"),
        Err(PlanRefused::NotACondition("Observation/o-1".into()))
    );
    let revoked = plan
        .transition(PlanStatus::Revoked, at("2026-11-06T00:00:00Z"))
        .expect("revoked");
    assert_eq!(revoked.plan("Task/t-1"), Err(PlanRefused::Ended));
    assert_eq!(revoked.address("Condition/c-2"), Err(PlanRefused::Ended));
}

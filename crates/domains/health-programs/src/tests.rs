use super::*;

fn at(text: &str) -> Timestamp {
    text.parse().expect("timestamp")
}

fn art() -> Program {
    Program {
        url: "https://health.go.ug/fhir/PlanDefinition/art-program".into(),
        version: "1.0.0".into(),
        episode_type: Coded {
            text: "HIV care".into(),
            system: None,
            code: None,
        },
        eligibility: Some("%patient.active".into()),
        workflows: vec![],
        regimens: vec!["https://health.go.ug/fhir/ActivityDefinition/tdf-3tc-dtg".into()],
        sources: vec!["https://health.go.ug/protocols/consolidated-hiv-2024".into()],
    }
}

fn enrolment(program: &Program, eligible: bool) -> Enrolment<'_> {
    Enrolment {
        id: "0190f1a2-7c3d-7e4f-8a5b-000000000501".into(),
        patient: "Patient/p-1".into(),
        program,
        facility: "KAM013".into(),
        eligible,
        by: "clinician-1".into(),
        at: at("2026-11-05T10:00:00Z"),
        transferred_from: None,
    }
}

#[test]
fn an_eligible_patient_is_enrolled_in_an_active_episode() {
    let program = art();
    let (episode, event) = enrol(&enrolment(&program, true)).expect("enrolled");
    assert_eq!(episode.status, EpisodeStatus::Active);
    assert_eq!(episode.episode_type.text, "HIV care");
    assert_eq!(event.name(), "programs.enrolled.v1");
    assert_eq!(
        enrol(&enrolment(&program, false)),
        Err(ProgramRefused::NotEligible(program.url.clone()))
    );
    let mut blank = enrolment(&program, true);
    blank.facility = " ".into();
    assert_eq!(enrol(&blank), Err(ProgramRefused::Incomplete));
}

#[test]
fn one_open_episode_per_program_but_programs_run_concurrently() {
    let tb = "https://health.go.ug/fhir/PlanDefinition/tb-program";
    assert_eq!(
        open_guard("Patient/p-1", &art().url),
        open_guard("Patient/p-1", &art().url)
    );
    assert_ne!(
        open_guard("Patient/p-1", &art().url),
        open_guard("Patient/p-1", tb)
    );
    assert_ne!(
        open_guard("Patient/p-1", &art().url),
        open_guard("Patient/p-2", &art().url)
    );
}

#[test]
fn the_lifecycle_follows_r5() {
    let program = art();
    let (active, _) = enrol(&enrolment(&program, true)).expect("enrolled");
    let (held, event) = active
        .transition(EpisodeStatus::Onhold, at("2026-12-01T00:00:00Z"))
        .expect("on hold");
    assert_eq!(event.name(), "programs.status-changed.v1");
    assert!(held.end.is_none());
    let (done, _) = held
        .transition(EpisodeStatus::Finished, at("2027-01-01T00:00:00Z"))
        .expect("finished");
    assert_eq!(done.end, Some(at("2027-01-01T00:00:00Z")));
    assert!(!done.status.is_open());
    assert_eq!(
        done.transition(EpisodeStatus::Active, at("2027-02-01T00:00:00Z")),
        Err(ProgramRefused::Transition {
            from: "finished",
            to: "active"
        })
    );
    assert!(
        done.transition(EpisodeStatus::EnteredInError, at("2027-02-01T00:00:00Z"))
            .is_err(),
        "only an open episode is entered in error"
    );
    assert!(
        active
            .transition(EpisodeStatus::EnteredInError, at("2026-11-06T00:00:00Z"))
            .is_ok()
    );
    assert!(
        active
            .transition(EpisodeStatus::Cancelled, at("2026-11-06T00:00:00Z"))
            .is_err(),
        "an active episode finishes, it is not cancelled"
    );
    for status in [
        EpisodeStatus::Planned,
        EpisodeStatus::Waitlist,
        EpisodeStatus::Active,
        EpisodeStatus::Onhold,
        EpisodeStatus::Finished,
        EpisodeStatus::Cancelled,
        EpisodeStatus::EnteredInError,
    ] {
        assert_eq!(EpisodeStatus::from_code(status.code()), Some(status));
    }
    assert_eq!(EpisodeStatus::from_code("closed"), None);
}

#[test]
fn a_transfer_finishes_the_episode_here_and_names_where_the_patient_went() {
    let program = art();
    let (active, _) = enrol(&enrolment(&program, true)).expect("enrolled");
    let (left, event) = active
        .transfer_out("MBR001", at("2027-03-01T00:00:00Z"))
        .expect("transferred");
    assert_eq!(left.status, EpisodeStatus::Finished);
    assert_eq!(left.transferred_to.as_deref(), Some("MBR001"));
    assert_eq!(event.name(), "programs.transferred-out.v1");
    assert_eq!(
        left.transfer_out("MBR002", at("2027-03-02T00:00:00Z")),
        Err(ProgramRefused::CannotTransfer)
    );
    assert_eq!(
        active.transfer_out("KAM013", at("2027-03-01T00:00:00Z")),
        Err(ProgramRefused::CannotTransfer)
    );

    // The new facility's episode continues the old one.
    let mut arriving = enrolment(&program, true);
    arriving.facility = "MBR001".into();
    arriving.transferred_from = Some(format!("EpisodeOfCare/{}", active.id));
    let (continued, _) = enrol(&arriving).expect("enrolled");
    assert_eq!(
        continued.transferred_from,
        Some(format!("EpisodeOfCare/{}", active.id))
    );
}

#[test]
fn regimens_cite_an_approved_source() {
    let program = art();
    let regimens = vec![
        (
            "https://health.go.ug/fhir/ActivityDefinition/tdf-3tc-dtg".to_owned(),
            vec!["https://health.go.ug/protocols/consolidated-hiv-2024".to_owned()],
        ),
        (
            "https://health.go.ug/fhir/ActivityDefinition/home-brew".to_owned(),
            vec!["https://example.org/blog".to_owned()],
        ),
        (
            "https://health.go.ug/fhir/ActivityDefinition/uncited".to_owned(),
            vec![],
        ),
    ];
    assert_eq!(
        unapproved_regimens(&program, &regimens),
        [
            "https://health.go.ug/fhir/ActivityDefinition/home-brew",
            "https://health.go.ug/fhir/ActivityDefinition/uncited"
        ]
    );
}

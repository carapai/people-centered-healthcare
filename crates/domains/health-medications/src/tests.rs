use super::*;

fn at(text: &str) -> Timestamp {
    text.parse().expect("timestamp")
}

fn medicine(text: &str) -> Medicine {
    Medicine {
        text: text.into(),
        system: None,
        code: None,
    }
}

fn request(id: &str, what: Medicine, when: &str) -> Prescribe {
    Prescribe {
        id: id.into(),
        patient: "Patient/p-1".into(),
        encounter: Some("Encounter/e-1".into()),
        facility: "KAM013".into(),
        medicine: what,
        dosage: " 4 tablets twice daily ".into(),
        days: Some(3),
        prescriber: "clinician-1".into(),
        at: at(when),
        based_on: None,
    }
}

#[test]
fn a_prescription_is_written_active_with_no_more_than_its_event() {
    let (prescription, event) = prescribe(&request(
        "rx1",
        medicine("Artemether/lumefantrine"),
        "2026-11-05T10:00:00Z",
    ))
    .expect("prescribed");
    assert_eq!(prescription.status, PrescriptionStatus::Active);
    assert_eq!(prescription.dosage, "4 tablets twice daily");
    assert_eq!(event.name(), "medications.prescribed.v1");
    let mut blank = request("rx2", medicine(" "), "2026-11-05T10:00:00Z");
    assert_eq!(prescribe(&blank), Err(PrescriptionRefused::Incomplete));
    blank = request("rx2", medicine("Paracetamol"), "2026-11-05T10:00:00Z");
    blank.dosage = String::new();
    assert_eq!(prescribe(&blank), Err(PrescriptionRefused::Incomplete));
    let mut no_course = request("rx3", medicine("Paracetamol"), "2026-11-05T10:00:00Z");
    no_course.days = Some(0);
    assert_eq!(prescribe(&no_course), Err(PrescriptionRefused::NoCourse));
}

#[test]
fn the_lifecycle_follows_r5() {
    use PrescriptionStatus::{Active, Cancelled, Completed, EnteredInError, OnHold, Stopped};
    let (active, _) = prescribe(&request(
        "rx1",
        medicine("Paracetamol"),
        "2026-11-05T10:00:00Z",
    ))
    .expect("prescribed");
    let (held, event) = active
        .transition(OnHold, at("2026-11-06T00:00:00Z"))
        .expect("on hold");
    assert_eq!(event.name(), "medications.status-changed.v1");
    assert_eq!(
        held.transition(Active, at("2026-11-07T00:00:00Z"))
            .expect("resumed")
            .0
            .status,
        Active
    );
    let (stopped, _) = held
        .transition(Stopped, at("2026-11-07T00:00:00Z"))
        .expect("stopped");
    assert_eq!(
        stopped.transition(Active, at("2026-11-08T00:00:00Z")),
        Err(PrescriptionRefused::Transition {
            from: "stopped",
            to: "active"
        })
    );
    assert!(
        stopped
            .transition(EnteredInError, at("2026-11-08T00:00:00Z"))
            .is_err(),
        "only an open prescription is entered in error"
    );
    assert!(
        active
            .transition(EnteredInError, at("2026-11-08T00:00:00Z"))
            .is_ok()
    );
    assert!(
        held.transition(Completed, at("2026-11-08T00:00:00Z"))
            .is_err(),
        "a held prescription resumes before it completes"
    );
    for status in [
        Active,
        OnHold,
        Cancelled,
        Completed,
        Stopped,
        EnteredInError,
    ] {
        assert_eq!(PrescriptionStatus::from_code(status.code()), Some(status));
    }
    assert_eq!(PrescriptionStatus::from_code("draft"), None);
}

#[test]
fn safety_checks_warn_of_duplicates_and_allergies() {
    let coded = |code: &str| Medicine {
        text: "Amoxicillin 500 mg".into(),
        system: Some("http://snomed.info/sct".into()),
        code: Some(code.into()),
    };
    let (open, _) =
        prescribe(&request("rx1", coded("27658006"), "2026-11-05T10:00:00Z")).expect("prescribed");
    let (stopped, _) = open
        .transition(PrescriptionStatus::Stopped, at("2026-11-05T11:00:00Z"))
        .expect("stopped");
    let (new, _) =
        prescribe(&request("rx2", coded("27658006"), "2026-11-05T12:00:00Z")).expect("prescribed");
    let patient = Patient {
        prescriptions: vec![open.clone()],
        allergies: vec!["Amoxicillin".into(), " ".into()],
    };
    let warnings = BasicChecks.check(&new, &patient);
    let codes: Vec<&str> = warnings.iter().map(|w| w.code.as_str()).collect();
    assert_eq!(codes, ["duplicate-therapy", "allergy"]);
    assert!(warnings[1].message.contains("allergic to Amoxicillin"));

    // Stopped medicines and other codes are no duplicate; a different medicine, no allergy.
    let quiet = Patient {
        prescriptions: vec![stopped],
        allergies: vec!["Penicillin G".into()],
    };
    assert_eq!(BasicChecks.check(&new, &quiet), vec![]);
    assert!(
        BasicChecks
            .check(
                &new,
                &Patient {
                    prescriptions: vec![open],
                    allergies: vec![]
                }
            )
            .len()
            == 1
    );
    assert!(medicine("Paracetamol").same_as(&medicine(" paracetamol ")));
    assert!(!coded("1").same_as(&coded("2")));
}

#[test]
fn the_worklist_is_the_facilitys_open_prescriptions_oldest_first() {
    let (newer, _) =
        prescribe(&request("rx2", medicine("B"), "2026-11-05T11:00:00Z")).expect("prescribed");
    let (older, _) =
        prescribe(&request("rx1", medicine("A"), "2026-11-05T10:00:00Z")).expect("prescribed");
    let (done, _) =
        prescribe(&request("rx3", medicine("C"), "2026-11-05T09:00:00Z")).expect("prescribed");
    let (done, _) = done
        .transition(PrescriptionStatus::Completed, at("2026-11-05T12:00:00Z"))
        .expect("completed");
    let mut elsewhere = older.clone();
    elsewhere.id = "rx4".into();
    elsewhere.facility = "MBR001".into();
    let ids: Vec<String> = worklist(&[newer, older, done, elsewhere], "KAM013")
        .into_iter()
        .map(|p| p.id)
        .collect();
    assert_eq!(ids, ["rx1", "rx2"]);
}

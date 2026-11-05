use super::*;

fn at(text: &str) -> Timestamp {
    text.parse().expect("timestamp")
}

fn done(by: &str, when: &str) -> Done {
    Done {
        by: by.into(),
        at: at(when),
    }
}

fn coded(text: &str) -> Coded {
    Coded {
        text: text.into(),
        system: None,
        code: None,
    }
}

fn hb(value: &str) -> ResultEntry {
    ResultEntry {
        code: coded("Haemoglobin"),
        value: ResultValue::Quantity {
            value: value.into(),
            unit: "g/dL".into(),
        },
        reference: Some(Range {
            low: Some("11.5".into()),
            high: Some("16.5".into()),
        }),
        critical: Some(Range {
            low: Some("7.0".into()),
            high: Some("20.0".into()),
        }),
        flag: None,
    }
}

fn ordered() -> LabTest {
    order(Order {
        id: "sr-1".into(),
        patient: "Patient/p-1".into(),
        encounter: Some("Encounter/e-1".into()),
        facility: "KAM013".into(),
        test: coded("Full blood count"),
        ordered: done("clinician-1", "2026-11-05T10:00:00Z"),
    })
    .expect("ordered")
}

fn collected() -> LabTest {
    ordered()
        .collect(Specimen {
            id: "sp-1".into(),
            kind: coded("Venous blood"),
            collected: done("phlebotomist-1", "2026-11-05T10:15:00Z"),
        })
        .expect("collected")
        .0
}

#[test]
fn results_are_flagged_against_their_ranges_in_exact_decimals() {
    assert_eq!(flag(&hb("13.2")), Ok(Flag::Normal));
    assert_eq!(
        flag(&hb("11.5")),
        Ok(Flag::Normal),
        "the limit is inside the range"
    );
    assert_eq!(flag(&hb("11.49")), Ok(Flag::Low));
    assert_eq!(flag(&hb("17.0")), Ok(Flag::High));
    assert_eq!(flag(&hb("6.9")), Ok(Flag::CriticalLow));
    assert_eq!(flag(&hb("20.1")), Ok(Flag::CriticalHigh));
    assert_eq!(
        flag(&hb("seven")),
        Err(LabRefused::NotANumber("seven".into()))
    );
    let unbounded = ResultEntry {
        reference: None,
        critical: None,
        ..hb("99")
    };
    assert_eq!(flag(&unbounded), Ok(Flag::Normal));

    let rdt = |given: Option<Flag>| ResultEntry {
        code: coded("Malaria RDT"),
        value: ResultValue::Coded {
            concept: coded("Positive"),
        },
        reference: None,
        critical: None,
        flag: given,
    };
    assert_eq!(flag(&rdt(None)), Ok(Flag::Normal));
    assert_eq!(flag(&rdt(Some(Flag::High))), Ok(Flag::Abnormal));
    assert_eq!(flag(&rdt(Some(Flag::CriticalHigh))), Ok(Flag::Critical));
    for f in [
        Flag::Normal,
        Flag::Low,
        Flag::High,
        Flag::CriticalLow,
        Flag::CriticalHigh,
        Flag::Abnormal,
        Flag::Critical,
    ] {
        assert!(!f.code().is_empty());
    }
}

#[test]
fn a_test_runs_from_order_to_review_and_is_released_only_once_verified() {
    let test = collected();
    assert_eq!(test.stage, Stage::Collected);
    assert!(!test.released());
    let (resulted, event) = test
        .result(vec![hb("13.2")], done("analyst-1", "2026-11-05T11:00:00Z"))
        .expect("resulted");
    assert_eq!(event.name(), "laboratory.resulted.v1");
    assert!(!resulted.released(), "not before verification");
    // Corrected before verification.
    let (corrected, _) = resulted
        .result(vec![hb("12.9")], done("analyst-1", "2026-11-05T11:05:00Z"))
        .expect("corrected");
    assert_eq!(
        corrected.verify(done("analyst-1", "2026-11-05T11:10:00Z")),
        Err(LabRefused::SameVerifier)
    );
    let (verified, event) = corrected
        .verify(done("analyst-2", "2026-11-05T11:10:00Z"))
        .expect("verified");
    assert!(verified.released());
    assert_eq!(
        event,
        LabEvent::Verified {
            test: "sr-1".into(),
            patient: "Patient/p-1".into(),
            critical: false,
            at: at("2026-11-05T11:10:00Z")
        }
    );
    assert!(
        verified
            .result(vec![hb("1")], done("analyst-1", "2026-11-05T12:00:00Z"))
            .is_err(),
        "verified results are not edited"
    );
    let (reviewed, event) = verified
        .review(done("clinician-1", "2026-11-05T12:00:00Z"))
        .expect("reviewed");
    assert_eq!(
        (reviewed.stage, event.name()),
        (Stage::Reviewed, "laboratory.reviewed.v1")
    );
    assert!(reviewed.released());
}

#[test]
fn a_critical_result_is_verified_as_critical() {
    let (resulted, _) = collected()
        .result(vec![hb("6.2")], done("analyst-1", "2026-11-05T11:00:00Z"))
        .expect("resulted");
    assert!(resulted.is_critical());
    let (_, event) = resulted
        .verify(done("analyst-2", "2026-11-05T11:10:00Z"))
        .expect("verified");
    assert!(matches!(event, LabEvent::Verified { critical: true, .. }));
}

#[test]
fn steps_out_of_order_and_incomplete_steps_are_refused() {
    let test = ordered();
    assert_eq!(
        test.result(vec![hb("13")], done("a", "2026-11-05T11:00:00Z")),
        Err(LabRefused::Step {
            stage: "ordered",
            step: "resulted"
        })
    );
    assert!(test.verify(done("a", "2026-11-05T11:00:00Z")).is_err());
    assert!(test.review(done("a", "2026-11-05T11:00:00Z")).is_err());
    assert!(
        test.reject("haemolysed", at("2026-11-05T11:00:00Z"))
            .is_err(),
        "nothing collected to reject"
    );
    assert_eq!(
        collected().result(vec![], done("a", "2026-11-05T11:00:00Z")),
        Err(LabRefused::NoResults)
    );
    assert_eq!(
        collected().result(vec![hb(" ")], done("a", "2026-11-05T11:00:00Z")),
        Err(LabRefused::NoResults)
    );
    assert!(
        collected()
            .collect(Specimen {
                id: "sp-2".into(),
                kind: coded("Urine"),
                collected: done("p", "2026-11-05T11:00:00Z")
            })
            .is_err()
    );
    assert!(
        test.collect(Specimen {
            id: "sp-2".into(),
            kind: coded(" "),
            collected: done("p", "2026-11-05T11:00:00Z")
        })
        .is_err()
    );
    assert!(
        order(Order {
            test: coded(""),
            ..Order {
                id: "x".into(),
                patient: "Patient/p".into(),
                encounter: None,
                facility: "F".into(),
                test: coded("t"),
                ordered: done("c", "2026-11-05T10:00:00Z")
            }
        })
        .is_err()
    );

    let (rejected, event) = collected()
        .reject("haemolysed", at("2026-11-05T11:00:00Z"))
        .expect("rejected");
    assert_eq!(
        (rejected.stage, rejected.reason.as_deref(), event.name()),
        (Stage::Rejected, Some("haemolysed"), "laboratory.closed.v1")
    );
    assert_eq!(
        collected().reject(" ", at("2026-11-05T11:00:00Z")),
        Err(LabRefused::Missing("a reason"))
    );
    let (cancelled, _) = ordered()
        .cancel("ordered in error", at("2026-11-05T11:00:00Z"))
        .expect("cancelled");
    assert_eq!(cancelled.stage, Stage::Cancelled);
    let (resulted, _) = collected()
        .result(vec![hb("13")], done("a", "2026-11-05T11:00:00Z"))
        .expect("resulted");
    assert!(
        resulted.cancel("late", at("2026-11-05T11:30:00Z")).is_err(),
        "not after results"
    );
    for stage in [
        Stage::Ordered,
        Stage::Collected,
        Stage::Resulted,
        Stage::Verified,
        Stage::Reviewed,
        Stage::Rejected,
        Stage::Cancelled,
    ] {
        assert!(!stage.code().is_empty());
    }
}

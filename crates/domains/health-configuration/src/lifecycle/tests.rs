use health_time::Timestamp;

use super::*;

fn now() -> Timestamp {
    "2026-11-04T08:00:00Z".parse().expect("timestamp")
}

fn draft(kind: PackageKind) -> Lifecycle {
    let version = "1.2.0".parse().expect("version");
    let (lifecycle, record) =
        Lifecycle::create("ug.moh.anc", version, kind, "ann", "sha-a", now()).expect("created");
    assert_eq!(record.step, Step::Created);
    assert_eq!(record.to, Stage::Draft);
    lifecycle
}

/// Takes a draft through submission, validation, simulation and review by `reviewer`.
fn to_approval(lifecycle: &mut Lifecycle, reviewer: &str) {
    lifecycle.submit("ann", now()).expect("submitted");
    lifecycle.validated(vec![], now()).expect("validated");
    lifecycle.simulated(vec![], now()).expect("simulated");
    lifecycle.review(reviewer, now()).expect("reviewed");
    assert_eq!(lifecycle.stage, Stage::Approval);
}

#[test]
fn clinical_content_needs_two_distinct_approvers_who_did_not_author_it() {
    for kind in [
        PackageKind::NationalCore,
        PackageKind::Terminology,
        PackageKind::Program,
    ] {
        let mut lifecycle = draft(kind);
        assert!(is_clinical(kind));
        to_approval(&mut lifecycle, "rita");

        assert_eq!(lifecycle.approve("ann", now()), Err(Refused::Author));
        lifecycle.approve("bob", now()).expect("first approval");
        assert_eq!(
            lifecycle.approve("bob", now()),
            Err(Refused::AlreadyApproved("bob".into()))
        );
        assert_eq!(
            lifecycle.publish("ops", "sha-a", now()),
            Err(Refused::NotApproved { have: 1, need: 2 })
        );
        lifecycle.approve("cara", now()).expect("second approval");
        let record = lifecycle.publish("ops", "sha-a", now()).expect("published");
        assert_eq!(record.step, Step::Published);
        assert_eq!(
            (record.from, record.to),
            (Stage::Approval, Stage::Published)
        );
    }
}

#[test]
fn configuration_needs_one_approver() {
    for kind in [PackageKind::CountryConfiguration, PackageKind::Override] {
        let mut lifecycle = draft(kind);
        assert!(!is_clinical(kind));
        to_approval(&mut lifecycle, "rita");
        lifecycle.approve("bob", now()).expect("approved");
        lifecycle.publish("ops", "sha-a", now()).expect("published");
        assert_eq!(lifecycle.stage, Stage::Published);
    }
}

#[test]
fn no_author_reviews_and_a_co_author_counts_as_an_author() {
    let mut lifecycle = draft(PackageKind::Program);
    lifecycle.revise("dan", "sha-b", now()).expect("revised");
    assert_eq!(lifecycle.content, "sha-b");
    lifecycle.submit("dan", now()).expect("a co-author submits");
    lifecycle.validated(vec![], now()).expect("validated");
    lifecycle.simulated(vec![], now()).expect("simulated");
    assert_eq!(lifecycle.review("ann", now()), Err(Refused::Author));
    assert_eq!(lifecycle.review("dan", now()), Err(Refused::Author));
    assert_eq!(lifecycle.review(" ", now()), Err(Refused::Incomplete));
    lifecycle.review("rita", now()).expect("reviewed");
    assert_eq!(lifecycle.approve("dan", now()), Err(Refused::Author));
}

#[test]
fn only_an_author_submits() {
    let mut lifecycle = draft(PackageKind::Program);
    assert_eq!(lifecycle.submit("eve", now()), Err(Refused::NotAuthor));
}

#[test]
fn failures_return_to_draft_and_a_return_clears_review_and_approvals() {
    let mut lifecycle = draft(PackageKind::Program);
    lifecycle.submit("ann", now()).expect("submitted");
    let record = lifecycle
        .validated(vec!["Questionnaire/anc: no snapshot".into()], now())
        .expect("recorded");
    assert!(matches!(record.step, Step::ValidationFailed { .. }));
    assert_eq!((record.by.as_str(), record.to), (SYSTEM, Stage::Draft));

    lifecycle.submit("ann", now()).expect("resubmitted");
    lifecycle.validated(vec![], now()).expect("validated");
    let record = lifecycle
        .simulated(vec!["tests/anc-visit: wrong Observation".into()], now())
        .expect("recorded");
    assert!(matches!(record.step, Step::SimulationFailed { .. }));
    assert_eq!(lifecycle.stage, Stage::Draft);

    to_approval(&mut lifecycle, "rita");
    lifecycle.approve("bob", now()).expect("approved");
    assert_eq!(
        lifecycle.request_changes("cara", " ", now()),
        Err(Refused::Incomplete)
    );
    assert_eq!(
        lifecycle.request_changes("ann", "mine", now()),
        Err(Refused::Author)
    );
    let record = lifecycle
        .request_changes("cara", "the danger signs are incomplete", now())
        .expect("sent back");
    assert_eq!((record.from, record.to), (Stage::Approval, Stage::Draft));
    assert_eq!(lifecycle.reviewer, None);
    assert!(lifecycle.approvers.is_empty());

    to_approval(&mut lifecycle, "rita");
    lifecycle.approve("bob", now()).expect("approved again");
    assert_eq!(
        lifecycle.publish("ops", "sha-a", now()),
        Err(Refused::NotApproved { have: 1, need: 2 })
    );
}

#[test]
fn content_changes_only_in_draft_and_publication_signs_what_was_approved() {
    let mut lifecycle = draft(PackageKind::CountryConfiguration);
    lifecycle.submit("ann", now()).expect("submitted");
    assert_eq!(
        lifecycle.revise("ann", "sha-b", now()),
        Err(Refused::Stage(Stage::Validation))
    );
    lifecycle.validated(vec![], now()).expect("validated");
    lifecycle.simulated(vec![], now()).expect("simulated");
    lifecycle.review("rita", now()).expect("reviewed");
    lifecycle.approve("bob", now()).expect("approved");
    assert_eq!(
        lifecycle.publish("ops", "sha-b", now()),
        Err(Refused::ContentChanged {
            approved: "sha-a".into(),
            signed: "sha-b".into()
        })
    );
    lifecycle.publish("ops", "sha-a", now()).expect("published");
    assert_eq!(
        lifecycle.approve("cara", now()),
        Err(Refused::Stage(Stage::Published))
    );
    assert_eq!(
        lifecycle.request_changes("cara", "late", now()),
        Err(Refused::Stage(Stage::Published))
    );
    assert_eq!(
        lifecycle.publish("ops", "sha-a", now()),
        Err(Refused::Stage(Stage::Published))
    );
}

#[test]
fn steps_out_of_order_are_refused() {
    let mut lifecycle = draft(PackageKind::Program);
    assert_eq!(
        lifecycle.validated(vec![], now()),
        Err(Refused::Stage(Stage::Draft))
    );
    assert_eq!(
        lifecycle.simulated(vec![], now()),
        Err(Refused::Stage(Stage::Draft))
    );
    assert_eq!(
        lifecycle.review("rita", now()),
        Err(Refused::Stage(Stage::Draft))
    );
    assert_eq!(
        lifecycle.approve("bob", now()),
        Err(Refused::Stage(Stage::Draft))
    );
    assert_eq!(
        lifecycle.request_changes("bob", "why", now()),
        Err(Refused::Stage(Stage::Draft))
    );
    assert_eq!(
        lifecycle.publish("ops", "sha-a", now()),
        Err(Refused::Stage(Stage::Draft))
    );
}

#[test]
fn creation_needs_who_and_what() {
    let version: Version = "1.0.0".parse().expect("version");
    for (package, by, content) in [(" ", "ann", "sha"), ("p", "", "sha"), ("p", "ann", " ")] {
        assert_eq!(
            Lifecycle::create(
                package,
                version.clone(),
                PackageKind::Program,
                by,
                content,
                now()
            ),
            Err(Refused::Incomplete)
        );
    }
}

#[test]
fn records_serialise_with_their_step() {
    let mut lifecycle = draft(PackageKind::Program);
    lifecycle.submit("ann", now()).expect("submitted");
    let record = lifecycle
        .validated(vec!["bad".into()], now())
        .expect("recorded");
    let json = serde_json::to_value(&record).expect("json");
    assert_eq!(json["step"], "validation-failed");
    assert_eq!(json["problems"][0], "bad");
    assert_eq!(json["version"], "1.2.0");
    assert_eq!(json["from"], "validation");
    assert_eq!(json["to"], "draft");
    let back: Record = serde_json::from_value(json).expect("read back");
    assert_eq!(back, record);
}

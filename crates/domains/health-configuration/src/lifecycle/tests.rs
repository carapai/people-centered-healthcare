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
fn only_the_holder_edits_checks_submits_and_hands_over() {
    let mut lifecycle = draft(PackageKind::Program);
    assert_eq!(lifecycle.holder, "ann");
    for action in [
        Action::Edit,
        Action::Check,
        Action::Submit,
        Action::HandOver,
        Action::Withdraw,
    ] {
        assert_eq!(lifecycle.check(action, "ann"), Ok(()), "{action:?}");
        assert_eq!(
            lifecycle.check(action, "eve"),
            Err(Refused::NotHolder),
            "{action:?}"
        );
    }
    assert_eq!(
        lifecycle.revise("eve", "sha-b", now()),
        Err(Refused::NotHolder)
    );
    assert_eq!(lifecycle.submit("eve", now()), Err(Refused::NotHolder));
    assert_eq!(
        lifecycle.hand_over("eve", "dan", now()),
        Err(Refused::NotHolder)
    );
    assert_eq!(lifecycle.withdraw("eve", now()), Err(Refused::NotHolder));
    lifecycle
        .revise("ann", "sha-b", now())
        .expect("the holder revises");
    assert_eq!(lifecycle.authors, BTreeSet::from(["ann".to_owned()]));
}

#[test]
fn an_unaccepted_handover_leaves_the_draft_with_its_holder() {
    let mut lifecycle = draft(PackageKind::Program);
    assert_eq!(
        lifecycle.hand_over("ann", "ann", now()),
        Err(Refused::AlreadyHolder("ann".into()))
    );
    assert_eq!(
        lifecycle.hand_over("ann", " ", now()),
        Err(Refused::Incomplete)
    );
    let record = lifecycle.hand_over("ann", "dan", now()).expect("offered");
    assert_eq!(record.step, Step::HandoverOffered { to: "dan".into() });
    assert_eq!((record.from, record.to), (Stage::Draft, Stage::Draft));

    // Until dan accepts, ann holds it: she edits and submits, and dan does not.
    assert_eq!(lifecycle.holder, "ann");
    assert_eq!(lifecycle.offered_to.as_deref(), Some("dan"));
    assert_eq!(
        lifecycle.check(Action::Edit, "dan"),
        Err(Refused::NotHolder)
    );
    assert_eq!(lifecycle.check(Action::Edit, "ann"), Ok(()));
    assert!(!lifecycle.authors.contains("dan"));
    // Nobody else accepts it.
    assert_eq!(
        lifecycle.accept_handover("eve", now()),
        Err(Refused::NotOffered)
    );
    assert_eq!(
        lifecycle.check(Action::AcceptHandover, "ann"),
        Err(Refused::NotOffered)
    );

    // Called off, by dan here, it stays with ann.
    let record = lifecycle.decline_handover("dan", now()).expect("declined");
    assert_eq!(record.step, Step::HandoverDeclined { to: "dan".into() });
    assert_eq!(lifecycle.offered_to, None);
    assert_eq!(
        lifecycle.accept_handover("dan", now()),
        Err(Refused::NotOffered)
    );
    assert_eq!(
        lifecycle.decline_handover("ann", now()),
        Err(Refused::NotOffered)
    );
    assert_eq!(lifecycle.holder, "ann");
}

#[test]
fn a_former_holder_counts_as_an_author_and_cannot_review_or_approve() {
    let mut lifecycle = draft(PackageKind::Program);
    lifecycle.hand_over("ann", "dan", now()).expect("offered");
    let record = lifecycle.accept_handover("dan", now()).expect("accepted");
    assert_eq!(record.step, Step::HandedOver { from: "ann".into() });
    assert_eq!(record.by, "dan");
    assert_eq!(lifecycle.holder, "dan");
    assert_eq!(lifecycle.offered_to, None);
    assert_eq!(
        lifecycle.authors,
        BTreeSet::from(["ann".to_owned(), "dan".to_owned()])
    );

    // ann no longer holds it, and dan does.
    assert_eq!(
        lifecycle.revise("ann", "sha-b", now()),
        Err(Refused::NotHolder)
    );
    assert_eq!(lifecycle.submit("ann", now()), Err(Refused::NotHolder));
    lifecycle
        .revise("dan", "sha-b", now())
        .expect("dan revises");
    lifecycle.submit("dan", now()).expect("dan submits");
    lifecycle.validated(vec![], now()).expect("validated");
    lifecycle.simulated(vec![], now()).expect("simulated");
    assert_eq!(lifecycle.review("ann", now()), Err(Refused::Author));
    assert_eq!(lifecycle.review("dan", now()), Err(Refused::Author));
    assert_eq!(lifecycle.review(" ", now()), Err(Refused::Incomplete));
    lifecycle.review("rita", now()).expect("reviewed");
    // The former holder may not approve it.
    assert_eq!(lifecycle.approve("ann", now()), Err(Refused::Author));
    assert_eq!(
        lifecycle.check(Action::Approve, "ann"),
        Err(Refused::Author)
    );
    assert_eq!(lifecycle.approve("dan", now()), Err(Refused::Author));
    lifecycle.approve("bob", now()).expect("approved");
}

#[test]
fn a_manager_reassigns_a_draft_with_a_reason_and_the_new_holder_is_an_author() {
    let mut lifecycle = draft(PackageKind::Program);
    lifecycle.hand_over("ann", "dan", now()).expect("offered");
    assert_eq!(
        lifecycle.reassign("mo", "fay", " ", now()),
        Err(Refused::Incomplete)
    );
    assert_eq!(
        lifecycle.reassign("mo", "ann", "away", now()),
        Err(Refused::AlreadyHolder("ann".into()))
    );
    let record = lifecycle
        .reassign("mo", "fay", "ann is on leave until March", now())
        .expect("reassigned");
    assert_eq!(
        record.step,
        Step::Reassigned {
            from: "ann".into(),
            to: "fay".into(),
            reason: "ann is on leave until March".into(),
        }
    );
    assert_eq!(record.by, "mo");
    assert_eq!(lifecycle.holder, "fay");
    // The pending offer is gone; the manager is not an author.
    assert_eq!(lifecycle.offered_to, None);
    assert_eq!(
        lifecycle.authors,
        BTreeSet::from(["ann".to_owned(), "fay".to_owned()])
    );
    let json = serde_json::to_value(&record).expect("json");
    assert_eq!(json["step"], "reassigned");
    assert_eq!(json["reason"], "ann is on leave until March");
    assert_eq!(
        (json["previous-holder"].as_str(), json["holder"].as_str()),
        (Some("ann"), Some("fay"))
    );
    assert_eq!(
        (json["from"].as_str(), json["to"].as_str()),
        (Some("draft"), Some("draft"))
    );
    let back: Record = serde_json::from_value(json).expect("read back");
    assert_eq!(back, record);

    lifecycle.submit("fay", now()).expect("submitted");
    assert_eq!(
        lifecycle.reassign("mo", "dan", "late", now()),
        Err(Refused::Stage(Stage::Validation))
    );
}

#[test]
fn the_holder_withdraws_an_unpublished_draft_and_it_ends_there() {
    let mut lifecycle = draft(PackageKind::Program);
    to_approval(&mut lifecycle, "rita");
    let record = lifecycle.withdraw("ann", now()).expect("withdrawn");
    assert_eq!(record.step, Step::Withdrawn);
    assert_eq!(
        (record.from, record.to),
        (Stage::Approval, Stage::Withdrawn)
    );
    assert_eq!(lifecycle.withdrawn_at, Some(now()));
    // Nothing further happens to it.
    for action in Action::ALL {
        assert!(lifecycle.check(action, "ann").is_err(), "{action:?}");
    }
    assert_eq!(
        lifecycle.withdraw("ann", now()),
        Err(Refused::Stage(Stage::Withdrawn))
    );

    let mut published = draft(PackageKind::CountryConfiguration);
    to_approval(&mut published, "rita");
    published.approve("bob", now()).expect("approved");
    published.publish("ops", "sha-a", now()).expect("published");
    assert_eq!(
        published.withdraw("ann", now()),
        Err(Refused::Stage(Stage::Published))
    );
}

#[test]
fn a_lifecycle_kept_before_the_draft_holder_is_held_by_its_author() {
    let json = serde_json::json!({
        "package": "ug.moh.anc", "version": "1.2.0", "kind": "program", "stage": "draft",
        "authors": ["ann"], "content": "sha-a", "reviewer": null, "approvers": []
    });
    let lifecycle: Lifecycle = serde_json::from_value(json).expect("read");
    assert_eq!(lifecycle.holder, "ann");
    assert_eq!(lifecycle.offered_to, None);
    let back: Lifecycle =
        serde_json::from_value(serde_json::to_value(&lifecycle).expect("json")).expect("read");
    assert_eq!(back, lifecycle);
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

#[test]
fn what_a_person_may_do_is_checked_by_the_rules_the_transitions_enforce() {
    let mut lifecycle = draft(PackageKind::Program);
    assert_eq!(lifecycle.check(Action::Submit, "ann"), Ok(()));
    assert_eq!(
        lifecycle.check(Action::Submit, "eve"),
        Err(Refused::NotHolder)
    );
    assert_eq!(
        lifecycle.check(Action::Approve, "bob"),
        Err(Refused::Stage(Stage::Draft))
    );
    to_approval(&mut lifecycle, "rita");
    // The author is refused review and approval; the stage is right, so that is the reason.
    assert_eq!(
        lifecycle.check(Action::Approve, "ann"),
        Err(Refused::Author)
    );
    assert_eq!(
        lifecycle.check(Action::RequestChanges, "ann"),
        Err(Refused::Author)
    );
    assert_eq!(lifecycle.check(Action::Approve, "bob"), Ok(()));
    assert_eq!(
        lifecycle.check(Action::Publish, "ops"),
        Err(Refused::NotApproved { have: 0, need: 2 })
    );
    lifecycle.approve("bob", now()).expect("approved");
    assert_eq!(
        lifecycle.check(Action::Approve, "bob"),
        Err(Refused::AlreadyApproved("bob".into()))
    );
    // A check changes nothing.
    let before = lifecycle.clone();
    for action in Action::ALL {
        let _ = lifecycle.check(action, "cara");
    }
    assert_eq!(lifecycle, before);
}

#[test]
fn check_now_records_both_results_and_leaves_the_draft_where_it_is() {
    let mut lifecycle = draft(PackageKind::Program);
    let before = lifecycle.clone();
    let record = lifecycle
        .checked("ann", vec!["a problem".into()], vec![], now())
        .expect("checked");
    assert_eq!(
        record.step,
        Step::Checked {
            validation: vec!["a problem".into()],
            simulation: vec![],
        }
    );
    assert_eq!((record.from, record.to), (Stage::Draft, Stage::Draft));
    assert_eq!(record.by, "ann");
    assert_eq!(record.content, "sha-a");
    assert_eq!(lifecycle, before, "a check changes nothing");

    // Only a draft is checked; once submitted, submission's own run decides.
    lifecycle.submit("ann", now()).expect("submitted");
    assert_eq!(
        lifecycle.checked("ann", vec![], vec![], now()),
        Err(Refused::Stage(Stage::Validation))
    );
    assert_eq!(
        lifecycle.check(Action::Check, "ann"),
        Err(Refused::Stage(Stage::Validation))
    );
    assert_eq!(
        lifecycle.check(Action::Edit, "ann"),
        Err(Refused::Stage(Stage::Validation))
    );
}

#[test]
fn a_draft_changes_version_until_it_is_submitted() {
    let mut lifecycle = draft(PackageKind::Program);
    let next: Version = "1.3.0".parse().expect("version");
    let record = lifecycle
        .change_version("ann", next.clone(), "sha-b", now())
        .expect("renumbered");
    assert_eq!(
        record.step,
        Step::VersionChanged {
            previous: "1.2.0".parse().expect("version"),
        }
    );
    assert_eq!(record.version, next);
    assert_eq!(lifecycle.version, next);
    assert_eq!(lifecycle.content, "sha-b");
    assert_eq!(lifecycle.stage, Stage::Draft);

    lifecycle.submit("ann", now()).expect("submitted");
    assert_eq!(
        lifecycle.change_version("ann", "1.4.0".parse().expect("version"), "sha-c", now()),
        Err(Refused::Stage(Stage::Validation))
    );
}

#[test]
fn the_holder_keeps_one_change_note_on_a_draft() {
    let mut lifecycle = draft(PackageKind::Program);
    assert_eq!(lifecycle.note, None);
    lifecycle
        .set_note("ann", Some("  Reworded the BP question.  ".into()))
        .expect("noted");
    assert_eq!(lifecycle.note.as_deref(), Some("Reworded the BP question."));
    assert_eq!(lifecycle.stage, Stage::Draft);
    assert_eq!(
        lifecycle.set_note("bob", Some("mine".into())),
        Err(Refused::NotHolder)
    );
    let back: Lifecycle =
        serde_json::from_value(serde_json::to_value(&lifecycle).expect("json")).expect("read");
    assert_eq!(back.note, lifecycle.note);

    lifecycle
        .set_note("ann", Some("   ".into()))
        .expect("cleared");
    assert_eq!(lifecycle.note, None);

    lifecycle.submit("ann", now()).expect("submitted");
    assert_eq!(
        lifecycle.set_note("ann", Some("late".into())),
        Err(Refused::Stage(Stage::Validation))
    );
}

// One discussion per draft (#496): anyone in the Studio comments, naming the artefact if they
// like; the comment is a record like the history's, and moves nothing.
#[test]
fn a_comment_joins_the_history_without_moving_the_draft() {
    let mut lifecycle = draft(PackageKind::Program);
    let record = lifecycle
        .comment(
            "rita",
            "  Should BP be required?  ",
            Some("Questionnaire-anc-visit.json"),
            now(),
        )
        .expect("commented");
    assert_eq!(
        record.step,
        Step::Commented {
            text: "Should BP be required?".into(),
            about: Some("Questionnaire-anc-visit.json".into()),
        }
    );
    assert_eq!((record.from, record.to), (Stage::Draft, Stage::Draft));
    assert_eq!(
        (record.by.as_str(), record.content.as_str()),
        ("rita", "sha-a")
    );
    let json = serde_json::to_value(&record).expect("json");
    assert_eq!(json["step"], "commented");
    assert_eq!(json["about"], "Questionnaire-anc-visit.json");
    let back: Record = serde_json::from_value(json).expect("read back");
    assert_eq!(back, record);

    // A comment on no artefact; a blank one, or by no one, is refused.
    let general = lifecycle
        .comment("ann", "Ready soon", Some("  "), now())
        .expect("commented");
    assert_eq!(
        general.step,
        Step::Commented {
            text: "Ready soon".into(),
            about: None
        }
    );
    assert_eq!(
        lifecycle.comment("ann", "   ", None, now()),
        Err(Refused::Incomplete)
    );
    assert_eq!(
        lifecycle.comment(" ", "hello", None, now()),
        Err(Refused::Incomplete)
    );

    // In approval a comment keeps the review and the approvals.
    to_approval(&mut lifecycle, "rita");
    lifecycle.approve("bob", now()).expect("approved");
    let record = lifecycle
        .comment("bob", "I read the Luganda", None, now())
        .expect("commented");
    assert_eq!((record.from, record.to), (Stage::Approval, Stage::Approval));
    assert_eq!(lifecycle.stage, Stage::Approval);
    assert_eq!(lifecycle.reviewer.as_deref(), Some("rita"));
    assert_eq!(lifecycle.approvers.len(), 1);

    // Once the draft has ended, its discussion is closed.
    lifecycle.withdraw("ann", now()).expect("withdrawn");
    assert_eq!(
        lifecycle.comment("bob", "too late", None, now()),
        Err(Refused::Stage(Stage::Withdrawn))
    );
}

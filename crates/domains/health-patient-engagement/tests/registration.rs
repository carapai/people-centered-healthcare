//! The §59 Registration Request: no automatic link below level3 (ADR 0029).

use health_patient_engagement::registration::{
    Assurance, Matched, Outcome, RegistrationRequest, RequestRefused, Reviewed, Route, Status,
    route,
};

fn submitted(assurance: Assurance) -> RegistrationRequest {
    let mut request = RegistrationRequest::new("task-1", "account-1");
    request
        .submit("QuestionnaireResponse/qr-1")
        .expect("submit");
    request.verified(assurance).expect("verified");
    request
}

#[test]
fn routes_after_matching() {
    let certain = Matched::Certain("p1".into());
    let possible = Matched::Possible(vec!["p1".into(), "p2".into()]);
    assert_eq!(route(Assurance::Level1, &Matched::None), Route::Create);
    assert_eq!(route(Assurance::Level3, &Matched::None), Route::Create);
    assert_eq!(route(Assurance::Level3, &certain), Route::Link("p1".into()));
    assert_eq!(
        route(Assurance::Level1, &certain),
        Route::Review(vec!["p1".into()])
    );
    assert_eq!(
        route(Assurance::Level3, &possible),
        Route::Review(vec!["p1".into(), "p2".into()])
    );
}

#[test]
fn a_phone_account_with_a_certain_match_goes_to_review_not_a_link() {
    let mut request = submitted(Assurance::Level1);
    assert_eq!(
        request.matched(&Matched::Certain("p1".into())),
        Ok(Route::Review(vec!["p1".into()]))
    );
    assert_eq!(request.status, Status::Review);
    // Nothing can complete it as a link without a review.
    assert!(matches!(
        request.complete(Outcome::Linked("p1".into())),
        Err(RequestRefused::WrongState { .. })
    ));
    // A reviewer may not link it either until the holder is verified at level3.
    assert_eq!(
        request.review(Reviewed::Link("p1".into()), "clerk-1"),
        Err(RequestRefused::NeedsLevel3)
    );
    request
        .verified(Assurance::Level3)
        .expect("in-person check");
    assert_eq!(
        request.review(Reviewed::Link("p1".into()), "clerk-1"),
        Ok(Route::Link("p1".into()))
    );
    assert_eq!(
        request.complete(Outcome::Linked("p2".into())),
        Err(RequestRefused::NotTheRoute)
    );
    request
        .complete(Outcome::Linked("p1".into()))
        .expect("linked");
    assert_eq!(request.status, Status::Completed);
    assert_eq!(request.reviewer.as_deref(), Some("clerk-1"));
}

#[test]
fn level3_and_a_certain_match_links_and_no_match_creates() {
    let mut request = submitted(Assurance::Level3);
    assert_eq!(
        request.matched(&Matched::Certain("p1".into())),
        Ok(Route::Link("p1".into()))
    );
    request
        .complete(Outcome::Linked("p1".into()))
        .expect("linked");

    let mut new = submitted(Assurance::Level1);
    assert_eq!(new.matched(&Matched::None), Ok(Route::Create));
    assert_eq!(
        new.complete(Outcome::Linked("p1".into())),
        Err(RequestRefused::NotTheRoute)
    );
    new.complete(Outcome::Created("p-new".into()))
        .expect("created");
    assert_eq!(new.outcome, Some(Outcome::Created("p-new".into())));
}

#[test]
fn steps_happen_in_order() {
    let mut request = RegistrationRequest::new("task-1", "account-1");
    assert!(matches!(
        request.matched(&Matched::None),
        Err(RequestRefused::WrongState { .. })
    ));
    assert!(matches!(
        request.verified(Assurance::Level3),
        Err(RequestRefused::WrongState { .. })
    ));
    request
        .submit("QuestionnaireResponse/qr-1")
        .expect("submit");
    assert!(matches!(
        request.submit("again"),
        Err(RequestRefused::WrongState { .. })
    ));
    // Assurance never falls.
    request.verified(Assurance::Level3).expect("nin");
    request.verified(Assurance::Level1).expect("otp");
    assert_eq!(request.assurance, Assurance::Level3);
    // Staff may create a new patient for any reviewed request.
    let mut review = submitted(Assurance::Level1);
    review
        .matched(&Matched::Possible(vec!["p1".into()]))
        .expect("matched");
    assert_eq!(
        review.review(Reviewed::Create, "clerk-1"),
        Ok(Route::Create)
    );
    review
        .complete(Outcome::Created("p-new".into()))
        .expect("created");
}

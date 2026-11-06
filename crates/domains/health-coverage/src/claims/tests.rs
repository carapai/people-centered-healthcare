use super::*;

fn d(n: i64) -> Decimal {
    Decimal::from(n)
}

fn at(text: &str) -> Timestamp {
    text.parse().expect("timestamp")
}

fn charge(id: &str, when: &str, gross: i64, payer: i64) -> Claimable {
    Claimable {
        charge: id.into(),
        item: format!("{id}-item"),
        quantity: Decimal::ONE,
        gross: d(gross),
        payer_share: d(payer),
        at: at(when),
    }
}

fn header(id: &str, use_: ClaimUse) -> ClaimFor {
    ClaimFor {
        id: id.into(),
        use_,
        patient: "Patient/p1".into(),
        coverage: "primary".into(),
        payer: "Organization/aar".into(),
        account: "Account/a1".into(),
    }
}

fn claim() -> Claim {
    assemble(
        &header("claim-1", ClaimUse::Claim),
        &[
            charge("c2", "2026-11-07T09:00:00Z", 30_000, 27_000),
            charge("c1", "2026-11-06T09:00:00Z", 100_000, 72_000),
            // The patient pays it all: nothing to claim.
            charge("c3", "2026-11-06T10:00:00Z", 5_000, 0),
        ],
        at("2026-11-30T00:00:00Z"),
    )
    .expect("assembled")
}

#[test]
fn a_claim_is_the_coverage_s_shares_in_service_order() {
    let claim = claim();
    let items: Vec<(u32, &str, Decimal)> = claim
        .items
        .iter()
        .map(|i| (i.sequence, i.charge.as_str(), i.claimed))
        .collect();
    assert_eq!(items, [(1, "c1", d(72_000)), (2, "c2", d(27_000))]);
    assert_eq!(
        (claim.total(), claim.status),
        (d(99_000), ClaimStatus::Draft)
    );
    assert_eq!(
        assemble(
            &header("x", ClaimUse::Preauthorization),
            &[charge("c3", "2026-11-06T10:00:00Z", 5_000, 0)],
            at("2026-11-30T00:00:00Z")
        ),
        Err(ClaimRefused::Nothing)
    );
}

#[test]
fn adjudication_settles_in_full_in_part_or_not_at_all() {
    let sent = claim().submitted("AAR-778").expect("submitted");
    assert_eq!(
        sent.submitted("again"),
        Err(ClaimRefused::Status(ClaimStatus::Submitted))
    );

    let full = Adjudication {
        payer_reference: "AAR-778".into(),
        items: vec![
            ItemAdjudication {
                sequence: 1,
                approved: d(72_000),
                reason: None,
            },
            ItemAdjudication {
                sequence: 2,
                approved: d(27_000),
                reason: None,
            },
        ],
        denied: None,
    };
    let paid = sent.adjudicate(&full).expect("settled");
    assert_eq!(
        (paid.claim.status, paid.approved, paid.shortfalls.len()),
        (ClaimStatus::Approved, d(99_000), 0)
    );

    let part = Adjudication {
        payer_reference: "AAR-778".into(),
        items: vec![ItemAdjudication {
            sequence: 1,
            approved: d(60_000),
            reason: Some("above the tariff".into()),
        }],
        denied: None,
    };
    let partly = sent.adjudicate(&part).expect("settled");
    assert_eq!(
        (partly.claim.status, partly.approved),
        (ClaimStatus::Partial, d(60_000))
    );
    assert_eq!(
        partly.shortfalls,
        [
            Shortfall {
                charge: "c1".into(),
                amount: d(12_000),
                reason: Some("above the tariff".into())
            },
            Shortfall {
                charge: "c2".into(),
                amount: d(27_000),
                reason: None
            },
        ]
    );

    let denied = Adjudication {
        payer_reference: "AAR-778".into(),
        items: full.items.clone(),
        denied: Some("policy lapsed".into()),
    };
    let refused = sent.adjudicate(&denied).expect("settled");
    assert_eq!(
        (refused.claim.status, refused.approved),
        (ClaimStatus::Denied, Decimal::ZERO)
    );
    assert!(
        refused
            .shortfalls
            .iter()
            .all(|s| s.reason.as_deref() == Some("policy lapsed"))
    );

    let wrong = Adjudication {
        payer_reference: "AAR-778".into(),
        items: vec![ItemAdjudication {
            sequence: 1,
            approved: d(80_000),
            reason: None,
        }],
        denied: None,
    };
    assert_eq!(
        sent.adjudicate(&wrong),
        Err(ClaimRefused::BadAdjudication(1)),
        "more than claimed"
    );
    assert_eq!(
        claim().adjudicate(&full),
        Err(ClaimRefused::Status(ClaimStatus::Draft)),
        "not sent yet"
    );
}

#[test]
fn an_appeal_claims_what_was_not_paid_and_names_the_original() {
    let sent = claim().submitted("AAR-778").expect("submitted");
    let part = Adjudication {
        payer_reference: "AAR-778".into(),
        items: vec![
            ItemAdjudication {
                sequence: 1,
                approved: d(60_000),
                reason: Some("above the tariff".into()),
            },
            ItemAdjudication {
                sequence: 2,
                approved: d(27_000),
                reason: None,
            },
        ],
        denied: None,
    };
    let settled = sent.adjudicate(&part).expect("settled");
    let appeal = settled
        .claim
        .appeal("claim-2", &settled, at("2026-12-05T00:00:00Z"))
        .expect("appealed");
    assert_eq!(
        (appeal.appeals.as_deref(), appeal.status),
        (Some("claim-1"), ClaimStatus::Draft)
    );
    assert_eq!(
        appeal
            .items
            .iter()
            .map(|i| (i.sequence, i.charge.as_str(), i.claimed))
            .collect::<Vec<_>>(),
        [(1, "c1", d(12_000))]
    );
    let full = Adjudication {
        payer_reference: "AAR-778".into(),
        items: vec![
            ItemAdjudication {
                sequence: 1,
                approved: d(72_000),
                reason: None,
            },
            ItemAdjudication {
                sequence: 2,
                approved: d(27_000),
                reason: None,
            },
        ],
        denied: None,
    };
    let paid = sent.adjudicate(&full).expect("settled");
    assert_eq!(
        paid.claim
            .appeal("claim-3", &paid, at("2026-12-05T00:00:00Z")),
        Err(ClaimRefused::Status(ClaimStatus::Approved))
    );
}

use std::collections::BTreeMap;

use rust_decimal::Decimal;

use super::*;

fn at(text: &str) -> health_time::Timestamp {
    text.parse().expect("timestamp")
}

fn d(value: i64) -> Decimal {
    Decimal::from(value)
}

fn approved() -> Approval {
    Approval::Approved {
        by: "finance-1".into(),
        at: at("2025-12-01T00:00:00Z"),
    }
}

fn since_2026() -> Effective {
    Effective {
        from: at("2026-01-01T00:00:00Z"),
        to: None,
    }
}

fn entry(item: &str, unit: i64) -> Entry {
    Entry {
        item: item.into(),
        unit: d(unit),
        tiers: vec![],
    }
}

fn book(id: &str, version: u32, scope: Scope, entries: Vec<Entry>) -> PriceBook {
    PriceBook {
        id: id.into(),
        version,
        scope,
        currency: "UGX".into(),
        effective: since_2026(),
        approval: approved(),
        author: "finance-author".into(),
        entries,
    }
}

fn kam() -> Scope {
    Scope::Facility("KAM013".into())
}

fn books() -> Vec<PriceBook> {
    let mut ors = entry("ors", 500);
    ors.tiers = vec![Tier {
        min: d(10),
        unit: d(400),
    }];
    let mut expired = book("kam", 1, kam(), vec![entry("consult", 11_000)]);
    expired.effective = Effective {
        from: at("2025-01-01T00:00:00Z"),
        to: Some(at("2026-01-01T00:00:00Z")),
    };
    let mut draft = book("kam", 3, kam(), vec![entry("consult", 99_999)]);
    draft.approval = Approval::Draft;
    vec![
        book(
            "tenant",
            1,
            Scope::Tenant,
            vec![
                entry("consult", 10_000),
                ors,
                entry("anc-visit", 9_000),
                entry("hb-test", 7_000),
            ],
        ),
        book("kam", 2, kam(), vec![entry("consult", 12_000)]),
        expired,
        draft,
    ]
}

fn rule(id: &str, stage: Stage, priority: u32, adjustment: Adjustment) -> PriceRule {
    PriceRule {
        id: id.into(),
        version: 1,
        stage,
        priority,
        items: vec![],
        applicability: None,
        adjustment,
        effective: since_2026(),
        approval: approved(),
        author: "finance-author".into(),
    }
}

fn rules() -> Vec<PriceRule> {
    let mut after_hours = rule(
        "after-hours",
        Stage::Surcharge,
        1,
        Adjustment::Percent { percent: d(20) },
    );
    after_hours.applicability = Some("%afterHours".into());
    let mut staff = rule(
        "staff",
        Stage::Discount,
        2,
        Adjustment::Percent { percent: d(10) },
    );
    staff.applicability = Some("%staff".into());
    let mut ors = rule(
        "ors-discount",
        Stage::Discount,
        3,
        Adjustment::Amount { amount: d(300) },
    );
    ors.items = vec!["ors".into()];
    let mut subsidy = rule(
        "consult-subsidy",
        Stage::Subsidy,
        1,
        Adjustment::Amount { amount: d(500) },
    );
    subsidy.items = vec!["consult".into()];
    vec![
        after_hours,
        rule(
            "anc-package",
            Stage::Discount,
            1,
            Adjustment::Bundle {
                items: vec!["anc-visit".into(), "hb-test".into()],
                price: d(15_000),
                item: "anc-package".into(),
            },
        ),
        staff,
        ors,
        subsidy,
    ]
}

/// Applicability stubbed by expression.
struct Stub(BTreeMap<&'static str, bool>);

impl Applicability for Stub {
    fn applies(&self, expression: &str, _: &Line) -> Result<bool, String> {
        self.0
            .get(expression)
            .copied()
            .ok_or_else(|| format!("no stub for {expression}"))
    }
}

fn stub() -> Stub {
    Stub(BTreeMap::from([("%afterHours", true), ("%staff", false)]))
}

fn line(id: &str, item: &str, quantity: i64) -> Line {
    Line {
        id: id.into(),
        item: item.into(),
        quantity: d(quantity),
    }
}

fn request(lines: Vec<Line>) -> Request {
    Request {
        service_time: at("2026-11-04T20:00:00Z"),
        currency: "UGX".into(),
        minor_units: 0,
        scopes: vec![kam(), Scope::Tenant],
        lines,
        coverages: vec![],
        overrides: vec![],
    }
}

fn coverages() -> Vec<CoverageTerms> {
    vec![
        CoverageTerms {
            coverage: "Coverage/primary".into(),
            excluded: vec![],
            copay: Copay::Percent(d(10)),
            deductible_remaining: d(2_000),
            limit_remaining: Some(d(20_000)),
            estimate: false,
        },
        CoverageTerms {
            coverage: "Coverage/secondary".into(),
            excluded: vec!["ors".into()],
            copay: Copay::None,
            deductible_remaining: Decimal::ZERO,
            limit_remaining: Some(d(1_000)),
            estimate: true,
        },
    ]
}

fn applied(id: &str, version: u32) -> Applied {
    Applied {
        id: id.into(),
        version,
    }
}

fn amounts(line: &PricedLine) -> Vec<(Kind, Decimal)> {
    line.components.iter().map(|c| (c.kind, c.amount)).collect()
}

/// The acceptance fixture: four lines through all six stages, with two coverages, a bundle, a
/// subsidy and an override.
#[test]
fn a_charge_passes_through_all_six_stages() {
    let mut charge = request(vec![
        line("L1", "consult", 1),
        line("L2", "ors", 12),
        line("L3", "anc-visit", 1),
        line("L4", "hb-test", 1),
    ]);
    charge.coverages = coverages();
    charge.overrides = vec![Override {
        line: "L2".into(),
        patient_share: Decimal::ZERO,
        by: "cashier-lead".into(),
        reason: "exempt: under five".into(),
    }];
    let priced = price(&charge, &books(), &rules(), &stub()).expect("priced");
    let by_id = |id: &str| {
        priced
            .lines
            .iter()
            .find(|l| l.id == id)
            .expect("a line")
            .clone()
    };

    // Consult: the facility's book beats the tenant's; +20% after hours; the primary pays after
    // its deductible and 10% copay; the secondary pays up to its limit; a subsidy of 500.
    let consult = by_id("L1");
    assert_eq!(
        amounts(&consult),
        [
            (Kind::Base, d(12_000)),
            (Kind::Surcharge, d(2_400)),
            (Kind::Subsidy, d(-500)),
        ]
    );
    assert_eq!(consult.components[0].from, Some(applied("kam", 2)));
    assert_eq!(consult.gross, d(14_400));
    assert_eq!(
        consult.payer_shares,
        [
            ("Coverage/primary".to_owned(), d(11_160)),
            ("Coverage/secondary".to_owned(), d(1_000))
        ]
    );
    assert_eq!(consult.patient_share, d(1_740));
    assert!(consult.estimate, "the secondary's accumulators are cached");

    // ORS: 12 at the tier price of 400; +20%; 300 off; the secondary excludes it; overridden to
    // nothing for the patient, with who and why.
    let ors = by_id("L2");
    assert_eq!(
        amounts(&ors),
        [
            (Kind::Base, d(4_800)),
            (Kind::Surcharge, d(960)),
            (Kind::Discount, d(-300)),
            (Kind::Override, d(-546)),
        ]
    );
    assert_eq!(
        ors.payer_shares,
        [("Coverage/primary".to_owned(), d(4_914))]
    );
    assert_eq!(ors.patient_share, Decimal::ZERO);
    assert_eq!(
        ors.components[3].note.as_deref(),
        Some("cashier-lead: exempt: under five")
    );

    // The ANC visit and Hb test are priced in the bundle line, which the primary pays up to what
    // is left of its limit.
    for member in ["L3", "L4"] {
        let bundled = by_id(member);
        assert_eq!(bundled.gross, Decimal::ZERO);
        assert_eq!(bundled.bundled_in.as_deref(), Some("bundle:anc-package:L3"));
        assert_eq!(bundled.patient_share, Decimal::ZERO);
    }
    let bundle = by_id("bundle:anc-package:L3");
    assert_eq!(bundle.item, "anc-package");
    assert_eq!(amounts(&bundle), [(Kind::Base, d(15_000))]);
    assert_eq!(
        bundle.payer_shares,
        [
            ("Coverage/primary".to_owned(), d(3_926)),
            ("Coverage/secondary".to_owned(), Decimal::ZERO)
        ]
    );
    assert_eq!(bundle.patient_share, d(11_074));

    assert_eq!(priced.books, [applied("kam", 2), applied("tenant", 1)]);
    assert_eq!(
        priced.rules,
        [
            applied("after-hours", 1),
            applied("anc-package", 1),
            applied("consult-subsidy", 1),
            applied("ors-discount", 1)
        ],
        "the staff discount did not apply, so it is not recorded"
    );
    assert_eq!(
        check(&priced, charge.service_time, &books(), &rules()),
        Vec::<Mismatch>::new()
    );
}

#[test]
fn the_base_price_needs_one_most_specific_approved_book_in_effect() {
    let no_rules: Vec<PriceRule> = vec![];
    let priced = price(
        &request(vec![line("L1", "consult", 1), line("L2", "ors", 9)]),
        &books(),
        &no_rules,
        &stub(),
    )
    .expect("priced");
    assert_eq!(
        priced.lines[0].gross,
        d(12_000),
        "not the expired or draft book"
    );
    assert_eq!(priced.lines[1].gross, d(4_500), "below the tier");

    assert_eq!(
        price(
            &request(vec![line("L1", "xray", 1)]),
            &books(),
            &no_rules,
            &stub()
        ),
        Err(PricingError::NoPrice("xray".into()))
    );
    let mut other_currency = request(vec![line("L1", "consult", 1)]);
    other_currency.currency = "USD".into();
    assert!(matches!(
        price(&other_currency, &books(), &no_rules, &stub()),
        Err(PricingError::NoPrice(_))
    ));
    let mut twice = books();
    twice.push(book("kam-annex", 1, kam(), vec![entry("consult", 13_000)]));
    assert_eq!(
        price(
            &request(vec![line("L1", "consult", 1)]),
            &twice,
            &no_rules,
            &stub()
        ),
        Err(PricingError::Ambiguous("consult".into()))
    );
}

#[test]
fn a_waiver_reduces_only_the_patient_share() {
    let mut waiver = rule(
        "refugee-waiver",
        Stage::Subsidy,
        1,
        Adjustment::Percent { percent: d(100) },
    );
    waiver.applicability = Some("%refugee".into());
    let mut charge = request(vec![line("L1", "consult", 1)]);
    charge.coverages = vec![coverages().remove(0)];
    let stub = Stub(BTreeMap::from([("%refugee", true)]));
    let priced = price(&charge, &books(), &[waiver], &stub).expect("priced");
    let consult = &priced.lines[0];
    assert_eq!(
        consult.payer_shares,
        [("Coverage/primary".to_owned(), d(9_000))],
        "the payer's share is untouched"
    );
    assert_eq!(consult.patient_share, Decimal::ZERO);
    assert_eq!(amounts(consult)[1], (Kind::Subsidy, d(-3_000)));
}

#[test]
fn an_override_needs_a_reason_and_an_author() {
    let mut charge = request(vec![line("L1", "consult", 1)]);
    charge.overrides = vec![Override {
        line: "L1".into(),
        patient_share: d(5_000),
        by: "cashier-lead".into(),
        reason: " ".into(),
    }];
    assert_eq!(
        price(&charge, &books(), &[], &stub()),
        Err(PricingError::BadOverride("L1".into()))
    );
}

#[test]
fn an_applicability_that_cannot_be_evaluated_refuses_the_price() {
    let mut odd = rule(
        "odd",
        Stage::Surcharge,
        1,
        Adjustment::Amount { amount: d(1) },
    );
    odd.applicability = Some("%unknown".into());
    assert!(matches!(
        price(
            &request(vec![line("L1", "consult", 1)]),
            &books(),
            &[odd],
            &stub()
        ),
        Err(PricingError::Applicability { .. })
    ));
}

#[test]
fn rules_of_one_stage_and_priority_that_could_both_apply_conflict() {
    let mut a = rule(
        "a",
        Stage::Discount,
        5,
        Adjustment::Percent { percent: d(5) },
    );
    a.items = vec!["consult".into()];
    let mut b = rule(
        "b",
        Stage::Discount,
        5,
        Adjustment::Amount { amount: d(100) },
    );
    b.items = vec!["ors".into()];
    assert_eq!(validate(&[a.clone(), b.clone()]), vec![], "different items");
    b.items = vec![];
    assert_eq!(
        validate(&[a.clone(), b.clone()]),
        [RuleProblem::Conflict("a".into(), "b".into())]
    );
    b.effective = Effective {
        from: at("2025-01-01T00:00:00Z"),
        to: Some(at("2026-01-01T00:00:00Z")),
    };
    assert_eq!(validate(&[a, b]), vec![], "periods that do not overlap");

    let bad = rule(
        "too-much",
        Stage::Discount,
        1,
        Adjustment::Percent { percent: d(120) },
    );
    let lone = rule(
        "lone-bundle",
        Stage::Surcharge,
        1,
        Adjustment::Bundle {
            items: vec!["consult".into()],
            price: d(1),
            item: "x".into(),
        },
    );
    assert_eq!(
        validate(&[bad, lone]),
        [
            RuleProblem::BadAdjustment("too-much".into()),
            RuleProblem::BadBundle("lone-bundle".into())
        ]
    );
}

#[test]
fn the_authority_finds_versions_not_in_effect_at_the_service_time() {
    let priced = price(
        &request(vec![line("L1", "consult", 1)]),
        &books(),
        &[],
        &stub(),
    )
    .expect("priced");
    let mut retired = books();
    retired[1].effective.to = Some(at("2026-06-01T00:00:00Z"));
    assert_eq!(
        check(&priced, at("2026-11-04T20:00:00Z"), &retired, &[]),
        [Mismatch::NotInEffect(applied("kam", 2))]
    );
    assert_eq!(
        check(&priced, at("2026-11-04T20:00:00Z"), &books()[2..], &[]),
        [Mismatch::Unknown(applied("kam", 2))]
    );
}

#[test]
fn books_and_rules_round_trip_as_json() {
    let json = serde_json::to_string(&(books(), rules())).expect("json");
    let back: (Vec<PriceBook>, Vec<PriceRule>) = serde_json::from_str(&json).expect("back");
    assert_eq!(back, (books(), rules()));
}

fn draft(mut book: PriceBook) -> PriceBook {
    book.approval = Approval::Draft;
    book
}

#[test]
fn a_book_is_approved_once_by_someone_else_and_never_overlaps_itself() {
    let now = at("2026-11-04T09:00:00Z");
    let mut next = draft(book("kam", 4, kam(), vec![entry("consult", 13_000)]));
    assert_eq!(
        approve_book(&next, &books(), "finance-author", now),
        Err(LifecycleError::OwnApproval)
    );
    assert_eq!(
        approve_book(&next, &books(), "finance-1", now),
        Err(LifecycleError::Overlaps(applied("kam", 2))),
        "version 2 is in effect from 2026 with no end"
    );
    next.effective = Effective {
        from: at("2027-01-01T00:00:00Z"),
        to: None,
    };
    let mut all = books();
    all[1] = retire_book(&all[1], at("2027-01-01T00:00:00Z"), now).expect("retire v2");
    let approved_v4 = approve_book(&next, &all, "finance-1", now).expect("approved");
    assert!(matches!(approved_v4.approval, Approval::Approved { ref by, .. } if by == "finance-1"));
    assert_eq!(
        approve_book(&approved_v4, &all, "finance-2", now),
        Err(LifecycleError::NotDraft)
    );

    let empty = draft(book("empty", 1, kam(), vec![]));
    assert!(matches!(
        approve_book(&empty, &[], "finance-1", now),
        Err(LifecycleError::BadPrices(_))
    ));
    let twice = draft(book(
        "twice",
        1,
        kam(),
        vec![entry("consult", 1), entry("consult", 2)],
    ));
    assert!(matches!(
        approve_book(&twice, &[], "finance-1", now),
        Err(LifecycleError::BadPrices(_))
    ));
}

#[test]
fn retirement_never_rewrites_the_past() {
    let now = at("2026-11-04T09:00:00Z");
    let v2 = &books()[1];
    assert_eq!(
        retire_book(v2, at("2026-06-01T00:00:00Z"), now),
        Err(LifecycleError::BadRetirement),
        "charges already priced from it keep their price"
    );
    let retired = retire_book(v2, at("2026-12-01T00:00:00Z"), now).expect("retired");
    assert_eq!(retired.effective.to, Some(at("2026-12-01T00:00:00Z")));
    assert_eq!(
        retire_book(&retired, at("2027-01-01T00:00:00Z"), now),
        Err(LifecycleError::BadRetirement),
        "an end is never pushed later"
    );
    assert_eq!(
        retire_book(&draft(v2.clone()), at("2026-12-01T00:00:00Z"), now),
        Err(LifecycleError::NotApproved)
    );
}

#[test]
fn a_rule_conflicting_with_an_approved_rule_is_refused() {
    let now = at("2026-11-04T09:00:00Z");
    let mut clash = rule(
        "night-surcharge",
        Stage::Surcharge,
        1,
        Adjustment::Percent { percent: d(10) },
    );
    clash.approval = Approval::Draft;
    assert_eq!(
        approve_rule(&clash, &rules(), "finance-1", now),
        Err(LifecycleError::Rule(vec![RuleProblem::Conflict(
            "after-hours".into(),
            "night-surcharge".into()
        )]))
    );
    clash.priority = 2;
    let approved_rule = approve_rule(&clash, &rules(), "finance-1", now).expect("approved");
    assert!(matches!(approved_rule.approval, Approval::Approved { .. }));
    assert_eq!(
        approve_rule(&clash, &rules(), "finance-author", now),
        Err(LifecycleError::OwnApproval)
    );
}

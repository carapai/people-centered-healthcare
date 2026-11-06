//! Coverage for pricing and claims (#125): a facility prices from its cached accumulator, so the
//! split is an estimate; at claim time central prices it again from its own accumulators, and the
//! difference is corrected.

use std::collections::BTreeMap;

use health_coverage::{
    Accumulator, BenefitPlan, Copay, Correction, Coverage, CoverageStatus, Snapshot,
};
use health_coverage_app::{Accumulators, ClaimCharge, CoverageError, pricing_terms, settle_claim};
use health_pricing::{
    Applicability, Approval, Effective, Entry, Line, PriceBook, Request, Scope, price,
};
use health_time::Timestamp;
use jiff::civil::date;
use rust_decimal::Decimal;

fn d(n: i64) -> Decimal {
    Decimal::from(n)
}

fn at(text: &str) -> Timestamp {
    text.parse().expect("timestamp")
}

struct NoRules;

impl Applicability for NoRules {
    fn applies(&self, expression: &str, _: &Line) -> Result<bool, String> {
        Err(format!("no rules here: {expression}"))
    }
}

fn books() -> Vec<PriceBook> {
    vec![PriceBook {
        id: "kam".into(),
        version: 1,
        scope: Scope::Facility("KAM013".into()),
        currency: "UGX".into(),
        effective: Effective {
            from: at("2026-01-01T00:00:00Z"),
            to: None,
        },
        approval: Approval::Approved {
            by: "finance-1".into(),
            at: at("2025-12-01T00:00:00Z"),
        },
        author: "finance-author".into(),
        entries: vec![Entry {
            item: "consult".into(),
            unit: d(100_000),
            tiers: vec![],
        }],
    }]
}

fn plan() -> BenefitPlan {
    BenefitPlan {
        id: "plan-gold".into(),
        version: 1,
        copay: Copay::Percent(d(10)),
        deductible: d(50_000),
        limit: Some(d(1_000_000)),
        excluded: vec![],
    }
}

fn coverage(id: &str, order: u32) -> Coverage {
    Coverage {
        id: id.into(),
        beneficiary: "Patient/p1".into(),
        payer: "Organization/aar".into(),
        plan: "plan-gold".into(),
        order,
        status: CoverageStatus::Active,
        start: date(2026, 1, 1),
        end: Some(date(2026, 12, 31)),
    }
}

fn request(charge: &str, service_time: &str) -> Request {
    Request {
        service_time: at(service_time),
        currency: "UGX".into(),
        minor_units: 0,
        scopes: vec![Scope::Facility("KAM013".into())],
        lines: vec![Line {
            id: format!("{charge}-1"),
            item: "consult".into(),
            quantity: Decimal::ONE,
        }],
        coverages: vec![],
        overrides: vec![],
    }
}

#[test]
fn offline_splits_are_estimates_corrected_at_claim_time() {
    let coverages = [coverage("primary", 1)];
    // Central knows another facility already took 30 000 of the deductible this year.
    let mut central = BTreeMap::new();
    let mut used = Accumulator::new("primary", 2026);
    used.consume(d(30_000), d(0));
    central.insert(("primary".to_owned(), 2026), used);
    // This facility's last snapshot predates it: nothing used.
    let snapshot = Snapshot {
        accumulator: Accumulator::new("primary", 2026),
        as_of: at("2026-10-01T00:00:00Z"),
    };
    let cached = BTreeMap::from([("primary".to_owned(), snapshot)]);

    // At the facility: the whole deductible is thought left, and the split is an estimate.
    let mut at_facility = request("c1", "2026-11-06T09:00:00Z");
    at_facility.coverages = pricing_terms(
        &at_facility,
        &coverages,
        &[plan()],
        Accumulators::Cached(&cached),
    )
    .expect("terms");
    assert!(at_facility.coverages[0].estimate);
    let priced = price(&at_facility, &books(), &[], &NoRules).expect("priced");
    let line = &priced.lines[0];
    assert!(line.estimate, "marked an estimate");
    assert_eq!(
        (line.payer_shares[0].1, line.patient_share),
        (d(45_000), d(55_000)),
        "50 000 deductible, then 10% of the 50 000 left"
    );

    // At claim time: central prices it again with 20 000 of the deductible left.
    let charges = [ClaimCharge {
        charge: "c1".into(),
        request: at_facility,
        priced,
    }];
    let settled = settle_claim(
        &charges,
        &coverages,
        &[plan()],
        central,
        &books(),
        &[],
        &NoRules,
    )
    .expect("settled");
    assert_eq!(settled.repriced, ["c1"]);
    assert_eq!(
        settled.corrections,
        [(
            "c1".to_owned(),
            Correction {
                line: "c1-1".into(),
                coverage: "primary".into(),
                payer_delta: d(27_000),
                patient_delta: d(-27_000),
            }
        )],
        "the payer pays 72 000, not 45 000; the patient is owed 27 000 back"
    );
    let after = &settled.accumulators[&("primary".to_owned(), 2026)];
    assert_eq!((after.deductible_used, after.paid), (d(50_000), d(72_000)));
}

#[test]
fn central_s_own_splits_are_final_and_run_the_accumulators_on() {
    let coverages = [coverage("primary", 1)];
    let central = BTreeMap::new();
    let mut first = request("c1", "2026-11-06T09:00:00Z");
    first.coverages = pricing_terms(
        &first,
        &coverages,
        &[plan()],
        Accumulators::Central(&central),
    )
    .expect("terms");
    assert!(!first.coverages[0].estimate, "central's own: final");
    let priced = price(&first, &books(), &[], &NoRules).expect("priced");
    // A second visit later, estimated at a facility that had not heard of the first.
    let mut second = request("c2", "2026-11-07T09:00:00Z");
    second.coverages = pricing_terms(
        &second,
        &coverages,
        &[plan()],
        Accumulators::Cached(&BTreeMap::new()),
    )
    .expect("terms");
    let second_priced = price(&second, &books(), &[], &NoRules).expect("priced");
    let charges = [
        ClaimCharge {
            charge: "c2".into(),
            request: second,
            priced: second_priced,
        },
        ClaimCharge {
            charge: "c1".into(),
            request: first,
            priced,
        },
    ];
    let settled = settle_claim(
        &charges,
        &coverages,
        &[plan()],
        central,
        &books(),
        &[],
        &NoRules,
    )
    .expect("settled");
    assert_eq!(
        settled.repriced,
        ["c2"],
        "the final one stands; it runs first, by service time"
    );
    // c1 took the whole deductible; c2 had assumed it was all left.
    assert_eq!(settled.corrections.len(), 1);
    let (charge, correction) = &settled.corrections[0];
    assert_eq!(
        (
            charge.as_str(),
            correction.payer_delta,
            correction.patient_delta
        ),
        ("c2", d(45_000), d(-45_000))
    );
    let after = &settled.accumulators[&("primary".to_owned(), 2026)];
    assert_eq!(
        (after.deductible_used, after.paid),
        (d(50_000), d(45_000 + 90_000))
    );
}

#[test]
fn only_coverages_in_force_count_and_each_needs_its_plan() {
    let mut lapsed = coverage("old", 1);
    lapsed.end = Some(date(2025, 12, 31));
    let mut other = coverage("other", 2);
    other.plan = "plan-unknown".into();
    let req = request("c1", "2026-11-06T09:00:00Z");
    let terms = pricing_terms(
        &req,
        &[lapsed.clone(), coverage("primary", 1)],
        &[plan()],
        Accumulators::Cached(&BTreeMap::new()),
    )
    .expect("terms");
    assert_eq!(
        terms
            .iter()
            .map(|t| t.coverage.as_str())
            .collect::<Vec<_>>(),
        ["primary"]
    );
    assert_eq!(
        pricing_terms(
            &req,
            &[other],
            &[plan()],
            Accumulators::Cached(&BTreeMap::new())
        ),
        Err(CoverageError::NoPlan("plan-unknown".into()))
    );
}

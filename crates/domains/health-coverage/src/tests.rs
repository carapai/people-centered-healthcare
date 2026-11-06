use std::sync::atomic::{AtomicBool, Ordering};

use jiff::civil::date;
use rust_decimal::Decimal;

use super::*;

fn d(n: i64) -> Decimal {
    Decimal::from(n)
}

fn plan() -> BenefitPlan {
    BenefitPlan {
        id: "plan-gold".into(),
        version: 1,
        copay: Copay::Percent(d(10)),
        deductible: d(50_000),
        limit: Some(d(1_000_000)),
        excluded: vec!["cosmetic".into()],
    }
}

fn coverage(id: &str, order: u32, status: CoverageStatus) -> Coverage {
    Coverage {
        id: id.into(),
        beneficiary: "Patient/p1".into(),
        payer: "Organization/aar".into(),
        plan: "plan-gold".into(),
        order,
        status,
        start: date(2026, 1, 1),
        end: Some(date(2026, 12, 31)),
    }
}

fn at(text: &str) -> Timestamp {
    text.parse().expect("timestamp")
}

#[test]
fn coverages_apply_in_order_within_their_period() {
    let all = [
        coverage("secondary", 2, CoverageStatus::Active),
        coverage("primary", 1, CoverageStatus::Active),
        coverage("lapsed", 1, CoverageStatus::Cancelled),
        coverage("draft", 3, CoverageStatus::Draft),
    ];
    let ids: Vec<&str> = in_force(&all, date(2026, 11, 6))
        .iter()
        .map(|c| c.id.as_str())
        .collect();
    assert_eq!(ids, ["primary", "secondary"]);
    assert!(
        in_force(&all, date(2027, 1, 1)).is_empty(),
        "after its period"
    );
}

#[test]
fn central_terms_are_final_and_a_facility_s_are_estimates() {
    let primary = coverage("primary", 1, CoverageStatus::Active);
    let mut central = Accumulator::new("primary", 2026);
    central.consume(d(30_000), d(200_000));
    let authoritative = terms(&primary, &plan(), Basis::Authoritative(&central));
    assert_eq!(
        (
            authoritative.deductible_remaining,
            authoritative.limit_remaining,
            authoritative.estimate
        ),
        (d(20_000), Some(d(800_000)), false)
    );
    // The facility's snapshot is older: it still thinks only 10 000 of the deductible is used.
    let mut older = Accumulator::new("primary", 2026);
    older.consume(d(10_000), d(50_000));
    let snapshot = Snapshot {
        accumulator: older,
        as_of: at("2026-11-01T00:00:00Z"),
    };
    let cached = terms(&primary, &plan(), Basis::Cached(&snapshot));
    assert_eq!(
        (cached.deductible_remaining, cached.estimate),
        (d(40_000), true)
    );
    let unknown = terms(&primary, &plan(), Basis::Unknown);
    assert_eq!(
        (
            unknown.deductible_remaining,
            unknown.limit_remaining,
            unknown.estimate
        ),
        (d(50_000), Some(d(1_000_000)), true)
    );
    // Remaining never goes below zero.
    let mut spent = Accumulator::new("primary", 2026);
    spent.consume(d(60_000), d(2_000_000));
    assert_eq!(
        spent.remaining(&plan()),
        (Decimal::ZERO, Some(Decimal::ZERO))
    );
}

struct FakePayer {
    reachable: AtomicBool,
}

impl Payer for FakePayer {
    async fn eligible(&self, coverage: &Coverage, _day: Date) -> Result<bool, PayerError> {
        if self.reachable.load(Ordering::SeqCst) {
            Ok(coverage.id != "refused")
        } else {
            Err(PayerError::Unreachable)
        }
    }
}

#[test]
fn eligibility_offline_is_the_last_answer_pending_verification() {
    let payer = FakePayer {
        reachable: AtomicBool::new(true),
    };
    let primary = coverage("primary", 1, CoverageStatus::Active);
    let day = date(2026, 11, 6);
    let asked_at = at("2026-11-06T08:00:00Z");
    let online = futures_block(check(&payer, &primary, day, None, asked_at));
    assert_eq!(
        (online.status, online.last_answer),
        (EligibilityStatus::Eligible, Some(true))
    );
    let refused = futures_block(check(
        &payer,
        &coverage("refused", 1, CoverageStatus::Active),
        day,
        None,
        asked_at,
    ));
    assert_eq!(refused.status, EligibilityStatus::NotEligible);

    payer.reachable.store(false, Ordering::SeqCst);
    let later = at("2026-11-06T14:00:00Z");
    let offline = futures_block(check(&payer, &primary, day, Some(&online), later));
    assert_eq!(
        (offline.status, offline.last_answer, offline.age(later)),
        (
            EligibilityStatus::PendingVerification,
            Some(true),
            Some(SignedDuration::from_hours(6))
        )
    );
    let never = pending(&primary, None);
    assert_eq!((never.last_answer, never.age(later)), (None, None));
}

#[test]
fn claim_time_corrections_are_the_differences_in_each_split() {
    let split = |line: &str, payer: i64, patient: i64| Split {
        line: line.into(),
        coverage: "primary".into(),
        payer_share: d(payer),
        patient_share: d(patient),
    };
    let estimated = [split("l1", 54_000, 46_000), split("l2", 90_000, 10_000)];
    let actual = [split("l1", 72_000, 28_000), split("l2", 90_000, 10_000)];
    assert_eq!(
        corrections(&estimated, &actual),
        [Correction {
            line: "l1".into(),
            coverage: "primary".into(),
            payer_delta: d(18_000),
            patient_delta: d(-18_000),
        }],
        "the patient paid 18 000 too much: an adjustment back"
    );
    assert!(corrections(&actual, &actual).is_empty());
}

/// Runs a future that never waits (the fake payer answers at once).
fn futures_block<F: Future>(future: F) -> F::Output {
    use std::pin::pin;
    use std::task::{Context, Poll, Waker};
    let mut future = pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    match future.as_mut().poll(&mut context) {
        Poll::Ready(output) => output,
        Poll::Pending => panic!("the fake payer never waits"),
    }
}

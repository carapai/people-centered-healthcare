//! Coverage over SurrealDB (memory engine; #125): a settlement keeps central's accumulators and
//! is not taken in twice, concurrent settlements of one coverage both count, a snapshot gives a
//! facility central's use, and eligibility is kept, pending verification when the payer cannot be
//! reached.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use health_coverage::{
    BenefitPlan, Copay, Coverage, CoverageStatus, EligibilityStatus, Payer, PayerError,
};
use health_coverage_app::{Accumulators, Claim, ClaimCharge, Coverages, pricing_terms};
use health_pricing::{
    Applicability, Approval, Effective, Entry, Line, PriceBook, Request, Scope, price,
};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::{SystemClock, Timestamp};
use jiff::civil::{Date, date};
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

async fn coverages(namespace: &str) -> Coverages<SurrealStore> {
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: namespace.into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    Coverages::new(SurrealStore::new(connection, Arc::new(SystemClock)))
}

/// A charge priced at a facility from an empty snapshot: an estimate.
fn estimated(charge: &str, service_time: &str) -> ClaimCharge {
    let mut req = request(charge, service_time);
    req.coverages = pricing_terms(
        &req,
        &[coverage("primary", 1)],
        &[plan()],
        Accumulators::Cached(&BTreeMap::new()),
    )
    .expect("terms");
    let priced = price(&req, &books(), &[], &NoRules).expect("priced");
    ClaimCharge {
        charge: charge.into(),
        request: req,
        priced,
    }
}

fn claim<'a>(
    charges: &'a [ClaimCharge],
    covers: &'a [Coverage],
    plans: &'a [BenefitPlan],
    books: &'a [PriceBook],
) -> Claim<'a> {
    Claim {
        charges,
        coverages: covers,
        plans,
        books,
        rules: &[],
        applicability: &NoRules,
    }
}

#[tokio::test]
async fn a_settlement_is_kept_and_never_taken_in_twice() {
    let store = coverages("coverage-settle").await;
    let (covers, plans, books) = ([coverage("primary", 1)], [plan()], books());
    let charges = [estimated("c1", "2026-11-06T09:00:00Z")];
    let first = store
        .settle(&claim(&charges, &covers, &plans, &books))
        .await
        .expect("settled");
    assert!(
        first.corrections.is_empty(),
        "nothing was used before: the estimate was right"
    );
    let kept = store
        .snapshot("primary", 2026, at("2026-11-06T10:00:00Z"))
        .await
        .expect("snapshot");
    assert_eq!(
        (kept.accumulator.deductible_used, kept.accumulator.paid),
        (d(50_000), d(45_000))
    );
    // Replayed: nothing changes.
    let again = store
        .settle(&claim(&charges, &covers, &plans, &books))
        .await
        .expect("settled");
    assert!(again.repriced.is_empty() && again.corrections.is_empty());
    let still = store
        .snapshot("primary", 2026, at("2026-11-06T10:00:00Z"))
        .await
        .expect("snapshot");
    assert_eq!(still.accumulator, kept.accumulator);

    // A later charge estimated from a stale snapshot is corrected against what central kept.
    let later = [estimated("c2", "2026-11-07T09:00:00Z")];
    let settled = store
        .settle(&claim(&later, &covers, &plans, &books))
        .await
        .expect("settled");
    assert_eq!(settled.corrections.len(), 1);
    assert_eq!(settled.corrections[0].1.patient_delta, d(-45_000));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_settlements_of_one_coverage_both_count() {
    let store = Arc::new(coverages("coverage-race").await);
    let mut runs = tokio::task::JoinSet::new();
    for i in 0..4 {
        let store = Arc::clone(&store);
        runs.spawn(async move {
            let (covers, plans, books) = ([coverage("primary", 1)], [plan()], books());
            let charges = [estimated(
                &format!("c{i}"),
                &format!("2026-11-0{}T09:00:00Z", i + 1),
            )];
            store
                .settle(&claim(&charges, &covers, &plans, &books))
                .await
                .map(|_| ())
        });
    }
    for outcome in runs.join_all().await {
        outcome.expect("settled");
    }
    let kept = store
        .snapshot("primary", 2026, at("2026-11-08T00:00:00Z"))
        .await
        .expect("snapshot");
    assert_eq!(
        kept.accumulator.charges.len(),
        4,
        "every charge taken in once"
    );
    assert_eq!(
        kept.accumulator.deductible_used,
        d(50_000),
        "the deductible is taken once in all"
    );
    assert_eq!(kept.accumulator.paid, d(45_000 + 3 * 90_000));
}

struct FakePayer {
    reachable: AtomicBool,
}

impl Payer for FakePayer {
    async fn eligible(&self, _coverage: &Coverage, _day: Date) -> Result<bool, PayerError> {
        if self.reachable.load(Ordering::SeqCst) {
            Ok(true)
        } else {
            Err(PayerError::Unreachable)
        }
    }
}

#[tokio::test]
async fn eligibility_is_kept_and_stands_pending_verification_offline() {
    let store = coverages("coverage-eligibility").await;
    let payer = FakePayer {
        reachable: AtomicBool::new(true),
    };
    let primary = coverage("primary", 1);
    let online = store
        .check_eligibility(
            &payer,
            &primary,
            date(2026, 11, 6),
            at("2026-11-06T08:00:00Z"),
        )
        .await
        .expect("checked");
    assert_eq!(online.status, EligibilityStatus::Eligible);
    payer.reachable.store(false, Ordering::SeqCst);
    let offline = store
        .check_eligibility(
            &payer,
            &primary,
            date(2026, 11, 6),
            at("2026-11-06T12:00:00Z"),
        )
        .await
        .expect("checked");
    assert_eq!(
        (offline.status, offline.last_answer, offline.answered_at),
        (
            EligibilityStatus::PendingVerification,
            Some(true),
            online.answered_at
        )
    );
}

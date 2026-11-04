//! Charge capture over SurrealDB 3.3.5 (in-memory) (#121): charges priced and posted once,
//! replays that never double-charge (on this node or another), coverage and free care, and
//! reversals on cancellation.

use std::collections::BTreeMap;
use std::sync::Arc;

use health_billing::{
    AccountBasis, ChargeStatus, ChargeTrigger, ClinicalEvent, EventItem, ItemSource,
};
use health_billing_app::{Billing, Settings};
use health_financial_ledger::{ChartTemplate, Currency, PostingStore, Side};
use health_persistence::{Store, Transaction};
use health_pricing::{
    Applicability, Approval, Copay, CoverageTerms, Effective, Entry, Line, PriceBook, PricingStore,
    Scope,
};
use health_surrealdb::{Connection, MIGRATIONS, SurrealStore, Target, migrate};
use health_time::{FixedClock, Timestamp};
use rust_decimal::Decimal;

fn at(text: &str) -> Timestamp {
    text.parse().expect("timestamp")
}

fn d(value: i64) -> Decimal {
    Decimal::from(value)
}

struct Never;

impl Applicability for Never {
    fn applies(&self, _: &str, _: &Line) -> Result<bool, String> {
        Ok(false)
    }
}

fn book(id: &str, scope: Scope, entries: &[(&str, i64)]) -> PriceBook {
    PriceBook {
        id: id.into(),
        version: 1,
        scope,
        currency: "UGX".into(),
        effective: Effective {
            from: at("2026-01-01T00:00:00Z"),
            to: None,
        },
        approval: Approval::Approved {
            by: "finance-1".into(),
            at: at("2025-12-01T00:00:00Z"),
        },
        author: "author-1".into(),
        entries: entries
            .iter()
            .map(|(item, unit)| Entry {
                item: (*item).into(),
                unit: d(*unit),
                tiers: vec![],
            })
            .collect(),
    }
}

async fn node(namespace: &str) -> (Billing<SurrealStore>, SurrealStore) {
    let connection = Connection::open(&health_surrealdb::Settings {
        target: Target::Memory,
        namespace: namespace.into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    let store = SurrealStore::new(
        connection,
        Arc::new(FixedClock::new(at("2026-11-05T09:00:00Z"))),
    );
    let mut tx = store.begin().await.expect("begin");
    tx.put_book(&book(
        "kam",
        Scope::Facility("KAM013".into()),
        &[("consult", 10_000), ("paracetamol", 50)],
    ))
    .await
    .expect("book");
    tx.put_book(&book("reference", Scope::Tenant, &[("tdf-3tc-dtg", 1_000)]))
        .await
        .expect("reference tariff");
    tx.commit().await.expect("commit");
    let settings = Settings {
        facility: "KAM013".into(),
        currency: Currency::from_code("UGX").expect("currency"),
        chart: ChartTemplate {
            funding_sources: vec!["patient".into(), "jubilee".into(), "pepfar".into()],
            service_classes: vec!["opd".into(), "pharmacy".into()],
            tills: vec![],
            inventory: false,
        },
        scopes: vec![Scope::Facility("KAM013".into())],
        reference_scopes: vec![Scope::Tenant],
        service_classes: BTreeMap::from([("consult".into(), "opd".into())]),
        default_class: "pharmacy".into(),
    };
    (Billing::new(store.clone(), settings), store)
}

fn triggers() -> Vec<ChargeTrigger> {
    let trigger = |url: &str, kind: &str, item| ChargeTrigger {
        url: url.into(),
        version: "1.0.0".into(),
        kind: kind.into(),
        service_points: vec![],
        codes: vec![],
        item,
        account: AccountBasis::Encounter,
        free_care: None,
    };
    let mut arv = trigger(
        "https://health.go.ug/fhir/ChargeTrigger/arv-free",
        "medication-dispense.completed",
        ItemSource::Event,
    );
    arv.codes = vec!["tdf-3tc-dtg".into()];
    arv.free_care = Some("pepfar".into());
    let mut dispense = trigger(
        "https://health.go.ug/fhir/ChargeTrigger/dispense",
        "medication-dispense.completed",
        ItemSource::Event,
    );
    dispense.codes = vec!["paracetamol".into()];
    vec![
        trigger(
            "https://health.go.ug/fhir/ChargeTrigger/opd-consult",
            "encounter.started",
            ItemSource::Fixed("consult".into()),
        ),
        dispense,
        arv,
    ]
}

fn started() -> ClinicalEvent {
    ClinicalEvent {
        id: "enc-1-start".into(),
        kind: "encounter.started".into(),
        at: at("2026-11-05T08:00:00Z"),
        patient: "Patient/p-1".into(),
        encounter: Some("Encounter/e-1".into()),
        episode: None,
        service_point: Some("opd".into()),
        items: vec![],
    }
}

fn dispensed() -> ClinicalEvent {
    ClinicalEvent {
        id: "disp-1".into(),
        kind: "medication-dispense.completed".into(),
        items: vec![
            EventItem {
                key: "line-1".into(),
                code: "paracetamol".into(),
                quantity: d(20),
            },
            EventItem {
                key: "line-2".into(),
                code: "tdf-3tc-dtg".into(),
                quantity: d(30),
            },
        ],
        ..started()
    }
}

async fn postings(store: &SurrealStore) -> Vec<health_financial_ledger::Posting> {
    let mut tx = store.begin().await.expect("begin");
    tx.postings().await.expect("postings")
}

fn lines(posting: &health_financial_ledger::Posting) -> Vec<(String, Side, Decimal)> {
    posting
        .lines
        .iter()
        .map(|l| (l.account.clone(), l.side, l.amount.amount()))
        .collect()
}

#[tokio::test]
async fn an_event_is_charged_and_posted_once_however_often_it_is_replayed() {
    let (billing, store) = node("billing-replay").await;
    let kept = billing
        .capture(&started(), &triggers(), &[], &Never)
        .await
        .expect("capture");
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].status, ChargeStatus::Billable);
    let posted = postings(&store).await;
    assert_eq!(posted.len(), 1);
    assert_eq!(
        lines(&posted[0]),
        [
            ("receivable/patient".to_owned(), Side::Debit, d(10_000)),
            ("revenue/opd".to_owned(), Side::Credit, d(10_000)),
        ]
    );
    assert!(
        posted[0].lines[0]
            .dimensions
            .price_versions
            .contains("kam|1")
    );

    for _ in 0..3 {
        let replayed = billing
            .capture(&started(), &triggers(), &[], &Never)
            .await
            .expect("replay");
        assert!(replayed.is_empty(), "a replay keeps nothing");
    }
    assert_eq!(postings(&store).await.len(), 1, "never double-charged");
    assert_eq!(
        billing
            .charges("Encounter/e-1")
            .await
            .expect("charges")
            .len(),
        1
    );
}

#[tokio::test]
async fn the_same_event_captured_on_two_nodes_gives_the_same_charges() {
    let (edge, _) = node("billing-edge").await;
    let (desktop, _) = node("billing-desktop").await;
    let a = edge
        .capture(&dispensed(), &triggers(), &[], &Never)
        .await
        .expect("edge");
    let b = desktop
        .capture(&dispensed(), &triggers(), &[], &Never)
        .await
        .expect("desktop");
    assert_eq!(
        a, b,
        "deterministic ids and prices: sync converges on one charge each"
    );
}

#[tokio::test]
async fn coverage_splits_the_receivable_and_free_care_goes_to_its_funding_source() {
    let (billing, store) = node("billing-split").await;
    let insured = [CoverageTerms {
        coverage: "jubilee".into(),
        excluded: vec![],
        copay: Copay::Percent(d(10)),
        deductible_remaining: Decimal::ZERO,
        limit_remaining: None,
        estimate: false,
    }];
    billing
        .capture(&started(), &triggers(), &insured, &Never)
        .await
        .expect("consult");
    let kept = billing
        .capture(&dispensed(), &triggers(), &insured, &Never)
        .await
        .expect("dispense");
    let arv = kept
        .iter()
        .find(|c| c.item == "tdf-3tc-dtg")
        .expect("the ARV charge");
    let price = arv.price.as_ref().expect("priced");
    assert_eq!(price.gross, d(30_000), "at the reference tariff");
    assert_eq!(price.patient_share, Decimal::ZERO);
    assert_eq!(price.payer_shares, [("pepfar".to_owned(), d(30_000))]);

    let posted = postings(&store).await;
    let consult = posted
        .iter()
        .find(|p| p.description.contains("consult"))
        .expect("consult posting");
    assert_eq!(
        lines(consult),
        [
            ("receivable/patient".to_owned(), Side::Debit, d(1_000)),
            ("receivable/jubilee".to_owned(), Side::Debit, d(9_000)),
            ("revenue/opd".to_owned(), Side::Credit, d(10_000)),
        ]
    );
    let free = posted
        .iter()
        .find(|p| p.description.contains("tdf-3tc-dtg"))
        .expect("ARV posting");
    assert_eq!(
        lines(free),
        [
            ("receivable/pepfar".to_owned(), Side::Debit, d(30_000)),
            ("revenue/pharmacy".to_owned(), Side::Credit, d(30_000)),
        ]
    );
}

#[tokio::test]
async fn cancelling_an_event_reverses_its_charges_once() {
    let (billing, store) = node("billing-cancel").await;
    billing
        .capture(&dispensed(), &triggers(), &[], &Never)
        .await
        .expect("capture");
    let before = postings(&store).await.len();
    let cancelled = billing
        .cancel_event(
            "disp-1",
            "dispense entered in error",
            at("2026-11-05T10:00:00Z"),
        )
        .await
        .expect("cancel");
    assert_eq!(cancelled.len(), 2);
    assert!(
        cancelled
            .iter()
            .all(|c| c.status == ChargeStatus::EnteredInError)
    );
    let after = postings(&store).await;
    assert_eq!(after.len(), before + 2, "one reversal per posted charge");
    let reversal = after.last().expect("a reversal");
    assert!(reversal.reverses.is_some());
    assert!(
        reversal
            .lines
            .iter()
            .any(|l| l.side == Side::Credit && l.account.starts_with("receivable/"))
    );

    let again = billing
        .cancel_event("disp-1", "again", at("2026-11-05T11:00:00Z"))
        .await
        .expect("cancel again");
    assert!(again.is_empty());
    assert_eq!(postings(&store).await.len(), before + 2, "reversed once");
    assert!(
        billing
            .capture(&dispensed(), &triggers(), &[], &Never)
            .await
            .expect("replay after cancellation")
            .is_empty(),
        "a replay after cancellation does not charge again"
    );
}

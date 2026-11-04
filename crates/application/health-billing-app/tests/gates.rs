//! Financial Gates over SurrealDB 3.3.5 (in-memory) (#122; ADR 0033): what is owed, urgent care
//! never blocked, settlements, deferrals and waivers.

use std::collections::BTreeMap;
use std::sync::Arc;

use health_billing::{
    AccountBasis, ChargeTrigger, ClinicalEvent, FinancialGate, ItemSource, Waiver,
};
use health_billing_app::{Billing, BillingError, Settings};
use health_billing_contract::{FinancialGates, GateOutcome, GateQuery, Urgency};
use health_financial_ledger::{ChartTemplate, Currency, PostingStore};
use health_payments::{Method, Payment, PaymentStore, Purpose, Status};
use health_persistence::{Store, Transaction};
use health_pricing::{
    Applicability, Approval, Effective, Entry, Line, PriceBook, PricingStore, Scope,
};
use health_surrealdb::{Connection, MIGRATIONS, SurrealStore, Target, migrate};
use health_time::{FixedClock, Timestamp};
use rust_decimal::Decimal;

const GATE: &str = "https://health.go.ug/fhir/FinancialGate/before-lab";
const ACCOUNT: &str = "Encounter/e-1";

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

async fn billing() -> (Billing<SurrealStore>, SurrealStore) {
    let connection = Connection::open(&health_surrealdb::Settings {
        target: Target::Memory,
        namespace: "billing-gates".into(),
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
    tx.put_book(&PriceBook {
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
        author: "author-1".into(),
        entries: vec![Entry {
            item: "fbc".into(),
            unit: d(20_000),
            tiers: vec![],
        }],
    })
    .await
    .expect("book");
    tx.put_payment(&Payment {
        id: "pay-1".into(),
        method: Method::Cash,
        purpose: Purpose::Settlement,
        amount: d(15_000),
        payer: "Patient/p-1".into(),
        reference: None,
        shift: Some("shift-1".into()),
        status: Status::Posted,
        recorded_at: at("2026-11-05T08:30:00Z"),
        expires_at: None,
        evidence: None,
        reason: None,
        refunded: Decimal::ZERO,
        receipt: Some("RCT-1".into()),
    })
    .await
    .expect("payment");
    tx.commit().await.expect("commit");
    let settings = Settings {
        facility: "KAM013".into(),
        currency: Currency::from_code("UGX").expect("currency"),
        chart: ChartTemplate {
            funding_sources: vec!["patient".into()],
            service_classes: vec!["lab".into()],
            tills: vec![],
            inventory: false,
        },
        scopes: vec![Scope::Facility("KAM013".into())],
        reference_scopes: vec![Scope::Tenant],
        service_classes: BTreeMap::new(),
        default_class: "lab".into(),
    };
    let gates = vec![FinancialGate {
        url: GATE.into(),
        version: "1.0.0".into(),
        active: true,
        allows_block: true,
        requires_preauthorization: false,
    }];
    (
        Billing::new(store.clone(), settings).with_gates(gates),
        store,
    )
}

/// An FBC ordered on the Account: 20,000 owed.
async fn ordered(billing: &Billing<SurrealStore>) {
    let trigger = ChargeTrigger {
        url: "https://health.go.ug/fhir/ChargeTrigger/lab".into(),
        version: "1.0.0".into(),
        kind: "service-request.created".into(),
        service_points: vec![],
        codes: vec![],
        item: ItemSource::Fixed("fbc".into()),
        account: AccountBasis::Encounter,
        free_care: None,
    };
    let event = ClinicalEvent {
        id: "sr-1".into(),
        kind: "service-request.created".into(),
        at: at("2026-11-05T08:00:00Z"),
        patient: "Patient/p-1".into(),
        encounter: Some(ACCOUNT.into()),
        episode: None,
        service_point: None,
        items: vec![],
    };
    billing
        .capture(&event, &[trigger], &[], &Never)
        .await
        .expect("captured");
}

fn query(urgency: Urgency) -> GateQuery {
    GateQuery {
        gate: GATE.into(),
        account: ACCOUNT.into(),
        patient: "Patient/p-1".into(),
        urgency,
    }
}

#[tokio::test]
async fn urgent_triage_always_defers_and_routine_care_waits_for_payment() {
    let (billing, _) = billing().await;
    assert_eq!(
        billing
            .evaluate_gate(&query(Urgency::Routine))
            .await
            .expect("gate")
            .code(),
        "pass",
        "nothing owed yet"
    );
    ordered(&billing).await;
    assert_eq!(billing.owed(ACCOUNT).await.expect("owed"), d(20_000));
    for urgency in [Urgency::Urgent, Urgency::Emergency] {
        assert_eq!(
            billing
                .evaluate_gate(&query(urgency))
                .await
                .expect("gate")
                .code(),
            "defer",
            "{urgency:?} care is never blocked"
        );
    }
    assert_eq!(
        billing.evaluate_gate(&query(Urgency::Routine)).await,
        Ok(GateOutcome::Block("20000 is owed before this step".into()))
    );

    billing
        .settle(
            "pay-1",
            ACCOUNT,
            d(15_000),
            "cashier-1",
            at("2026-11-05T09:00:00Z"),
        )
        .await
        .expect("settle");
    assert_eq!(billing.owed(ACCOUNT).await.expect("owed"), d(5_000));
    assert!(
        matches!(
            billing
                .settle(
                    "pay-1",
                    "Encounter/e-2",
                    d(1),
                    "cashier-1",
                    at("2026-11-05T09:01:00Z")
                )
                .await,
            Err(BillingError::Gate(_))
        ),
        "a payment is never settled beyond its amount"
    );
    assert!(matches!(
        billing
            .settle(
                "pay-x",
                ACCOUNT,
                d(1),
                "cashier-1",
                at("2026-11-05T09:01:00Z")
            )
            .await,
        Err(BillingError::NotPayable(_))
    ));

    billing
        .defer(
            ACCOUNT,
            GATE,
            "social-worker-1",
            "will pay tomorrow",
            at("2026-11-05T09:02:00Z"),
        )
        .await
        .expect("defer");
    assert_eq!(
        billing
            .evaluate_gate(&query(Urgency::Routine))
            .await
            .expect("gate")
            .code(),
        "defer"
    );
}

#[tokio::test]
async fn a_waiver_is_posted_and_clears_what_is_owed() {
    let (billing, store) = billing().await;
    ordered(&billing).await;
    let waiver = |amount: i64| Waiver {
        id: "w-1".into(),
        account: ACCOUNT.into(),
        amount: d(amount),
        by: "social-worker-1".into(),
        reason: "exempt: unable to pay".into(),
        at: at("2026-11-05T09:10:00Z"),
    };
    assert!(matches!(
        billing.waive(waiver(25_000)).await,
        Err(BillingError::Gate(_))
    ));
    billing.waive(waiver(20_000)).await.expect("waive");
    assert_eq!(billing.owed(ACCOUNT).await.expect("owed"), Decimal::ZERO);
    assert_eq!(
        billing
            .evaluate_gate(&query(Urgency::Routine))
            .await
            .expect("gate")
            .code(),
        "pass"
    );
    let mut tx = store.begin().await.expect("begin");
    let posted = tx.postings().await.expect("postings");
    let waived = posted
        .iter()
        .find(|p| p.id == "waiver-w-1")
        .expect("the waiver's posting");
    let accounts: Vec<&str> = waived.lines.iter().map(|l| l.account.as_str()).collect();
    assert_eq!(accounts, ["waiver/patient", "receivable/patient"]);
}

#[tokio::test]
async fn a_gate_not_configured_here_is_unavailable() {
    let (billing, _) = billing().await;
    let mut unknown = query(Urgency::Routine);
    unknown.gate = "urn:unknown".into();
    assert!(billing.evaluate_gate(&unknown).await.is_err());
}

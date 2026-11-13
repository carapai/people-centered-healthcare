//! MTN MoMo collections against a mock MTN (#124; testing.md § Payment reconciliation): a
//! payment is stored before the request goes out; dropped callbacks resolve by polling; a lost
//! answer is polled, never re-sent; duplicate callbacks post once; a wrong amount is never posted.

use std::sync::Arc;

mod common;

use common::{Mock, Next};

use health_financial_ledger::{ChartTemplate, Currency, IN_TRANSIT};
use health_identifiers::NumberFormat;
use health_integration::mobile_money::{Collections, MtnConfig, MtnMomo, Settled};
use health_payments::{Evidence, Method, NewPayment, Purpose, Status};
use health_payments_app::{Accounts, Payments, Settings};
use health_surrealdb::{Connection, MIGRATIONS, Settings as Db, SurrealStore, Target, migrate};
use health_time::{Clock, FixedClock, SignedDuration, Timestamp};
use rust_decimal::Decimal;

const MERCHANT: &str = "cash/merchant-mtn";

struct Fixture {
    collections: Collections<Mock>,
    payments: Payments<SurrealStore>,
    store: SurrealStore,
    mock: Mock,
    clock: Arc<FixedClock>,
}

fn settings() -> Settings {
    Settings {
        facility: "KAM013".parse().expect("facility"),
        node: "C1".parse().expect("node"),
        currency: Currency::from_code("UGX").expect("currency"),
        chart: ChartTemplate {
            funding_sources: vec!["patient".into()],
            tills: vec!["merchant-mtn".into()],
            ..ChartTemplate::default()
        },
        accounts: Accounts {
            receivable: "receivable/patient".into(),
            merchants: vec![(Method::MtnMomo, MERCHANT.into())],
        },
        receipts: NumberFormat {
            padding: 6,
            ..NumberFormat::default()
        },
    }
}

async fn fixture() -> Fixture {
    let connection = Connection::open(&Db {
        target: Target::Memory,
        namespace: "momo".into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    let clock = Arc::new(FixedClock::new(
        Timestamp::from_second(1_800_000_000).expect("now"),
    ));
    let store = SurrealStore::new(connection, Arc::clone(&clock) as Arc<dyn Clock>);
    let mock = Mock::new();
    let mtn = MtnMomo::new(
        mock.clone(),
        MtnConfig {
            subscription_key: "sub-key".into(),
            api_user: "api-user".into(),
            api_key: "api-key".into(),
            environment: "sandbox".into(),
            currency: "UGX".into(),
            callback_url: Some("https://central.example/api/payments/momo/callback".into()),
        },
    );
    Fixture {
        collections: Collections::new(
            store.clone(),
            settings(),
            mtn,
            Arc::clone(&clock) as Arc<dyn Clock>,
            SignedDuration::from_hours(1),
        ),
        payments: Payments::new(store.clone(), settings()),
        store,
        mock,
        clock,
    }
}

fn new(id: &'static str) -> NewPayment<'static> {
    NewPayment {
        id,
        method: Method::MtnMomo,
        purpose: Purpose::Settlement,
        amount: Decimal::from(15_000),
        payer: "Account/acc-1",
    }
}

impl Fixture {
    async fn balances(&self) -> (Decimal, Decimal) {
        let balances = self
            .payments
            .balances(&[IN_TRANSIT, MERCHANT])
            .await
            .expect("balances");
        (balances[0].1, balances[1].1)
    }
}

#[tokio::test]
async fn a_callback_posts_once_however_often_it_comes() {
    let f = fixture().await;
    let payment = f
        .collections
        .initiate(&new("pay-1"), "256772000001", "OPD visit", None)
        .await
        .expect("initiate");
    assert_eq!(
        payment.status,
        Status::Pending,
        "stored before the request went out"
    );
    let reference = payment.reference.clone().expect("reference");
    assert_eq!(f.balances().await, (Decimal::from(15_000), Decimal::ZERO));

    f.mock.answer(&reference, "SUCCESSFUL", "15000");
    assert_eq!(
        f.collections.callback(&reference).await.expect("callback"),
        Settled::Confirmed
    );
    assert_eq!(
        f.collections.callback(&reference).await.expect("again"),
        Settled::AlreadySettled
    );
    let posted = f.payments.payment("pay-1").await.expect("payment");
    assert_eq!(posted.status, Status::Posted);
    assert_eq!(posted.evidence, Some(Evidence::Callback));
    assert_eq!(
        f.balances().await,
        (Decimal::ZERO, Decimal::from(15_000)),
        "in the merchant account once"
    );
    // A poll after the callback finds nothing pending.
    assert!(f.collections.poll().await.expect("poll").is_empty());
    assert_eq!(
        f.mock.count("POST /collection/token/"),
        1,
        "the token is reused"
    );
}

#[tokio::test]
async fn a_dropped_callback_resolves_by_polling() {
    let f = fixture().await;
    let payment = f
        .collections
        .initiate(&new("pay-1"), "256772000001", "OPD visit", None)
        .await
        .expect("initiate");
    let reference = payment.reference.expect("reference");
    assert_eq!(
        f.collections.poll().await.expect("poll"),
        vec![("pay-1".to_owned(), Settled::Waiting)]
    );
    f.mock.answer(&reference, "SUCCESSFUL", "15000");
    // The callback never comes.
    assert_eq!(
        f.collections.poll().await.expect("poll"),
        vec![("pay-1".to_owned(), Settled::Confirmed)]
    );
    assert_eq!(
        f.payments.payment("pay-1").await.expect("payment").evidence,
        Some(Evidence::Poll)
    );
    assert_eq!(f.balances().await, (Decimal::ZERO, Decimal::from(15_000)));
}

#[tokio::test]
async fn a_lost_answer_is_polled_and_never_sent_again() {
    let f = fixture().await;
    f.mock.next(Next::LoseAnswer);
    let payment = f
        .collections
        .initiate(&new("pay-1"), "256772000001", "OPD visit", None)
        .await
        .expect("initiate");
    assert_eq!(payment.status, Status::Pending);
    let reference = payment.reference.expect("reference");
    f.mock.answer(&reference, "SUCCESSFUL", "15000");
    assert_eq!(
        f.collections.poll().await.expect("poll"),
        vec![("pay-1".to_owned(), Settled::Confirmed)]
    );
    assert_eq!(
        f.mock.count("POST /collection/v1_0/requesttopay"),
        1,
        "the request went out once"
    );
}

#[tokio::test]
async fn refusals_and_failures_void_the_payment() {
    let f = fixture().await;
    f.mock.next(Next::Refuse);
    let refused = f
        .collections
        .initiate(&new("pay-1"), "256700000000", "OPD visit", None)
        .await
        .expect("initiate");
    assert_eq!(refused.status, Status::Voided);
    assert!(
        refused
            .reason
            .as_deref()
            .is_some_and(|r| r.contains("PAYER_NOT_FOUND"))
    );

    f.mock.next(Next::Accept);
    let declined = f
        .collections
        .initiate(&new("pay-2"), "256772000001", "OPD visit", None)
        .await
        .expect("initiate");
    f.mock.answer(
        declined.reference.as_deref().expect("reference"),
        "FAILED",
        "15000",
    );
    let settled = f
        .collections
        .callback(declined.reference.as_deref().expect("reference"))
        .await
        .expect("callback");
    assert!(matches!(settled, Settled::Voided(why) if why.contains("APPROVAL_REJECTED")));
    assert_eq!(
        f.balances().await,
        (Decimal::ZERO, Decimal::ZERO),
        "nothing in transit"
    );
}

#[tokio::test]
async fn a_wrong_amount_is_never_posted() {
    let f = fixture().await;
    let payment = f
        .collections
        .initiate(&new("pay-1"), "256772000001", "OPD visit", None)
        .await
        .expect("initiate");
    let reference = payment.reference.expect("reference");
    f.mock.answer(&reference, "SUCCESSFUL", "1500");
    assert_eq!(
        f.collections.callback(&reference).await.expect("callback"),
        Settled::AmountMismatch {
            recorded: Decimal::from(15_000),
            provider: Decimal::from(1_500),
        }
    );
    assert_eq!(
        f.payments.payment("pay-1").await.expect("payment").status,
        Status::Pending
    );
    let queued = f.store.open_payment_exceptions().await.expect("exceptions");
    assert_eq!(queued.len(), 1, "for a person");
    assert_eq!(queued[0].kind, "amount");
    assert_eq!(queued[0].provider.as_deref(), Some("1500"));
}

#[tokio::test]
async fn a_request_mtn_never_received_is_voided_after_the_grace() {
    let f = fixture().await;
    // Taken by the store, then the request is lost before MTN: MTN has no such reference.
    let payment = f
        .collections
        .initiate(&new("pay-1"), "256772000001", "OPD visit", None)
        .await
        .expect("initiate");
    f.mock
        .held
        .lock()
        .expect("held")
        .remove(payment.reference.as_deref().expect("reference"));
    assert_eq!(
        f.collections.poll().await.expect("poll"),
        vec![("pay-1".to_owned(), Settled::Waiting)],
        "it may still be on its way"
    );
    f.clock.advance(SignedDuration::from_mins(6));
    let settled = f.collections.poll().await.expect("poll");
    assert!(
        matches!(&settled[..], [(_, Settled::Voided(why))] if why.contains("never reached MTN")),
        "{settled:?}"
    );
}

#[tokio::test]
async fn an_unknown_reference_is_not_settled() {
    let f = fixture().await;
    assert_eq!(
        f.collections
            .callback("8f0e3c6a-1111-4c4c-9999-000000000000")
            .await
            .expect("callback"),
        Settled::AlreadySettled
    );
}

#[tokio::test]
async fn a_callback_racing_a_poll_posts_once() {
    let f = fixture().await;
    let payment = f
        .collections
        .initiate(&new("pay-1"), "256772000001", "OPD visit", None)
        .await
        .expect("initiate");
    let reference = payment.reference.expect("reference");
    f.mock.answer(&reference, "SUCCESSFUL", "15000");
    let (callback, poll) = tokio::join!(f.collections.callback(&reference), f.collections.poll());
    // Whichever loses the race sees it settled (or retries later on a store conflict).
    let confirmed = usize::from(matches!(callback, Ok(Settled::Confirmed)))
        + poll
            .iter()
            .flatten()
            .filter(|(_, s)| *s == Settled::Confirmed)
            .count();
    assert!(confirmed <= 1, "{callback:?} {poll:?}");
    // Whatever the race left, a later poll settles it, and the money lands once.
    f.collections.poll().await.expect("poll");
    assert_eq!(f.balances().await, (Decimal::ZERO, Decimal::from(15_000)));
}

#[tokio::test]
async fn a_forged_callback_settles_nothing() {
    let f = fixture().await;
    let payment = f
        .collections
        .initiate(&new("pay-1"), "256772000001", "OPD visit", None)
        .await
        .expect("initiate");
    // Someone calls the public callback route; MTN still says the payer has not paid.
    assert_eq!(
        f.collections
            .callback(payment.reference.as_deref().expect("reference"))
            .await
            .expect("callback"),
        Settled::Waiting
    );
    assert_eq!(f.balances().await, (Decimal::from(15_000), Decimal::ZERO));
}

#[tokio::test]
async fn a_statement_confirms_what_it_shows_and_queues_the_rest() {
    use health_payments::StatementLine;

    let f = fixture().await;
    let shown = f
        .collections
        .initiate(&new("pay-1"), "256772000001", "OPD visit", None)
        .await
        .expect("initiate");
    let differs = f
        .collections
        .initiate(&new("pay-2"), "256772000002", "OPD visit", None)
        .await
        .expect("initiate");
    let line = |payment: &health_payments::Payment, amount: i64| StatementLine {
        reference: payment.reference.clone().expect("reference"),
        amount: Decimal::from(amount),
    };
    let lines = [
        line(&shown, 15_000),
        line(&differs, 14_000),
        StatementLine {
            reference: "nobody-has-this".into(),
            amount: Decimal::from(500),
        },
    ];
    let result = f
        .collections
        .reconcile_statement(&lines)
        .await
        .expect("reconcile");
    assert_eq!(result.confirm, vec!["pay-1".to_owned()]);
    assert_eq!(
        f.payments.payment("pay-1").await.expect("payment").evidence,
        Some(Evidence::Statement)
    );
    // Reconciled again (the same statement twice): the queue holds each exception once.
    f.collections
        .reconcile_statement(&lines)
        .await
        .expect("again");
    let queued = f.store.open_payment_exceptions().await.expect("exceptions");
    let kinds: Vec<(&str, &str)> = queued
        .iter()
        .map(|e| (e.kind.as_str(), e.reference.as_str()))
        .collect();
    assert_eq!(queued.len(), 2, "{kinds:?}");
    assert!(kinds.contains(&(
        "mismatched",
        differs.reference.as_deref().expect("reference")
    )));
    assert!(kinds.contains(&("unknown", "nobody-has-this")));
    // A person resolves one; it leaves the queue.
    assert!(
        f.store
            .resolve_payment_exception(&queued[0].id, "supervisor", "checked with MTN")
            .await
            .expect("resolve")
    );
    assert_eq!(
        f.store.open_payment_exceptions().await.expect("open").len(),
        1
    );
}

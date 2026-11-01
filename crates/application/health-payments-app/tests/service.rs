//! Payments and shifts stored in SurrealDB (memory engine): every step with its ledger posting
//! in one transaction, receipts numbered, provider references used once, and a pending payment
//! never confirmed before provider evidence.

use std::sync::Arc;

use health_financial_ledger::{ChartTemplate, Currency, IN_TRANSIT, TILL_VARIANCE};
use health_identifiers::NumberFormat;
use health_payments::{
    Evidence, Method, NewPayment, Purpose, ReceiptStatus, ShiftStatus, StatementLine, Status,
};
use health_payments_app::{Accounts, Payments, PaymentsError, Settings};
use health_surrealdb::{Connection, MIGRATIONS, Settings as Db, SurrealStore, Target, migrate};
use health_time::{SystemClock, Timestamp};
use rust_decimal::Decimal;

async fn payments() -> Payments<SurrealStore> {
    let connection = Connection::open(&Db {
        target: Target::Memory,
        namespace: "payments".into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    Payments::new(
        SurrealStore::new(connection, Arc::new(SystemClock)),
        Settings {
            facility: "KAM013".parse().expect("facility"),
            node: "C1".parse().expect("node"),
            currency: Currency::from_code("UGX").expect("currency"),
            chart: ChartTemplate {
                funding_sources: vec!["patient".into()],
                tills: vec!["till-1".into(), "merchant-mtn".into()],
                ..ChartTemplate::default()
            },
            accounts: Accounts {
                receivable: "receivable/patient".into(),
                merchants: vec![
                    (Method::MtnMomo, "cash/merchant-mtn".into()),
                    (Method::CustomerReported, "cash/merchant-mtn".into()),
                ],
            },
            receipts: NumberFormat {
                padding: 6,
                ..NumberFormat::default()
            },
        },
    )
}

fn at(seconds: i64) -> Timestamp {
    Timestamp::from_second(1_800_000_000 + seconds).expect("time")
}

fn ugx(n: i64) -> Decimal {
    Decimal::from(n)
}

fn new(id: &'static str, method: Method, amount: i64) -> NewPayment<'static> {
    NewPayment {
        id,
        method,
        purpose: Purpose::Settlement,
        amount: ugx(amount),
        payer: "Account/acc-1",
    }
}

#[tokio::test]
async fn a_shift_with_cash_and_a_confirmed_mobile_money_payment() {
    let payments = payments().await;
    payments
        .open_shift("shift-1", "till-1", "cashier-1", ugx(0), at(0))
        .await
        .expect("open");
    assert_eq!(
        payments
            .open_shift("shift-2", "till-1", "cashier-2", ugx(0), at(1))
            .await,
        Err(PaymentsError::ShiftOpen("till-1".into()))
    );
    let cash = payments
        .take_cash(&new("cash-1", Method::Cash, 30_000), "shift-1", at(2))
        .await
        .expect("cash");
    assert_eq!(cash.receipt(), ReceiptStatus::Confirmed);
    let first = cash.receipt.clone().expect("receipt number");

    let momo = payments
        .record_pending(
            &new("momo-1", Method::MtnMomo, 20_000),
            "x-ref-1",
            Some("shift-1"),
            at(3),
            at(603),
        )
        .await
        .expect("pending");
    assert_ne!(momo.receipt, Some(first), "each receipt its own number");
    assert_eq!(momo.receipt(), ReceiptStatus::Provisional);
    assert!(
        !payments
            .payment("momo-1")
            .await
            .expect("stored")
            .is_confirmed()
    );
    assert_eq!(
        payments
            .record_pending(
                &new("momo-2", Method::MtnMomo, 20_000),
                "x-ref-1",
                None,
                at(4),
                at(604)
            )
            .await,
        Err(PaymentsError::DuplicateReference("x-ref-1".into()))
    );
    let balances = payments
        .balances(&[IN_TRANSIT, "cash/merchant-mtn", "cash/till-1"])
        .await
        .expect("balances");
    assert_eq!(
        balances.iter().map(|b| b.1).collect::<Vec<_>>(),
        [ugx(20_000), ugx(0), ugx(30_000)]
    );

    let confirmed = payments
        .confirm("momo-1", Evidence::Callback, at(5))
        .await
        .expect("confirmed");
    assert_eq!(confirmed.receipt(), ReceiptStatus::Confirmed);
    let balances = payments
        .balances(&[IN_TRANSIT, "cash/merchant-mtn"])
        .await
        .expect("balances");
    assert_eq!(
        balances.iter().map(|b| b.1).collect::<Vec<_>>(),
        [ugx(0), ugx(20_000)]
    );

    // 30,000 expected; 29,000 counted; a supervisor approves the shortage, and the till reopens.
    let closed = payments
        .close_shift("shift-1", ugx(29_000))
        .await
        .expect("closed");
    assert_eq!(closed.status, ShiftStatus::AwaitingApproval);
    assert!(matches!(
        payments.approve_shift("shift-1", "cashier-1", at(6)).await,
        Err(PaymentsError::Shift(_))
    ));
    payments
        .approve_shift("shift-1", "supervisor-1", at(6))
        .await
        .expect("approved");
    let balances = payments
        .balances(&[TILL_VARIANCE, "cash/till-1"])
        .await
        .expect("balances");
    assert_eq!(
        balances.iter().map(|b| b.1).collect::<Vec<_>>(),
        [ugx(1_000), ugx(29_000)]
    );
    payments
        .open_shift("shift-2", "till-1", "cashier-2", ugx(0), at(7))
        .await
        .expect("reopened");
    let (shift, taken) = payments.shift("shift-1").await.expect("shift");
    assert_eq!(shift.status, ShiftStatus::Settled);
    assert_eq!(
        taken.len(),
        2,
        "the cash and the MTN payment taken in the shift"
    );
}

#[tokio::test]
async fn pending_payments_void_expire_or_are_confirmed_by_the_statement() {
    let payments = payments().await;
    payments
        .record_pending(
            &new("p-void", Method::MtnMomo, 1_000),
            "r-void",
            None,
            at(0),
            at(600),
        )
        .await
        .expect("pending");
    payments
        .void("p-void", "provider failure", at(1))
        .await
        .expect("voided");
    payments
        .record_pending(
            &new("p-late", Method::MtnMomo, 2_000),
            "r-late",
            None,
            at(0),
            at(60),
        )
        .await
        .expect("pending");
    assert_eq!(
        payments.expire_due(at(30)).await.expect("expiry"),
        Vec::<String>::new()
    );
    assert_eq!(
        payments.expire_due(at(60)).await.expect("expiry"),
        ["p-late"]
    );
    assert_eq!(
        payments.payment("p-late").await.expect("p").status,
        Status::Expired
    );

    payments
        .record_pending(
            &new("p-sms", Method::CustomerReported, 5_000),
            "SMS-77",
            None,
            at(0),
            at(86_400),
        )
        .await
        .expect("pending");
    payments
        .record_pending(
            &new("p-diff", Method::MtnMomo, 4_000),
            "r-diff",
            None,
            at(0),
            at(86_400),
        )
        .await
        .expect("pending");
    let result = payments
        .reconcile_statement(
            &[
                StatementLine {
                    reference: "SMS-77".into(),
                    amount: ugx(5_000),
                },
                StatementLine {
                    reference: "r-diff".into(),
                    amount: ugx(3_000),
                },
                StatementLine {
                    reference: "r-nobody".into(),
                    amount: ugx(9),
                },
            ],
            at(100),
        )
        .await
        .expect("reconciled");
    assert_eq!(result.confirm, ["p-sms"]);
    assert_eq!(result.mismatched.len(), 1);
    assert_eq!(result.unknown, ["r-nobody"]);
    assert!(payments.payment("p-sms").await.expect("p").is_confirmed());
    assert!(
        !payments.payment("p-diff").await.expect("p").is_confirmed(),
        "a mismatch confirms nothing"
    );
    let balances = payments
        .balances(&[IN_TRANSIT, "cash/merchant-mtn"])
        .await
        .expect("balances");
    assert_eq!(
        balances.iter().map(|b| b.1).collect::<Vec<_>>(),
        [ugx(4_000), ugx(5_000)]
    );
}

#[tokio::test]
async fn cash_refunds_need_the_permission_and_an_open_shift() {
    let payments = payments().await;
    payments
        .open_shift("shift-1", "till-1", "cashier-1", ugx(10_000), at(0))
        .await
        .expect("open");
    payments
        .take_cash(&new("cash-1", Method::Cash, 8_000), "shift-1", at(1))
        .await
        .expect("cash");
    assert!(matches!(
        payments.refund("cash-1", ugx(3_000), false, at(2)).await,
        Err(PaymentsError::Payment(_))
    ));
    let refunded = payments
        .refund("cash-1", ugx(3_000), true, at(2))
        .await
        .expect("refunded");
    assert_eq!(refunded.refunded, ugx(3_000));
    let (shift, _) = payments.shift("shift-1").await.expect("shift");
    assert_eq!(shift.expected(), ugx(15_000));
    let balances = payments.balances(&["cash/till-1"]).await.expect("balances");
    assert_eq!(balances[0].1, ugx(5_000));
    payments
        .close_shift("shift-1", ugx(15_000))
        .await
        .expect("closed");
    assert!(matches!(
        payments.refund("cash-1", ugx(1_000), true, at(3)).await,
        Err(PaymentsError::NotFound(_))
    ));
}

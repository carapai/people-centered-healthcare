//! Payments and tills (ADR 0032; §68): provisional payments never appear as confirmed.

use health_payments::{
    Evidence, Method, NewPayment, Payment, PaymentRefused, Purpose, ReceiptStatus, Shift,
    ShiftRefused, ShiftStatus, Status,
};
use health_time::Timestamp;
use rust_decimal::Decimal;

fn at(seconds: i64) -> Timestamp {
    Timestamp::from_second(1_800_000_000 + seconds).expect("time")
}

fn ugx(n: i64) -> Decimal {
    Decimal::from(n)
}

fn new(method: Method, amount: i64) -> NewPayment<'static> {
    NewPayment {
        id: "pay-1",
        method,
        purpose: Purpose::Settlement,
        amount: ugx(amount),
        payer: "Account/acc-1",
    }
}

fn shift() -> Shift {
    Shift::open("shift-1", "till-1", "cashier-1", ugx(50_000), at(0)).expect("shift")
}

#[test]
fn provisional_payments_never_appear_as_confirmed() {
    for method in [
        Method::MtnMomo,
        Method::AirtelMoney,
        Method::Card,
        Method::BankTransfer,
        Method::CustomerReported,
    ] {
        let mut payment =
            Payment::pending(&new(method, 20_000), "ref-1", at(0), at(600)).expect("pending");
        assert!(!payment.is_confirmed(), "{method:?}");
        assert_eq!(payment.receipt(), ReceiptStatus::Provisional, "{method:?}");
        // Still pending before its expiry, and still provisional.
        assert_eq!(payment.expire(at(10)), Ok(false));
        assert!(!payment.is_confirmed());
        payment
            .confirm(Evidence::Statement)
            .expect("confirmed by the statement");
        assert!(payment.is_confirmed());
        assert_eq!(payment.receipt(), ReceiptStatus::Confirmed);
    }
}

#[test]
fn a_non_cash_payment_needs_its_reference_and_provider_evidence() {
    assert_eq!(
        Payment::pending(&new(Method::MtnMomo, 1), " ", at(0), at(1)),
        Err(PaymentRefused::NoReference)
    );
    assert!(matches!(
        Payment::pending(&new(Method::Cash, 1), "r", at(0), at(1)),
        Err(PaymentRefused::WrongState { .. })
    ));
    assert_eq!(
        Payment::pending(&new(Method::Card, 0), "r", at(0), at(1)),
        Err(PaymentRefused::NonPositive)
    );
    let mut momo = Payment::pending(&new(Method::MtnMomo, 20_000), "x-ref-uuid", at(0), at(600))
        .expect("pending");
    assert_eq!(momo.status, Status::Pending);
    assert_eq!(momo.reference.as_deref(), Some("x-ref-uuid"));
    momo.confirm(Evidence::Callback).expect("callback");
    assert_eq!(momo.evidence, Some(Evidence::Callback));
    assert!(matches!(
        momo.confirm(Evidence::Poll),
        Err(PaymentRefused::WrongState { .. })
    ));

    // The customer's SMS is not evidence: only a statement line confirms it.
    let mut reported = Payment::pending(
        &new(Method::CustomerReported, 5_000),
        "SMS-REF-77",
        at(0),
        at(86_400),
    )
    .expect("pending");
    assert_eq!(reported.status, Status::PendingUnverified);
    assert_eq!(
        reported.confirm(Evidence::Callback),
        Err(PaymentRefused::StatementRequired)
    );
    assert!(!reported.is_confirmed());
}

#[test]
fn pending_payments_are_voided_or_expire_and_posted_ones_are_reversed() {
    let mut failed =
        Payment::pending(&new(Method::AirtelMoney, 1_000), "r1", at(0), at(600)).expect("pending");
    failed.void("insufficient funds").expect("voided");
    assert_eq!(failed.receipt(), ReceiptStatus::Void);
    assert!(matches!(
        failed.confirm(Evidence::Callback),
        Err(PaymentRefused::WrongState { .. })
    ));

    let mut late =
        Payment::pending(&new(Method::MtnMomo, 1_000), "r2", at(0), at(600)).expect("pending");
    assert_eq!(late.expire(at(600)), Ok(true));
    assert_eq!(late.status, Status::Expired);
    assert!(!late.is_confirmed());

    let mut s = shift();
    let mut cash = Payment::cash(&new(Method::Cash, 10_000), &mut s, at(5)).expect("cash");
    assert!(cash.is_confirmed());
    assert!(matches!(
        cash.void("x"),
        Err(PaymentRefused::WrongState { .. })
    ));
    cash.reverse("entered twice").expect("reversed");
    assert_eq!(cash.receipt(), ReceiptStatus::Void);
}

#[test]
fn cash_needs_an_open_shift_and_refunds_need_the_permission() {
    let mut s = shift();
    let mut cash = Payment::cash(&new(Method::Cash, 30_000), &mut s, at(5)).expect("cash");
    assert_eq!(cash.shift.as_deref(), Some("shift-1"));
    assert!(matches!(
        Payment::cash(&new(Method::MtnMomo, 1), &mut s, at(6)),
        Err(PaymentRefused::WrongState { .. })
    ));
    assert_eq!(
        cash.refund(ugx(5_000), false),
        Err(PaymentRefused::NotPermitted)
    );
    cash.refund(ugx(5_000), true).expect("refunded");
    assert_eq!(cash.refund(ugx(25_001), true), Err(PaymentRefused::TooMuch));
    s.close(ugx(80_000)).expect("closed");
    assert_eq!(
        Payment::cash(&new(Method::Cash, 1_000), &mut s, at(7)),
        Err(PaymentRefused::NoShift)
    );
    let mut pending =
        Payment::pending(&new(Method::Card, 1_000), "r", at(0), at(9)).expect("pending");
    assert!(matches!(
        pending.refund(ugx(1), true),
        Err(PaymentRefused::WrongState { .. })
    ));
}

#[test]
fn a_shift_settles_at_close_or_waits_for_a_supervisor() {
    let mut exact = shift();
    Payment::cash(&new(Method::Cash, 30_000), &mut exact, at(5)).expect("cash");
    exact.pay_out(ugx(5_000)).expect("refund paid out");
    assert_eq!(exact.expected(), ugx(75_000));
    assert_eq!(exact.close(ugx(75_000)), Ok(Decimal::ZERO));
    assert_eq!(exact.status, ShiftStatus::Settled);

    let mut short = shift();
    Payment::cash(&new(Method::Cash, 30_000), &mut short, at(5)).expect("cash");
    assert_eq!(short.close(ugx(79_500)), Ok(ugx(-500)));
    assert_eq!(short.status, ShiftStatus::AwaitingApproval);
    assert_eq!(short.approve("cashier-1"), Err(ShiftRefused::NotSupervisor));
    assert_eq!(short.approve("supervisor-1"), Ok(ugx(-500)));
    assert_eq!(short.status, ShiftStatus::Settled);
    assert!(matches!(
        short.close(ugx(1)),
        Err(ShiftRefused::WrongState { .. })
    ));
    assert!(Shift::open("s", "t", "c", ugx(-1), at(0)).is_err());
}

#[test]
fn a_provider_statement_confirms_pending_payments_and_queues_the_exceptions() {
    use health_payments::{StatementLine, reconcile};
    let pending = |id: &'static str, reference: &str, amount: i64, method: Method| {
        Payment::pending(
            &NewPayment {
                id,
                ..new(method, amount)
            },
            reference,
            at(0),
            at(86_400),
        )
        .expect("pending")
    };
    let mut posted = pending("p-posted", "ref-posted", 7_000, Method::MtnMomo);
    posted.confirm(Evidence::Callback).expect("callback");
    let payments = vec![
        pending("p-ok", "ref-ok", 20_000, Method::MtnMomo),
        pending("p-sms", "SMS-REF-77", 5_000, Method::CustomerReported),
        pending("p-short", "ref-short", 10_000, Method::MtnMomo),
        posted,
    ];
    let lines = vec![
        StatementLine {
            reference: "ref-ok".into(),
            amount: ugx(20_000),
        },
        StatementLine {
            reference: "SMS-REF-77".into(),
            amount: ugx(5_000),
        },
        StatementLine {
            reference: "ref-short".into(),
            amount: ugx(9_000),
        },
        StatementLine {
            reference: "ref-stranger".into(),
            amount: ugx(1_000),
        },
    ];
    let result = reconcile(&payments, &lines);
    assert_eq!(result.confirm, ["p-ok", "p-sms"]);
    assert_eq!(
        result.mismatched,
        [("p-short".to_owned(), ugx(10_000), ugx(9_000))]
    );
    assert_eq!(result.unknown, ["ref-stranger"]);
    assert_eq!(
        result.missing,
        ["p-posted"],
        "posted by callback but not on the statement"
    );
}

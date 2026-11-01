//! A cashier's shift through the financial ledger: cash, a mobile-money payment confirmed by the
//! provider, one voided, a deposit, a refund and a till shortage. Pending money sits in payments
//! in transit and is never counted as received (§68).

use health_financial_ledger::{
    ChartOfAccounts, ChartTemplate, Currency, DEPOSITS, Dimensions, IN_TRANSIT, Ledger,
    TILL_VARIANCE,
};
use health_payments::{Evidence, Method, NewPayment, Payment, Purpose, Shift};
use health_payments_app::{Accounts, Ledgering, PostingError, Step};
use health_time::Timestamp;
use rust_decimal::Decimal;

fn at(seconds: i64) -> Timestamp {
    Timestamp::from_second(1_800_000_000 + seconds).expect("time")
}

fn ugx(n: i64) -> Decimal {
    Decimal::from(n)
}

fn new(id: &'static str, method: Method, purpose: Purpose, amount: i64) -> NewPayment<'static> {
    NewPayment {
        id,
        method,
        purpose,
        amount: ugx(amount),
        payer: "Account/acc-1",
    }
}

fn balance(ledger: &Ledger, account: &str) -> Decimal {
    ledger.balance(account, |_: &Dimensions| true).amount()
}

#[test]
fn a_shift_through_the_ledger() {
    let currency = Currency::from_code("UGX").expect("currency");
    let chart = ChartOfAccounts::from_template(&ChartTemplate {
        funding_sources: vec!["patient".into()],
        tills: vec![
            "till-1".into(),
            "merchant-mtn".into(),
            "merchant-airtel".into(),
        ],
        ..ChartTemplate::default()
    })
    .expect("chart");
    let mut ledger = Ledger::new("KAM013", currency, chart);
    let accounts = Accounts {
        receivable: "receivable/patient".into(),
        merchants: vec![
            (Method::MtnMomo, "cash/merchant-mtn".into()),
            (Method::AirtelMoney, "cash/merchant-airtel".into()),
        ],
    };
    let ledgering = Ledgering {
        accounts: &accounts,
        facility: "KAM013",
        currency,
    };
    let till = Some("till-1");
    let mut shift = Shift::open("shift-1", "till-1", "cashier-1", ugx(0), at(0)).expect("shift");

    let mut cash = Payment::cash(
        &new("cash-1", Method::Cash, Purpose::Settlement, 30_000),
        &mut shift,
        at(1),
    )
    .expect("cash");
    ledger
        .post(
            ledgering
                .post(&cash, Step::Recorded, till, at(1))
                .expect("posting"),
        )
        .expect("posted");
    let deposit = Payment::cash(
        &new("dep-1", Method::Cash, Purpose::Deposit, 10_000),
        &mut shift,
        at(2),
    )
    .expect("deposit");
    ledger
        .post(
            ledgering
                .post(&deposit, Step::Recorded, till, at(2))
                .expect("posting"),
        )
        .expect("posted");

    let mut momo = Payment::pending(
        &new("momo-1", Method::MtnMomo, Purpose::Settlement, 20_000),
        "x-ref-1",
        at(3),
        at(603),
    )
    .expect("pending");
    ledger
        .post(
            ledgering
                .post(&momo, Step::Recorded, None, at(3))
                .expect("posting"),
        )
        .expect("posted");
    // Pending: in transit, not received.
    assert_eq!(balance(&ledger, IN_TRANSIT), ugx(20_000));
    assert_eq!(balance(&ledger, "cash/merchant-mtn"), Decimal::ZERO);
    assert!(matches!(
        ledgering.post(&momo, Step::Confirmed, None, at(4)),
        Err(PostingError::WrongStep(..))
    ));
    momo.confirm(Evidence::Callback).expect("confirmed");
    ledger
        .post(
            ledgering
                .post(&momo, Step::Confirmed, None, at(4))
                .expect("posting"),
        )
        .expect("posted");
    assert_eq!(balance(&ledger, "cash/merchant-mtn"), ugx(20_000));

    let mut airtel = Payment::pending(
        &new("airtel-1", Method::AirtelMoney, Purpose::Settlement, 5_000),
        "a-ref-1",
        at(5),
        at(605),
    )
    .expect("pending");
    ledger
        .post(
            ledgering
                .post(&airtel, Step::Recorded, None, at(5))
                .expect("posting"),
        )
        .expect("posted");
    airtel.void("insufficient funds").expect("voided");
    ledger
        .post(
            ledgering
                .post(&airtel, Step::Ended, None, at(6))
                .expect("posting"),
        )
        .expect("posted");
    assert_eq!(balance(&ledger, IN_TRANSIT), Decimal::ZERO);
    assert_eq!(balance(&ledger, "cash/merchant-airtel"), Decimal::ZERO);

    cash.refund(ugx(5_000), true).expect("refund");
    shift.pay_out(ugx(5_000)).expect("paid out");
    ledger
        .post(
            ledgering
                .post(&cash, Step::Refunded(ugx(5_000)), till, at(7))
                .expect("posting"),
        )
        .expect("posted");

    // 35,000 expected; 34,500 counted; a supervisor approves the shortage.
    assert_eq!(shift.close(ugx(34_500)), Ok(ugx(-500)));
    shift.approve("supervisor-1").expect("approved");
    ledger
        .post(ledgering.variance(&shift, at(8)).expect("variance"))
        .expect("posted");

    assert_eq!(ledger.trial_balance(), Decimal::ZERO);
    assert_eq!(balance(&ledger, "cash/till-1"), ugx(34_500));
    assert_eq!(balance(&ledger, DEPOSITS), ugx(10_000));
    assert_eq!(balance(&ledger, TILL_VARIANCE), ugx(500));
    // Received from the patient: 30,000 cash + 20,000 MTN − 5,000 refunded (the voided Airtel
    // payment is back on their account).
    assert_eq!(balance(&ledger, "receivable/patient"), ugx(-45_000));
}

#[test]
fn steps_that_do_not_fit_are_refused() {
    let currency = Currency::from_code("UGX").expect("currency");
    let accounts = Accounts {
        receivable: "receivable/patient".into(),
        merchants: vec![],
    };
    let ledgering = Ledgering {
        accounts: &accounts,
        facility: "KAM013",
        currency,
    };
    let mut card = Payment::pending(
        &new("card-1", Method::Card, Purpose::Settlement, 1_000),
        "c-ref",
        at(0),
        at(60),
    )
    .expect("pending");
    card.confirm(Evidence::Poll).expect("confirmed");
    assert_eq!(
        ledgering.post(&card, Step::Confirmed, None, at(1)),
        Err(PostingError::NoMerchant(Method::Card))
    );
    let mut shift = Shift::open("s", "till-1", "c", ugx(0), at(0)).expect("shift");
    let cash = Payment::cash(
        &new("cash-2", Method::Cash, Purpose::Settlement, 1_000),
        &mut shift,
        at(1),
    )
    .expect("cash");
    assert_eq!(
        ledgering.post(&cash, Step::Recorded, None, at(1)),
        Err(PostingError::NoTill)
    );
    assert!(matches!(
        ledgering.post(&cash, Step::Ended, Some("till-1"), at(1)),
        Err(PostingError::WrongStep(..))
    ));
    shift.close(ugx(1_000)).expect("closed");
    assert_eq!(
        ledgering.variance(&shift, at(2)),
        None,
        "no variance, no posting"
    );
}

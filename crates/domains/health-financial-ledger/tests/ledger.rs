//! The facility ledger (ADR 0030): exact money, balanced postings, reversals only, and balances
//! derived from postings.

use std::str::FromStr;

use health_financial_ledger::{
    ChartOfAccounts, ChartTemplate, Currency, Dimensions, Ledger, Line, Money, MoneyError,
    Original, Posting, PostingRefused, ROUNDING, Side,
};
use health_time::Timestamp;
use rust_decimal::Decimal;

fn d(text: &str) -> Decimal {
    Decimal::from_str(text).expect("decimal")
}

fn ugx() -> Currency {
    Currency::from_code("UGX").expect("UGX")
}

fn usd() -> Currency {
    Currency::from_code("USD").expect("USD")
}

fn ugx_money(amount: &str) -> Money {
    Money::new(d(amount), ugx()).expect("money")
}

fn at() -> Timestamp {
    Timestamp::from_second(1_800_000_000).expect("time")
}

fn ledger() -> Ledger {
    let chart = ChartOfAccounts::from_template(&ChartTemplate {
        funding_sources: vec!["patient".into(), "nhis".into(), "government".into()],
        service_classes: vec!["consultation".into(), "laboratory".into()],
        tills: vec!["till-1".into()],
    })
    .expect("chart");
    Ledger::new("KAM013", ugx(), chart)
}

fn dims(encounter: &str, funding: &str) -> Dimensions {
    Dimensions {
        facility: "KAM013".into(),
        service: Some("consultation".into()),
        funding_source: Some(funding.into()),
        encounter: Some(encounter.into()),
        ..Dimensions::default()
    }
}

fn line(account: &str, side: Side, amount: &str, dimensions: &Dimensions) -> Line {
    Line {
        account: account.into(),
        side,
        amount: ugx_money(amount),
        dimensions: dimensions.clone(),
        original: None,
    }
}

fn posting(id: &str, lines: Vec<Line>) -> Posting {
    Posting {
        id: id.into(),
        at: at(),
        description: id.into(),
        lines,
        reverses: None,
    }
}

#[test]
fn money_is_exact_at_the_currency_scale_and_rounds_half_even() {
    assert_eq!(ugx().scale(), 0);
    assert_eq!(usd().scale(), 2);
    assert!(Currency::from_code("XYZ").is_err());
    // UGX has no minor unit: 1.5 UGX is refused unless rounded on purpose.
    assert!(matches!(
        Money::new(d("1.5"), ugx()),
        Err(MoneyError::TooPrecise { .. })
    ));
    assert_eq!(
        Money::new(d("12.30"), usd()).expect("usd").amount(),
        d("12.30")
    );
    for (amount, rounded) in [("2.5", "2"), ("3.5", "4"), ("-2.5", "-2")] {
        assert_eq!(
            Money::rounded(d(amount), ugx()).amount(),
            d(rounded),
            "{amount}"
        );
    }
    for (amount, rounded) in [("0.125", "0.12"), ("0.135", "0.14")] {
        assert_eq!(
            Money::rounded(d(amount), usd()).amount(),
            d(rounded),
            "{amount}"
        );
    }
    assert!(matches!(
        ugx_money("1").plus(Money::zero(usd())),
        Err(MoneyError::CurrencyMismatch(..))
    ));
}

#[test]
fn largest_remainder_splits_always_sum_to_the_whole() {
    let shares = ugx_money("100")
        .split(&[d("1"), d("1"), d("1")])
        .expect("split");
    assert_eq!(
        shares.iter().map(Money::amount).collect::<Vec<_>>(),
        [d("34"), d("33"), d("33")]
    );
    let shares = Money::new(d("10.00"), usd())
        .expect("usd")
        .split(&[d("1"), d("1"), d("1")])
        .expect("split");
    assert_eq!(
        shares.iter().map(Money::amount).collect::<Vec<_>>(),
        [d("3.34"), d("3.33"), d("3.33")]
    );
    // 30/70 of 10,001 UGX: the left-over shilling goes to the larger remainder.
    let shares = ugx_money("10001")
        .split(&[d("0.3"), d("0.7")])
        .expect("split");
    assert_eq!(
        shares.iter().map(Money::amount).collect::<Vec<_>>(),
        [d("3000"), d("7001")]
    );
    // Negative amounts (refunds) split the same way.
    let shares = ugx_money("-100")
        .split(&[d("1"), d("1"), d("1")])
        .expect("split");
    assert_eq!(
        shares.iter().map(Money::amount).collect::<Vec<_>>(),
        [d("-34"), d("-33"), d("-33")]
    );
    assert_eq!(
        ugx_money("5").split(&[d("0"), d("0")]),
        Err(MoneyError::BadWeights)
    );
    assert_eq!(
        ugx_money("5").split(&[d("-1"), d("2")]),
        Err(MoneyError::BadWeights)
    );
    // Exhaustively: every amount and weighting sums exactly, and no share goes negative.
    for amount in 0..200 {
        for weights in [
            vec![d("1"), d("2"), d("3")],
            vec![d("0.15"), d("0.85")],
            vec![d("1"), d("0"), d("7")],
        ] {
            let whole = ugx_money(&amount.to_string());
            let shares = whole.split(&weights).expect("split");
            let sum: Decimal = shares.iter().map(Money::amount).sum();
            assert_eq!(sum, whole.amount(), "{amount} by {weights:?}");
            assert!(shares.iter().all(|s| !s.amount().is_sign_negative()));
        }
    }
}

#[test]
fn the_template_gives_the_chart_of_accounts() {
    let chart = ChartOfAccounts::from_template(&ChartTemplate {
        funding_sources: vec!["patient".into()],
        service_classes: vec!["pharmacy".into()],
        tills: vec!["main".into()],
    })
    .expect("chart");
    for code in [
        "receivable/patient",
        "waiver/patient",
        "revenue/pharmacy",
        "cash/main",
        "deposits/patient",
        "clearing/in-transit",
        ROUNDING,
        "variance/till",
    ] {
        assert!(chart.account(code).is_some(), "{code}");
    }
    assert!(
        ChartOfAccounts::from_template(&ChartTemplate {
            funding_sources: vec!["patient".into(), "patient".into()],
            ..ChartTemplate::default()
        })
        .is_err()
    );
}

#[test]
fn a_visit_is_charged_paid_and_balanced() {
    let mut ledger = ledger();
    let visit = dims("Encounter/e1", "patient");
    ledger
        .post(posting(
            "charge-1",
            vec![
                line("receivable/patient", Side::Debit, "5000", &visit),
                line("revenue/consultation", Side::Credit, "5000", &visit),
            ],
        ))
        .expect("charge");
    ledger
        .post(posting(
            "payment-1",
            vec![
                line("cash/till-1", Side::Debit, "3000", &visit),
                line("receivable/patient", Side::Credit, "3000", &visit),
            ],
        ))
        .expect("payment");
    assert_eq!(ledger.trial_balance(), Decimal::ZERO);
    assert_eq!(ledger.balance("cash/till-1", |_| true).amount(), d("3000"));
    assert_eq!(
        ledger.balance("revenue/consultation", |_| true).amount(),
        d("5000")
    );
    let owed = ledger.encounter_balances("Encounter/e1");
    assert_eq!(owed.len(), 1);
    assert_eq!(owed["patient"].amount(), d("2000"));
    assert!(ledger.encounter_balances("Encounter/other").is_empty());
}

#[test]
fn every_posting_balances_and_is_checked() {
    let mut ledger = ledger();
    let visit = dims("Encounter/e1", "patient");
    let refused = |ledger: &mut Ledger, p: Posting| ledger.post(p).expect_err("refused");
    assert!(matches!(
        refused(
            &mut ledger,
            posting(
                "p",
                vec![
                    line("receivable/patient", Side::Debit, "5000", &visit),
                    line("revenue/consultation", Side::Credit, "4999", &visit),
                ]
            )
        ),
        PostingRefused::Unbalanced { .. }
    ));
    assert_eq!(
        refused(
            &mut ledger,
            posting(
                "p",
                vec![line("receivable/patient", Side::Debit, "5000", &visit)]
            )
        ),
        PostingRefused::TooFewLines
    );
    assert_eq!(
        refused(
            &mut ledger,
            posting(
                "p",
                vec![
                    line("receivable/patient", Side::Debit, "0", &visit),
                    line("revenue/consultation", Side::Credit, "0", &visit),
                ]
            )
        ),
        PostingRefused::NonPositive
    );
    assert_eq!(
        refused(
            &mut ledger,
            posting(
                "p",
                vec![
                    line("receivable/patient", Side::Debit, "5000", &visit),
                    line("revenue/surgery", Side::Credit, "5000", &visit),
                ]
            )
        ),
        PostingRefused::UnknownAccount("revenue/surgery".into())
    );
    let elsewhere = Dimensions {
        facility: "WAK002".into(),
        ..visit.clone()
    };
    assert_eq!(
        refused(
            &mut ledger,
            posting(
                "p",
                vec![
                    line("receivable/patient", Side::Debit, "5000", &elsewhere),
                    line("revenue/consultation", Side::Credit, "5000", &elsewhere),
                ]
            )
        ),
        PostingRefused::OtherFacility("KAM013".into())
    );
    let mut dollars = posting(
        "p",
        vec![
            line("receivable/patient", Side::Debit, "5000", &visit),
            line("revenue/consultation", Side::Credit, "5000", &visit),
        ],
    );
    for l in &mut dollars.lines {
        l.amount = Money::new(d("5"), usd()).expect("usd");
    }
    assert_eq!(
        refused(&mut ledger, dollars),
        PostingRefused::WrongCurrency(ugx())
    );
    let good = posting(
        "p",
        vec![
            line("receivable/patient", Side::Debit, "5000", &visit),
            line("revenue/consultation", Side::Credit, "5000", &visit),
        ],
    );
    ledger.post(good.clone()).expect("posted");
    assert_eq!(
        refused(&mut ledger, good),
        PostingRefused::DuplicateId("p".into())
    );
}

#[test]
fn corrections_are_reversals_made_once() {
    let mut ledger = ledger();
    let visit = dims("Encounter/e1", "patient");
    ledger
        .post(posting(
            "charge-1",
            vec![
                line("receivable/patient", Side::Debit, "5000", &visit),
                line("revenue/consultation", Side::Credit, "5000", &visit),
            ],
        ))
        .expect("charge");
    let reversal = ledger
        .reversal("charge-1", "reversal-1", at(), "charged twice")
        .expect("reversal");
    assert_eq!(reversal.reverses.as_deref(), Some("charge-1"));
    assert_eq!(reversal.lines[0].side, Side::Credit);
    ledger.post(reversal).expect("posted");
    // The original stays; its effect is undone.
    assert_eq!(ledger.postings().len(), 2);
    assert!(ledger.encounter_balances("Encounter/e1").is_empty());
    assert_eq!(ledger.trial_balance(), Decimal::ZERO);
    // Never twice, and a reversal is not itself reversed.
    assert!(matches!(
        ledger.reversal("charge-1", "reversal-2", at(), "again"),
        Err(PostingRefused::NotReversible(_))
    ));
    assert!(matches!(
        ledger.reversal("reversal-1", "reversal-3", at(), "undo"),
        Err(PostingRefused::NotReversible(_))
    ));
    assert!(matches!(
        ledger.reversal("missing", "reversal-4", at(), "none"),
        Err(PostingRefused::NotReversible(_))
    ));
}

#[test]
fn a_charge_split_between_patient_and_insurer_balances_exactly() {
    let mut ledger = ledger();
    let charge = ugx_money("10001");
    let shares = charge.split(&[d("0.3"), d("0.7")]).expect("split");
    let patient = dims("Encounter/e2", "patient");
    let insurer = dims("Encounter/e2", "nhis");
    let mut lines = vec![
        Line {
            account: "receivable/patient".into(),
            side: Side::Debit,
            amount: shares[0],
            dimensions: patient.clone(),
            original: None,
        },
        Line {
            account: "receivable/nhis".into(),
            side: Side::Debit,
            amount: shares[1],
            dimensions: insurer,
            original: None,
        },
    ];
    lines.push(Line {
        account: "revenue/consultation".into(),
        side: Side::Credit,
        amount: charge,
        dimensions: patient,
        original: None,
    });
    ledger
        .post(posting("split-1", lines))
        .expect("split charge");
    let owed = ledger.encounter_balances("Encounter/e2");
    assert_eq!(owed["patient"].amount(), d("3000"));
    assert_eq!(owed["nhis"].amount(), d("7001"));
}

#[test]
fn free_care_is_priced_and_allocated_to_its_funder() {
    // §70: the patient share is zero, but consumption is recorded at the reference tariff.
    let mut ledger = ledger();
    let funded = dims("Encounter/e3", "government");
    ledger
        .post(posting(
            "free-1",
            vec![
                line("receivable/government", Side::Debit, "15000", &funded),
                line("revenue/laboratory", Side::Credit, "15000", &funded),
            ],
        ))
        .expect("free care");
    let owed = ledger.encounter_balances("Encounter/e3");
    assert_eq!(owed.get("patient"), None);
    assert_eq!(owed["government"].amount(), d("15000"));
    assert_eq!(
        ledger
            .balance("revenue/laboratory", |d| d.funding_source.as_deref()
                == Some("government"))
            .amount(),
        d("15000")
    );
}

#[test]
fn foreign_payments_keep_their_original_amount_and_rate() {
    let mut ledger = ledger();
    let visit = dims("Encounter/e4", "patient");
    ledger
        .post(posting(
            "charge-4",
            vec![
                line("receivable/patient", Side::Debit, "37000", &visit),
                line("revenue/consultation", Side::Credit, "37000", &visit),
            ],
        ))
        .expect("charge");
    // USD 10.00 at 3,700 UGX per USD.
    let original = Original {
        amount: Money::new(d("10.00"), usd()).expect("usd"),
        rate: d("3700"),
    };
    let converted = Money::rounded(original.amount.amount() * original.rate, ugx());
    let mut payment = posting(
        "payment-4",
        vec![
            line("cash/till-1", Side::Debit, "37000", &visit),
            line("receivable/patient", Side::Credit, "37000", &visit),
        ],
    );
    payment.lines[0].amount = converted;
    payment.lines[0].original = Some(original);
    ledger.post(payment).expect("payment");
    assert!(ledger.encounter_balances("Encounter/e4").is_empty());
    assert_eq!(ledger.postings()[1].lines[0].original, Some(original));
}

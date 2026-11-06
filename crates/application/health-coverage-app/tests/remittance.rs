//! Remittance postings (#126): a payer's payment against a claim clears the coverage's
//! receivable that billing debited, through the ledger's own checks.

use health_coverage::claims::Remittance;
use health_coverage_app::remittance_posting;
use health_financial_ledger::{
    ChartOfAccounts, ChartTemplate, Currency, Dimensions, IN_TRANSIT, Ledger, Line, Money, Posting,
    Side,
};
use health_time::Timestamp;
use rust_decimal::Decimal;

#[test]
fn a_remittance_clears_the_coverage_s_receivable() {
    let ugx = Currency::from_code("UGX").expect("currency");
    let chart = ChartOfAccounts::from_template(&ChartTemplate {
        funding_sources: vec!["patient".into(), "cov-1".into()],
        service_classes: vec!["consultation".into()],
        tills: vec![],
        inventory: false,
    })
    .expect("chart");
    let mut ledger = Ledger::new("KAM013", ugx, chart);
    let at: Timestamp = "2026-11-06T09:00:00Z".parse().expect("time");
    let money = |n: i64| Money::rounded(Decimal::from(n), ugx);
    let line = |account: &str, side: Side, n: i64| Line {
        account: account.into(),
        side,
        amount: money(n),
        dimensions: Dimensions {
            facility: "KAM013".into(),
            ..Dimensions::default()
        },
        original: None,
    };
    // As billing posts a charge: the coverage owes 72 000 of it.
    ledger
        .post(Posting {
            id: "charge-c1".into(),
            at,
            description: "Charge c1".into(),
            lines: vec![
                line("receivable/cov-1", Side::Debit, 72_000),
                line("receivable/patient", Side::Debit, 28_000),
                line("revenue/consultation", Side::Credit, 100_000),
            ],
            reverses: None,
        })
        .expect("charge posted");

    let paid = Remittance {
        claim: "claim-1".into(),
        payer: "Organization/aar".into(),
        amount: Decimal::from(60_000),
        reference: "AAR-PAY-9".into(),
        at: "2026-12-10T00:00:00Z".parse().expect("time"),
    };
    let posting = remittance_posting(&paid, "cov-1", "KAM013", ugx);
    ledger
        .post(posting.clone())
        .expect("balanced and on the chart");
    assert_eq!(
        ledger.balance("receivable/cov-1", |_| true),
        money(12_000),
        "what is still owed"
    );
    assert_eq!(ledger.balance(IN_TRANSIT, |_| true), money(60_000));
    assert!(ledger.post(posting).is_err(), "posted once");
}

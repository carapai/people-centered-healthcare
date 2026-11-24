use std::collections::BTreeMap;

use rust_decimal::Decimal;

use super::*;

fn given(code: &str) -> EstimateTypeDefinition {
    EstimateTypeDefinition {
        code: code.into(),
        title: code.into(),
        derived_from: None,
        percent: None,
    }
}

fn derived(code: &str, from: &str, percent: &str) -> EstimateTypeDefinition {
    EstimateTypeDefinition {
        derived_from: Some(from.into()),
        percent: Some(percent.into()),
        ..given(code)
    }
}

fn figure(place: &str, kind: &str, value: u64) -> Figure {
    Figure {
        place: place.into(),
        kind: kind.into(),
        value,
    }
}

fn uganda_2026(figures: Vec<Figure>) -> EstimatesDefinition {
    EstimatesDefinition {
        year: 2026,
        types: vec![
            given("total-population"),
            derived("expected-pregnancies", "total-population", "5"),
            given("under-1"),
        ],
        figures,
    }
}

/// Two districts of one region; Kampala has two facilities, Wakiso one.
fn hierarchy() -> BTreeMap<&'static str, &'static str> {
    BTreeMap::from([
        ("region-central", "central"),
        ("district-kampala", "region-central"),
        ("district-wakiso", "region-central"),
        ("KAM013", "district-kampala"),
        ("KAM020", "district-kampala"),
        ("WAK001", "district-wakiso"),
    ])
}

fn estimates(figures: Vec<Figure>) -> Estimates {
    let tree = hierarchy();
    Estimates::new(uganda_2026(figures), move |code| {
        tree.get(code).map(|p| (*p).to_owned())
    })
}

fn month() -> Period {
    Period::parse(PeriodType::Monthly, "2026-03").expect("march")
}

#[test]
fn expected_pregnancies_are_five_percent_of_the_total_population() {
    let estimates = estimates(vec![
        figure("district-kampala", "total-population", 1_797_722),
        figure("district-wakiso", "total-population", 3_411_177),
    ]);
    // By hand: 5% of 1,797,722 is 89,886.1; of Wakiso's 3,411,177, 170,558.85.
    assert_eq!(
        estimates.annual("district-kampala", "expected-pregnancies"),
        Some(Decimal::new(898_861, 1))
    );
    assert_eq!(
        estimates.annual("district-wakiso", "expected-pregnancies"),
        Some(Decimal::new(17_055_885, 2))
    );
    // The region: 5% of the summed 5,208,899.
    assert_eq!(
        estimates.annual("region-central", "expected-pregnancies"),
        Some(Decimal::new(26_044_495, 2))
    );
}

#[test]
fn a_district_s_month_is_its_annual_figure_over_twelve_and_a_facility_without_one_has_none() {
    let estimates = estimates(vec![figure(
        "district-kampala",
        "total-population",
        1_200_000,
    )]);
    assert_eq!(
        estimates.for_period("district-kampala", "total-population", &month()),
        Some(Decimal::from(100_000))
    );
    // Never split down: the district's figure is not shared among its facilities.
    assert_eq!(
        estimates.for_period("KAM013", "total-population", &month()),
        None
    );
    assert_eq!(
        estimates.for_period("KAM013", "expected-pregnancies", &month()),
        None
    );
    // A type nobody gave has no estimate anywhere.
    assert_eq!(estimates.annual("district-kampala", "under-1"), None);
    assert_eq!(estimates.annual("district-kampala", "unknown"), None);
}

#[test]
fn figures_sum_upward_and_a_place_s_own_figure_wins() {
    let estimates = estimates(vec![
        figure("KAM013", "total-population", 40_000),
        figure("KAM020", "total-population", 60_000),
        figure("district-wakiso", "total-population", 300_000),
        // Given below a district that has its own: the district keeps its own.
        figure("WAK001", "total-population", 10_000),
    ]);
    assert_eq!(
        estimates.annual("district-kampala", "total-population"),
        Some(Decimal::from(100_000))
    );
    assert_eq!(
        estimates.annual("district-wakiso", "total-population"),
        Some(Decimal::from(300_000))
    );
    assert_eq!(
        estimates.annual("region-central", "total-population"),
        Some(Decimal::from(400_000))
    );
    assert_eq!(
        estimates.annual("central", "total-population"),
        Some(Decimal::from(400_000))
    );
    assert_eq!(
        estimates.annual("WAK001", "total-population"),
        Some(Decimal::from(10_000))
    );
}

#[test]
fn periods_take_the_annual_figure_pro_rata() {
    let annual = Decimal::from(73_000);
    assert_eq!(
        pro_rata(annual, PeriodType::Monthly),
        annual / Decimal::from(12)
    );
    assert_eq!(
        pro_rata(annual, PeriodType::Quarterly),
        Decimal::from(18_250)
    );
    assert_eq!(
        pro_rata(annual, PeriodType::SemiAnnual),
        Decimal::from(36_500)
    );
    assert_eq!(pro_rata(annual, PeriodType::Weekly), Decimal::from(1_400));
    assert_eq!(pro_rata(annual, PeriodType::Annual), annual);
    assert_eq!(pro_rata(annual, PeriodType::FinancialJuly), annual);
}

#[test]
fn a_percentage_over_an_estimate() {
    // 750 first ANC visits in March over Kampala's 89,886.1 / 12 expected pregnancies.
    let month = Decimal::new(898_861, 1) / Decimal::from(12);
    assert_eq!(
        over_estimate(Calculation::Percentage, 750, Some(month)),
        Some(Decimal::new(1001, 2))
    );
    assert_eq!(
        over_estimate(
            Calculation::Rate { per: 1_000 },
            3,
            Some(Decimal::from(1_500))
        ),
        Some(Decimal::TWO)
    );
    assert_eq!(over_estimate(Calculation::Percentage, 750, None), None);
    assert_eq!(
        over_estimate(Calculation::Percentage, 750, Some(Decimal::ZERO)),
        None
    );
    assert_eq!(over_estimate(Calculation::Count, 750, Some(month)), None);
}

#[test]
fn a_csv_with_an_unknown_place_code_is_refused_with_the_row_named() {
    let definition = uganda_2026(Vec::new());
    let known = |code: &str| hierarchy().contains_key(code);
    let csv = "place,total-population,under-1\r\n\
               district-kampala,\"1,797,722\",\n\
               district-nowhere,250000,9000\n\
               \n\
               KAM013,40000,1500\n";
    assert_eq!(
        parse_estimates_csv(csv, &definition, known),
        Err(vec![
            "row 3: the place district-nowhere is not in the facility registry or the NHFR".into()
        ])
    );
    let fixed = csv.replace("district-nowhere", "district-wakiso");
    assert_eq!(
        parse_estimates_csv(&fixed, &definition, known),
        Ok(vec![
            figure("district-kampala", "total-population", 1_797_722),
            figure("district-wakiso", "total-population", 250_000),
            figure("district-wakiso", "under-1", 9_000),
            figure("KAM013", "total-population", 40_000),
            figure("KAM013", "under-1", 1_500),
        ])
    );
}

#[test]
fn a_csv_refuses_derived_and_unknown_columns_repeated_places_and_bad_values() {
    let definition = uganda_2026(Vec::new());
    let known = |_: &str| true;
    assert_eq!(
        parse_estimates_csv("code,expected-pregnancies,births\n", &definition, known),
        Err(vec![
            "row 1: the first column must be place, the place's code".into(),
            "row 1: the column expected-pregnancies is derived from total-population, not given"
                .into(),
            "row 1: the column births is not one of the year's estimate types".into(),
        ])
    );
    assert_eq!(
        parse_estimates_csv(
            "place,total-population\nKAM013,12.5\nKAM013,-3\n,4\n",
            &definition,
            known
        ),
        Err(vec![
            "row 2: total-population for KAM013 is 12.5, not a whole number".into(),
            "row 3: the place KAM013 is already on row 2".into(),
            "row 3: total-population for KAM013 is -3, not a whole number".into(),
            "row 4: the row names no place".into(),
        ])
    );
    assert_eq!(
        parse_estimates_csv("", &definition, known),
        Err(vec!["the file is empty".into()])
    );
}

#[test]
fn the_definition_s_problems() {
    assert_eq!(
        uganda_2026(vec![figure("KAM013", "under-1", 3)]).problems(),
        Vec::<String>::new()
    );
    let wrong = EstimatesDefinition {
        year: 26,
        types: vec![
            given("total-population"),
            given("total-population"),
            derived("a", "b", "5"),
            derived("b", "a", "5"),
            derived("c", "total-population", "0"),
            derived("d", "nothing", "101"),
            EstimateTypeDefinition {
                percent: Some("5".into()),
                ..given("e")
            },
        ],
        figures: vec![
            figure("KAM013", "c", 1),
            figure("KAM013", "births", 1),
            figure("KAM020", "total-population", 1),
            figure("KAM020", "total-population", 2),
        ],
    };
    assert_eq!(
        wrong.problems(),
        vec![
            "the year 26 is not a year",
            "the estimate type total-population is declared twice",
            "the estimate type a is derived from itself, through the types it is derived from",
            "the estimate type b is derived from itself, through the types it is derived from",
            "the estimate type c needs a percentage of total-population above 0 and at most 100",
            "the estimate type d is derived from nothing, which is not declared",
            "the estimate type d needs a percentage of nothing above 0 and at most 100",
            "the estimate type e has a percentage but is derived from no type",
            "KAM013 has a figure for c, which is derived, not given",
            "KAM013 has a figure for births, which is not a declared estimate type",
            "KAM020 has two figures for total-population",
        ]
    );
    assert_eq!(
        EstimatesDefinition::default().problems(),
        vec![
            "the year 0 is not a year",
            "the estimates declare no estimate type"
        ]
    );
}

#[test]
fn the_file_reads_and_writes_as_json() {
    let written = r#"{"year":2026,"types":[{"code":"total-population","title":"Total population"},{"code":"expected-pregnancies","title":"Expected pregnancies","derivedFrom":"total-population","percent":"5"}],"figures":[{"place":"district-kampala","type":"total-population","value":1797722}]}"#;
    let read: EstimatesDefinition = serde_json::from_str(written).expect("reads");
    assert_eq!(
        read.figures[0],
        figure("district-kampala", "total-population", 1_797_722)
    );
    assert_eq!(serde_json::to_string(&read).expect("writes"), written);
}

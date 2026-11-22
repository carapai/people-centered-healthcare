//! Date conditions relative to the period and membership in another indicator (#550), over the
//! ANC reference package's test patients and over records made for each condition.
//!
//! The expressions are what the condition tree writes
//! (`apps/client/src/features/studio/indicators/conditions.ts`, whose tests assert the same
//! text).

use std::path::{Path, PathBuf};

use health_analytics::{Evaluation, Period, PeriodType};
use health_analytics_app::{
    Library, PatientRecords, evaluate, is_test_patient, measure, test_patient,
};
use health_fhir_validation::OctofhirEngine;
use serde_json::{Value, json};

fn anc() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../packages/reference-packages/anc")
}

fn read(path: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(path).expect("read")).expect("json")
}

fn anc_measure(name: &str) -> Value {
    read(&anc().join(format!("Measure-{name}.json")))
}

fn patients() -> Vec<PatientRecords> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(anc().join("tests"))
        .expect("tests")
        .map(|e| e.expect("entry").path())
        .collect();
    files.sort();
    files
        .iter()
        .map(|path| read(path))
        .filter(is_test_patient)
        .map(|file| test_patient(&file).expect("a test patient").records)
        .collect()
}

/// MoH's Age Band Sets, from the national core reference package (#551).
fn moh_age_bands() -> Vec<health_analytics::AgeBandSet> {
    let core =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../packages/reference-packages/core");
    [
        "Library-age-bands-anc.json",
        "Library-age-bands-outpatient.json",
    ]
    .iter()
    .map(|name| {
        health_analytics_app::age_band_set(&read(&core.join(name)))
            .age_band_set()
            .expect("a set")
    })
    .collect()
}

/// Every ANC indicator, as *Try* and the edge take them, with MoH's Age Band Sets.
fn library() -> Library {
    Library::new(
        std::fs::read_dir(anc())
            .expect("anc")
            .map(|e| e.expect("entry").path())
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with("Measure-"))
            })
            .map(|p| measure(&read(&p)).indicator().expect("indicator")),
    )
    .with_age_band_sets(moh_age_bands())
}

/// A count of people with an Encounter in the period meeting `criteria`.
fn counting_people(url: &str, criteria: &str) -> Value {
    json!({
        "resourceType": "Measure",
        "url": url,
        "version": "1.0.0",
        "title": url,
        "extension": [
            {
                "url": "https://carapai.github.io/people-centered-healthcare/fhir/StructureDefinition/indicator-period-date",
                "valueExpression": { "language": "text/fhirpath", "expression": "Encounter.actualPeriod.start" }
            }
        ],
        "scoring": { "coding": [{ "code": "cohort" }] },
        "basis": "Encounter",
        "group": [{ "population": [{
            "code": { "coding": [{ "code": "initial-population" }] },
            "criteria": { "language": "text/fhirpath", "expression": criteria }
        }] }]
    })
}

async fn evaluated(
    resource: &Value,
    library: &Library,
    kind: PeriodType,
    id: &str,
    patients: &[PatientRecords],
) -> Result<Evaluation, String> {
    let indicator = measure(resource).indicator().expect("indicator");
    let period = Period::parse(kind, id).expect("period");
    evaluate(
        &OctofhirEngine::new(),
        library,
        &indicator,
        &period,
        patients,
    )
    .await
}

fn people(evaluation: &Evaluation) -> Vec<String> {
    evaluation.counted().map(|m| m.patient.clone()).collect()
}

const NEXT_APPOINTMENT: &str =
    "%records.entry.resource.ofType(Appointment).where(status = 'booked').start.first()";

#[tokio::test]
async fn next_appointment_within_4_weeks_after_the_period_end_matches_the_test_patients() {
    // By hand, March 2026, the women with a visit in March and their booked next visit: Akello
    // 8 April and Nakato 28 April (the 4th week's last day) are within 4 weeks after 31 March;
    // Auma's 15 April too. Namubiru's 6 May is 5 weeks after; Nansubuga's booked 30 March is
    // before the period's end (her cancelled 10 April does not count). The others have none.
    let criteria = format!(
        "%patient.gender = 'female' and ({NEXT_APPOINTMENT}.toDate() > %periodEnd.toDate() and {NEXT_APPOINTMENT}.toDate() <= %periodEnd.toDate() + 4 weeks)"
    );
    let count = evaluated(
        &counting_people("https://x/Measure/next", &criteria),
        &Library::default(),
        PeriodType::Monthly,
        "2026-03",
        &patients(),
    )
    .await
    .expect("evaluated");
    assert_eq!(
        people(&count),
        ["Patient/akello", "Patient/auma", "Patient/nakato"]
    );
    assert_eq!(count.errors().count(), 0);
}

/// One patient with one Encounter on 15 March 2026, whose `plannedEndDate` is `date`.
fn with_date(date: Option<&str>) -> Vec<PatientRecords> {
    let mut encounter = json!({
        "resourceType": "Encounter",
        "id": "visit",
        "status": "completed",
        "actualPeriod": { "start": "2026-03-15T10:00:00+03:00" }
    });
    if let Some(date) = date {
        encounter["plannedEndDate"] = json!(format!("{date}T08:00:00+03:00"));
    }
    vec![PatientRecords {
        patient: json!({ "resourceType": "Patient", "id": "p", "gender": "female" }),
        records: vec![encounter],
    }]
}

/// A criterion, the month it is evaluated over, and each date with whether it holds.
type Case<'a> = (String, &'a str, &'a [(&'a str, bool)]);

#[tokio::test]
async fn each_date_condition_holds_as_written() {
    let d = "Encounter.plannedEndDate.toDate()";
    let start = "%periodStart.toDate()";
    let end = "%periodEnd.toDate()";
    let cases: Vec<Case<'_>> = vec![
        // is in the reporting period
        (
            format!("({d} >= {start} and {d} <= {end})"),
            "2026-03",
            &[
                ("2026-03-01", true),
                ("2026-03-31", true),
                ("2026-02-28", false),
                ("2026-04-01", false),
            ],
        ),
        // is on the period's start
        (
            format!("{d} = {start}"),
            "2026-03",
            &[("2026-03-01", true), ("2026-03-02", false)],
        ),
        // is before the period's start
        (
            format!("{d} < {start}"),
            "2026-03",
            &[("2026-02-28", true), ("2026-03-01", false)],
        ),
        // is after the period's end
        (
            format!("{d} > {end}"),
            "2026-03",
            &[("2026-04-01", true), ("2026-03-31", false)],
        ),
        // is after a fixed date
        (
            format!("{d} > @2026-03-15"),
            "2026-03",
            &[("2026-03-16", true), ("2026-03-15", false)],
        ),
        // is between a fixed date and the period's end
        (
            format!("({d} >= @2026-03-10 and {d} <= {end})"),
            "2026-03",
            &[("2026-03-10", true), ("2026-03-09", false)],
        ),
        // is within 4 weeks after the period's end
        (
            format!("({d} > {end} and {d} <= {end} + 4 weeks)"),
            "2026-03",
            &[
                ("2026-04-28", true),
                ("2026-04-29", false),
                ("2026-03-31", false),
            ],
        ),
        // is within 3 months before the period's start
        (
            format!("({d} >= {start} - 3 months and {d} < {start})"),
            "2026-03",
            &[
                ("2025-12-01", true),
                ("2025-11-30", false),
                ("2026-03-01", false),
            ],
        ),
        // is within 10 days before the period's end
        (
            format!("({d} >= {end} - 10 days and {d} < {end})"),
            "2026-03",
            &[("2026-03-21", true), ("2026-03-20", false)],
        ),
    ];
    for (criteria, month, dates) in &cases {
        for (date, holds) in *dates {
            let count = evaluated(
                &counting_people("https://x/Measure/date", criteria),
                &Library::default(),
                PeriodType::Monthly,
                month,
                &with_date(Some(date)),
            )
            .await
            .expect("evaluated");
            assert_eq!(count.errors().count(), 0, "{criteria} on {date}");
            assert_eq!(count.summary.count == 1, *holds, "{criteria} on {date}");
        }
    }
}

#[tokio::test]
async fn in_the_previous_periods_follows_the_reports_period_type() {
    let d = "Encounter.plannedEndDate.toDate()";
    let previous = |n: usize| {
        format!(
            "({d} >= %previousPeriods.start[{}].toDate() and {d} < %periodStart.toDate())",
            n - 1
        )
    };
    let cases: [(usize, PeriodType, &str, &str, bool); 7] = [
        (3, PeriodType::Monthly, "2026-03", "2025-12-01", true),
        (3, PeriodType::Monthly, "2026-03", "2026-02-28", true),
        (3, PeriodType::Monthly, "2026-03", "2025-11-30", false),
        (3, PeriodType::Monthly, "2026-03", "2026-03-01", false),
        (2, PeriodType::Weekly, "2026W11", "2026-02-23", true),
        (2, PeriodType::Weekly, "2026W11", "2026-02-22", false),
        (2, PeriodType::Quarterly, "2026Q2", "2025-10-01", true),
    ];
    for (n, kind, period, date, holds) in cases {
        let mut patients = with_date(Some(date));
        // Place the visit in the period evaluated.
        let start = Period::parse(kind, period).expect("period").start;
        patients[0].records[0]["actualPeriod"]["start"] = json!(format!("{start}T10:00:00+03:00"));
        let count = evaluated(
            &counting_people("https://x/Measure/previous", &previous(n)),
            &Library::default(),
            kind,
            period,
            &patients,
        )
        .await
        .expect("evaluated");
        assert_eq!(count.errors().count(), 0, "{n} {period} {date}");
        assert_eq!(count.summary.count == 1, holds, "{n} {period} {date}");
    }
}

#[tokio::test]
async fn a_date_is_empty_or_not() {
    for (criteria, date, holds) in [
        ("Encounter.plannedEndDate.toDate().empty()", None, true),
        (
            "Encounter.plannedEndDate.toDate().empty()",
            Some("2026-03-02"),
            false,
        ),
        (
            "Encounter.plannedEndDate.toDate().exists()",
            Some("2026-03-02"),
            true,
        ),
        ("Encounter.plannedEndDate.toDate().exists()", None, false),
    ] {
        let count = evaluated(
            &counting_people("https://x/Measure/empty", criteria),
            &Library::default(),
            PeriodType::Monthly,
            "2026-03",
            &with_date(date),
        )
        .await
        .expect("evaluated");
        assert_eq!(count.summary.count == 1, holds, "{criteria} {date:?}");
    }
}

const ANC_FIRST_VISITS: &str = "https://health.go.ug/fhir/Measure/anc-first-visits|1.0.0";

#[tokio::test]
async fn is_in_anc_first_visits_numerator_gives_the_same_people_as_evaluating_it() {
    let member = counting_people(
        "https://x/Measure/member",
        &format!("'{ANC_FIRST_VISITS}' in %indicators.numerator"),
    );
    for (kind, id) in [
        (PeriodType::Monthly, "2026-03"),
        (PeriodType::Monthly, "2026-04"),
        (PeriodType::Quarterly, "2026Q1"),
        (PeriodType::Annual, "2026"),
    ] {
        let direct = evaluated(
            &anc_measure("anc-first-visits"),
            &Library::default().with_age_band_sets(moh_age_bands()),
            kind,
            id,
            &patients(),
        )
        .await
        .expect("evaluated");
        let through = evaluated(&member, &library(), kind, id, &patients())
            .await
            .expect("evaluated");
        assert!(!people(&direct).is_empty(), "{id}");
        assert_eq!(people(&through), people(&direct), "{id}");
    }
}

#[tokio::test]
async fn is_in_a_denominator_reads_that_side() {
    // *ANC 1st visit before 12 weeks*' denominator in March is the four first visits; its
    // numerator leaves out Akello (16 weeks).
    let side = |side: &str| {
        counting_people(
            "https://x/Measure/side",
            &format!(
                "'https://health.go.ug/fhir/Measure/anc-first-visit-before-12-weeks|1.0.0' in %indicators.{side}"
            ),
        )
    };
    let denominator = evaluated(
        &side("denominator"),
        &library(),
        PeriodType::Monthly,
        "2026-03",
        &patients(),
    )
    .await
    .expect("evaluated");
    assert_eq!(
        people(&denominator),
        [
            "Patient/akello",
            "Patient/nakato",
            "Patient/namubiru",
            "Patient/nansubuga"
        ]
    );
    let numerator = evaluated(
        &side("numerator"),
        &library(),
        PeriodType::Monthly,
        "2026-03",
        &patients(),
    )
    .await
    .expect("evaluated");
    assert_eq!(
        people(&numerator),
        ["Patient/nakato", "Patient/namubiru", "Patient/nansubuga"]
    );
}

#[tokio::test]
async fn the_reference_indicator_counts_first_visits_with_the_next_visit_booked() {
    // Of March's four first visits, Akello and Nakato have their next visit booked within 4
    // weeks after 31 March: 2 of 4.
    let count = evaluated(
        &anc_measure("anc-first-visit-next-visit-booked"),
        &library(),
        PeriodType::Monthly,
        "2026-03",
        &patients(),
    )
    .await
    .expect("evaluated");
    assert_eq!(count.summary.count, 2);
    assert_eq!(count.summary.denominator, Some(4));
    assert_eq!(count.summary.value, Some(50.into()));
    assert_eq!(people(&count), ["Patient/akello", "Patient/nakato"]);
}

#[tokio::test]
async fn a_missing_indicator_a_circle_or_a_count_s_denominator_is_an_error() {
    let missing = counting_people(
        "https://x/Measure/a",
        "'https://x/Measure/gone|1.0.0' in %indicators.numerator",
    );
    let error = evaluated(
        &missing,
        &library(),
        PeriodType::Monthly,
        "2026-03",
        &patients(),
    )
    .await
    .expect_err("missing");
    assert!(error.contains("https://x/Measure/gone|1.0.0"), "{error}");
    assert!(error.contains("not available"), "{error}");

    let a = counting_people(
        "https://x/Measure/a",
        "'https://x/Measure/b|1.0.0' in %indicators.numerator",
    );
    let b = counting_people(
        "https://x/Measure/b",
        "'https://x/Measure/a|1.0.0' in %indicators.numerator",
    );
    let circle = Library::new([&a, &b].map(|m| measure(m).indicator().expect("indicator")));
    let error = evaluated(&a, &circle, PeriodType::Monthly, "2026-03", &patients())
        .await
        .expect_err("circle");
    assert_eq!(
        error,
        "the indicators refer to each other in a circle: https://x/Measure/a|1.0.0 → https://x/Measure/b|1.0.0 → https://x/Measure/a|1.0.0"
    );

    let denominator = counting_people(
        "https://x/Measure/c",
        &format!("'{ANC_FIRST_VISITS}' in %indicators.denominator"),
    );
    let error = evaluated(
        &denominator,
        &library(),
        PeriodType::Monthly,
        "2026-03",
        &patients(),
    )
    .await
    .expect_err("a count has no denominator");
    assert!(error.ends_with("which has none"), "{error}");
}

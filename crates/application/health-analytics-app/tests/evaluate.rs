//! The ANC reference package's indicators over its test patients (#548, #549), against hand
//! calculations.

use std::path::{Path, PathBuf};

use health_analytics::{Calculation, Evaluation, Period, PeriodType, Reason, Values};
use health_analytics_app::{PatientRecords, evaluate, is_test_patient, measure, test_patient};
use health_fhir_validation::OctofhirEngine;
use rust_decimal::Decimal;
use serde_json::{Value, json};

fn anc() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../packages/reference-packages/anc")
}

fn read(path: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(path).expect("read")).expect("json")
}

fn anc_first_visits() -> Value {
    read(&anc().join("Measure-anc-first-visits.json"))
}

fn anc_measure(name: &str) -> Value {
    read(&anc().join(format!("Measure-{name}.json")))
}

async fn evaluated(resource: &Value, kind: PeriodType, id: &str) -> Evaluation {
    let indicator = measure(resource).indicator().expect("indicator");
    let period = Period::parse(kind, id).expect("period");
    evaluate(&OctofhirEngine::new(), &indicator, &period, &patients()).await
}

fn people(evaluation: &Evaluation) -> Vec<&str> {
    evaluation.counted().map(|m| m.patient.as_str()).collect()
}

fn number(text: &str) -> Option<Decimal> {
    Some(text.parse().expect("decimal"))
}

/// The package's test patients, in file order.
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

async fn counted(kind: PeriodType, id: &str) -> (u64, Vec<String>) {
    let count = evaluated(&anc_first_visits(), kind, id).await;
    let people = count.counted().map(|m| m.patient.clone()).collect();
    (count.summary.count, people)
}

#[tokio::test]
async fn anc_first_visits_in_march_match_the_hand_count() {
    // By hand, March 2026: Akello (11 March), Nakato (4 March; her follow-up on 28 March is not
    // a first visit), Namubiru (31 March at 22:30 local, entered in April) and Nansubuga
    // (recorded twice, counted once). Not Auma (first visit in January), Okello (OPD), Nabirye
    // (April), Atim (February), Opio (a man) or Kisakye (planned, not started).
    assert_eq!(
        counted(PeriodType::Monthly, "2026-03").await,
        (
            4,
            vec![
                "Patient/akello".to_owned(),
                "Patient/nakato".to_owned(),
                "Patient/namubiru".to_owned(),
                "Patient/nansubuga".to_owned(),
            ]
        )
    );
}

#[tokio::test]
async fn the_same_indicator_counts_any_period_a_report_chooses() {
    assert_eq!(
        counted(PeriodType::Monthly, "2026-04").await,
        (1, vec!["Patient/nabirye".to_owned()])
    );
    let (q1, people) = counted(PeriodType::Quarterly, "2026Q1").await;
    assert_eq!(q1, 6, "{people:?}");
    assert!(people.contains(&"Patient/auma".to_owned()));
    assert!(people.contains(&"Patient/atim".to_owned()));
    // Week 14 of 2026 is 30 March to 5 April: only Namubiru's first visit falls in it.
    assert_eq!(
        counted(PeriodType::Weekly, "2026W14").await,
        (1, vec!["Patient/namubiru".to_owned()])
    );
}

#[tokio::test]
async fn each_patient_says_why_they_are_counted_or_not() {
    let indicator = measure(&anc_first_visits()).indicator().expect("indicator");
    let march = Period::parse(PeriodType::Monthly, "2026-03").expect("march");
    let count = evaluate(&OctofhirEngine::new(), &indicator, &march, &patients()).await;
    let member = |patient: &str| {
        count
            .members
            .iter()
            .find(|m| m.patient == patient)
            .expect(patient)
    };
    let reasons = |patient: &str| -> Vec<(String, Reason)> {
        member(patient)
            .records
            .iter()
            .map(|r| (r.reference.clone(), r.reason.clone()))
            .collect()
    };
    assert_eq!(
        reasons("Patient/nakato"),
        [
            ("Encounter/nakato-visit-1".to_owned(), Reason::Meets),
            ("Encounter/nakato-visit-2".to_owned(), Reason::DoesNotMeet),
        ]
    );
    assert_eq!(
        reasons("Patient/auma"),
        [
            ("Encounter/auma-visit-1".to_owned(), Reason::OutsidePeriod),
            ("Encounter/auma-visit-2".to_owned(), Reason::OutsidePeriod),
            ("Encounter/auma-visit-3".to_owned(), Reason::DoesNotMeet),
        ]
    );
    assert_eq!(
        reasons("Patient/opio"),
        [("Encounter/opio-visit-1".to_owned(), Reason::DoesNotMeet)]
    );
    assert_eq!(
        reasons("Patient/kisakye"),
        [("Encounter/kisakye-visit-1".to_owned(), Reason::NoPeriodDate)]
    );
    let namubiru = &member("Patient/namubiru").records[0];
    assert_eq!(namubiru.date, Some("2026-03-31".parse().expect("date")));
    assert_eq!(count.members.len(), 11);
    assert_eq!(count.errors().count(), 0);
}

#[tokio::test]
async fn the_period_and_the_patient_are_variables_of_the_criteria() {
    let mut resource = anc_first_visits();
    resource["group"][0]["population"][0]["criteria"]["expression"] = json!(
        "%patient.gender = 'female' and Encounter.actualPeriod.start.toString().substring(0, 10) = %periodEnd"
    );
    let indicator = measure(&resource).indicator().expect("indicator");
    let march = Period::parse(PeriodType::Monthly, "2026-03").expect("march");
    let count = evaluate(&OctofhirEngine::new(), &indicator, &march, &patients()).await;
    let people: Vec<&str> = count.counted().map(|m| m.patient.as_str()).collect();
    assert_eq!(people, ["Patient/namubiru"]);
}

#[tokio::test]
async fn criteria_that_cannot_be_evaluated_are_reported_per_record() {
    let mut resource = anc_first_visits();
    resource["group"][0]["population"][0]["criteria"]["expression"] =
        json!("Encounter.type.coding.code");
    let indicator = measure(&resource).indicator().expect("indicator");
    let march = Period::parse(PeriodType::Monthly, "2026-03").expect("march");
    let count = evaluate(&OctofhirEngine::new(), &indicator, &march, &patients()).await;
    assert_eq!(count.summary.count, 0);
    let errors: Vec<(&str, &str)> = count.errors().collect();
    assert!(
        errors
            .iter()
            .any(|(record, why)| *record == "Encounter/akello-visit-1"
                && why.contains("not true or false")),
        "{errors:?}"
    );
    let mut resource = anc_first_visits();
    resource["extension"][0]["valueExpression"]["expression"] = json!("Encounter.status");
    let indicator = measure(&resource).indicator().expect("indicator");
    let count = evaluate(&OctofhirEngine::new(), &indicator, &march, &patients()).await;
    assert_eq!(count.summary.count, 0, "a status is no date");
}

#[test]
fn the_measure_reads_as_written() {
    let definition = measure(&anc_first_visits());
    assert_eq!(definition.title.as_deref(), Some("ANC 1st visits"));
    assert_eq!(definition.scoring.as_deref(), Some("cohort"));
    assert_eq!(definition.basis.as_deref(), Some("Encounter"));
    assert_eq!(definition.counts.as_deref(), Some("people"));
    assert_eq!(definition.groups, 1);
    assert!(
        definition.problems().is_empty(),
        "{:?}",
        definition.problems()
    );
    let mut resource = anc_first_visits();
    resource["extension"] = json!([]);
    assert_eq!(
        measure(&resource).problems(),
        [
            "the indicator has no period date: the declared date that places care in a Report's period"
        ]
    );
}

#[test]
fn a_test_patient_needs_a_name_and_one_patient() {
    let patient = json!({ "resourceType": "Patient", "id": "a", "name": [{ "given": ["Ann"], "family": "Apio" }] });
    let file = json!({ "type": "test-patient", "name": "Ann", "resources": [patient] });
    let read = test_patient(&file).expect("test patient");
    assert_eq!(read.records.reference(), "Patient/a");
    assert_eq!(read.records.name().as_deref(), Some("Ann Apio"));
    assert!(read.records.records.is_empty());
    for (file, why) in [
        (
            json!({ "type": "test-patient", "resources": [] }),
            "no name",
        ),
        (
            json!({ "type": "test-patient", "name": "x" }),
            "no resources",
        ),
        (
            json!({ "type": "test-patient", "name": "x", "resources": [] }),
            "needs one Patient, and it has 0",
        ),
        (
            json!({ "type": "test-patient", "name": "x", "resources": [{ "resourceType": "Encounter" }] }),
            "needs a resourceType and an id",
        ),
    ] {
        let error = test_patient(&file).expect_err(why);
        assert!(error.contains(why), "{error}");
    }
    let text = json!({ "resourceType": "Patient", "id": "b", "name": [{ "text": "Bosco O." }] });
    let file = json!({ "type": "test-patient", "name": "B", "resources": [text] });
    assert_eq!(
        test_patient(&file).expect("b").records.name().as_deref(),
        Some("Bosco O.")
    );
}

#[tokio::test]
async fn anc_first_visit_before_12_weeks_matches_the_hand_calculation() {
    // By hand, March 2026: the denominator is the four women with a first visit (Akello,
    // Nakato, Namubiru, Nansubuga). Their gestational ages at it were 16, 10, 9, and 8 (then 11
    // at the duplicate), so Nakato, Namubiru and Nansubuga are the numerator: 3 of 4, 75%.
    let resource = anc_measure("anc-first-visit-before-12-weeks");
    let march = evaluated(&resource, PeriodType::Monthly, "2026-03").await;
    assert_eq!(march.errors().collect::<Vec<_>>(), []);
    assert_eq!(march.summary.calculation, Calculation::Percentage);
    assert_eq!(
        (march.summary.count, march.summary.denominator),
        (3, Some(4))
    );
    assert_eq!(march.summary.value, number("75"));
    assert_eq!(
        people(&march),
        ["Patient/nakato", "Patient/namubiru", "Patient/nansubuga"]
    );
    let akello = march
        .members
        .iter()
        .find(|m| m.patient == "Patient/akello")
        .expect("akello");
    assert_eq!(akello.records[0].reason, Reason::DenominatorOnly);
    // Q1 adds Auma (12 weeks: not under 12) and Atim (13) to the denominator: 3 of 6, 50%.
    let q1 = evaluated(&resource, PeriodType::Quarterly, "2026Q1").await;
    assert_eq!((q1.summary.count, q1.summary.denominator), (3, Some(6)));
    assert_eq!(q1.summary.value, number("50"));
    // As a rate per 1,000, and as a ratio.
    let mut rate = resource.clone();
    rate["scoring"]["coding"][0]["code"] = json!("ratio");
    rate["extension"].as_array_mut().expect("extensions").push(json!({
        "url": "https://carapai.github.io/people-centered-healthcare/fhir/StructureDefinition/indicator-rate-per",
        "valueInteger": 1000
    }));
    let per_thousand = evaluated(&rate, PeriodType::Monthly, "2026-03").await;
    assert_eq!(
        per_thousand.summary.calculation,
        Calculation::Rate { per: 1000 }
    );
    assert_eq!(per_thousand.summary.value, number("750"));
    rate["extension"].as_array_mut().expect("extensions").pop();
    let ratio = evaluated(&rate, PeriodType::Monthly, "2026-03").await;
    assert_eq!(ratio.summary.value, number("0.75"));
}

#[tokio::test]
async fn counting_enrollments_or_visits_instead_of_people_changes_the_count() {
    let mut resource = anc_measure("anc-visits");
    let count = async |resource: &Value, counts: &str, kind, id| {
        let mut resource = resource.clone();
        resource["extension"][1]["valueCode"] = json!(counts);
        evaluated(&resource, kind, id).await.summary.count
    };
    // March: Akello 1, Auma 1 (a follow-up), Nakato 2 (first and follow-up), Namubiru 1 and
    // Nansubuga 2 (recorded twice): 7 visits by 5 women in 5 pregnancies. Opio is a man.
    assert_eq!(
        count(&resource, "visits", PeriodType::Monthly, "2026-03").await,
        7
    );
    assert_eq!(
        count(&resource, "people", PeriodType::Monthly, "2026-03").await,
        5
    );
    assert_eq!(
        count(&resource, "enrollments", PeriodType::Monthly, "2026-03").await,
        5
    );
    // January to June: Auma 3, Atim 1, Akello 1, Nakato 2, Namubiru 1, Nansubuga 2, Nabirye 1
    // and Achan 2 (a follow-up in February for one pregnancy, a first visit in June for the
    // next): 13 visits by 8 women in 9 pregnancies.
    assert_eq!(
        count(&resource, "visits", PeriodType::SemiAnnual, "2026S1").await,
        13
    );
    assert_eq!(
        count(&resource, "people", PeriodType::SemiAnnual, "2026S1").await,
        8
    );
    assert_eq!(
        count(&resource, "enrollments", PeriodType::SemiAnnual, "2026S1").await,
        9
    );
    // Cumulative: women with an ANC visit from the first record up to the end of March,
    // Achan's from November 2025 included.
    resource["extension"].as_array_mut().expect("extensions").push(json!({
        "url": "https://carapai.github.io/people-centered-healthcare/fhir/StructureDefinition/indicator-cumulative",
        "valueBoolean": true
    }));
    assert_eq!(
        count(&resource, "people", PeriodType::Monthly, "2026-03").await,
        7
    );
}

#[tokio::test]
async fn the_median_gestational_age_at_first_visit_matches_the_hand_calculation() {
    // By hand, March 2026, each woman's first value: Akello 16, Nakato 10, Namubiru 9 and
    // Nansubuga 8 (her duplicate record two days later says 11). Sorted 8, 9, 10, 16: the median
    // is 9.5. Taking the last value, Nansubuga's is 11: 9, 10, 11, 16, median 10.5.
    let mut resource = anc_measure("anc-gestational-age-at-first-visit");
    let march = evaluated(&resource, PeriodType::Monthly, "2026-03").await;
    assert_eq!(march.errors().collect::<Vec<_>>(), []);
    assert_eq!(march.summary.value, number("9.5"));
    assert_eq!(march.summary.count, 4);
    assert_eq!(
        march.summary.values,
        Some(Values {
            count: 4,
            sum: number("43"),
            minimum: number("8"),
            maximum: number("16"),
        })
    );
    // Q1 adds Auma (12) and Atim (13): 8, 9, 10, 12, 13, 16, median 11.
    let q1 = evaluated(&resource, PeriodType::Quarterly, "2026Q1").await;
    assert_eq!(q1.summary.value, number("11"));
    let observation = &mut resource["group"][0]["population"][2];
    observation["extension"][0]["valueCode"] = json!("last");
    let last = evaluated(&resource, PeriodType::Monthly, "2026-03").await;
    assert_eq!(last.summary.value, number("10.5"));
    let nansubuga = last
        .members
        .iter()
        .find(|m| m.patient == "Patient/nansubuga")
        .expect("nansubuga");
    let picked: Vec<bool> = nansubuga.records.iter().map(|r| r.picked).collect();
    assert_eq!(picked, [false, true]);
    // The average, sum, minimum and maximum of the same values.
    for (aggregate, value) in [
        ("average", "10.75"),
        ("sum", "43"),
        ("minimum", "8"),
        ("maximum", "16"),
    ] {
        resource["group"][0]["population"][2]["extension"][0]["valueCode"] = json!("first");
        resource["group"][0]["population"][2]["aggregateMethod"]["coding"][0]["code"] =
            json!(aggregate);
        let summary = evaluated(&resource, PeriodType::Monthly, "2026-03")
            .await
            .summary;
        assert_eq!(summary.value, number(value), "{aggregate}");
    }
}

#[tokio::test]
async fn a_value_can_be_an_age_or_a_distinct_count() {
    // Age at the first visit, in whole years: Akello 17, Nakato 23, Namubiru 31, Nansubuga 26.
    let mut resource = anc_measure("anc-gestational-age-at-first-visit");
    let observation = &mut resource["group"][0]["population"][2];
    observation
        .as_object_mut()
        .expect("population")
        .remove("criteria");
    observation["aggregateMethod"]["coding"][0]["code"] = json!("minimum");
    observation["extension"].as_array_mut().expect("extensions").push(json!({
        "url": "https://carapai.github.io/people-centered-healthcare/fhir/StructureDefinition/indicator-value-between",
        "extension": [
            { "url": "from", "valueExpression": { "language": "text/fhirpath", "expression": "%patient.birthDate" } },
            { "url": "to", "valueExpression": { "language": "text/fhirpath", "expression": "Encounter.actualPeriod.start" } },
            { "url": "unit", "valueCode": "years" }
        ]
    }));
    let ages = evaluated(&resource, PeriodType::Monthly, "2026-03").await;
    assert_eq!(ages.errors().collect::<Vec<_>>(), []);
    assert_eq!(ages.summary.value, number("17"));
    assert_eq!(
        ages.summary.values.as_ref().and_then(|v| v.maximum),
        number("31")
    );
    // How many visit types the March ANC visits had: first and follow-up.
    let mut distinct = anc_measure("anc-gestational-age-at-first-visit");
    distinct["extension"][1]["valueCode"] = json!("visits");
    let population = &mut distinct["group"][0]["population"];
    population[1]["criteria"]["expression"] = json!(
        "Encounter.type.coding.where(system = 'https://health.go.ug/fhir/CodeSystem/anc-visit-type').exists() and %patient.gender = 'female'"
    );
    population[2]["criteria"]["expression"] = json!("Encounter.type.coding.code.first()");
    population[2]["aggregateMethod"]["coding"][0]["code"] = json!("distinct-count");
    let types = evaluated(&distinct, PeriodType::Monthly, "2026-03").await;
    assert_eq!(types.summary.count, 7);
    assert_eq!(types.summary.value, number("2"));
}

#[test]
fn the_reference_indicators_read_as_written() {
    for name in [
        "anc-first-visits",
        "anc-first-visit-before-12-weeks",
        "anc-gestational-age-at-first-visit",
        "anc-visits",
    ] {
        let problems = measure(&anc_measure(name)).problems();
        assert!(problems.is_empty(), "{name}: {problems:?}");
    }
}

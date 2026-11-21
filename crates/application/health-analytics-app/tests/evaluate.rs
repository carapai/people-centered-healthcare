//! *ANC 1st visits* over the ANC reference package's test patients (#548), against hand counts.

use std::path::{Path, PathBuf};

use health_analytics::{Period, PeriodType, Reason, Scoring};
use health_analytics_app::{
    EvaluateError, PatientRecords, evaluate, is_test_patient, measure, test_patient,
};
use health_fhir_validation::OctofhirEngine;
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
    let indicator = measure(&anc_first_visits()).indicator().expect("indicator");
    let period = Period::parse(kind, id).expect("period");
    let count = evaluate(&OctofhirEngine::new(), &indicator, &period, &patients())
        .await
        .expect("evaluated");
    let people = count.counted().map(|m| m.patient.clone()).collect();
    (count.value, people)
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
    let count = evaluate(&OctofhirEngine::new(), &indicator, &march, &patients())
        .await
        .expect("evaluated");
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
    assert_eq!(count.members.len(), 10);
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
    let count = evaluate(&OctofhirEngine::new(), &indicator, &march, &patients())
        .await
        .expect("evaluated");
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
    let count = evaluate(&OctofhirEngine::new(), &indicator, &march, &patients())
        .await
        .expect("evaluated");
    assert_eq!(count.value, 0);
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
    let count = evaluate(&OctofhirEngine::new(), &indicator, &march, &patients())
        .await
        .expect("evaluated");
    assert_eq!(count.value, 0, "a status is no date");
}

#[tokio::test]
async fn other_scorings_and_counts_wait_for_their_calculations() {
    let mut definition = measure(&anc_first_visits());
    let march = Period::parse(PeriodType::Monthly, "2026-03").expect("march");
    let engine = OctofhirEngine::new();
    definition.counts = Some("visits".into());
    let indicator = definition.indicator().expect("indicator");
    let error = evaluate(&engine, &indicator, &march, &[])
        .await
        .expect_err("visits");
    assert_eq!(error.to_string(), "counting visits is not supported yet");
    let mut indicator = indicator;
    indicator.scoring = Scoring::Proportion;
    assert_eq!(
        evaluate(&engine, &indicator, &march, &[]).await,
        Err(EvaluateError::Scoring(Scoring::Proportion))
    );
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

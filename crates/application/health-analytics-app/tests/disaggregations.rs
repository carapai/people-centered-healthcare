//! Disaggregations (#552): an indicator broken down by MoH's ANC age bands or by a coded value,
//! over the ANC reference package's test patients and over records made for each case. Every
//! disaggregation has a *not recorded* band, so the bands add up to the total.

use std::path::{Path, PathBuf};

use health_analytics::{AgeBandSet, Evaluation, NOT_RECORDED, Period, PeriodType, Summary};
use health_analytics_app::{
    AGE_BAND_SET, Library, PatientRecords, age_band_set, evaluate, is_test_patient, measure,
    test_patient,
};
use health_fhir_validation::OctofhirEngine;
use rust_decimal::Decimal;
use serde_json::{Value, json};

const ANC_BANDS: &str = "https://health.go.ug/fhir/Library/age-bands-anc|1.0.0";

fn reference(package: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../packages/reference-packages")
        .join(package)
}

fn read(path: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(path).expect("read")).expect("json")
}

fn anc_measure(name: &str) -> Value {
    read(&reference("anc").join(format!("Measure-{name}.json")))
}

/// MoH's ANC age bands, from the national core reference package.
fn moh_anc_bands() -> AgeBandSet {
    age_band_set(&read(&reference("core").join("Library-age-bands-anc.json")))
        .age_band_set()
        .expect("a set")
}

fn anc_patients() -> Vec<PatientRecords> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(reference("anc").join("tests"))
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

async fn evaluated(
    resource: &Value,
    kind: PeriodType,
    id: &str,
    patients: &[PatientRecords],
) -> Result<Evaluation, String> {
    let indicator = measure(resource).indicator().expect("indicator");
    let period = Period::parse(kind, id).expect("period");
    let library = Library::default().with_age_band_sets([moh_anc_bands()]);
    evaluate(
        &OctofhirEngine::new(),
        &library,
        &indicator,
        &period,
        patients,
    )
    .await
}

/// Each band of the disaggregation at `at`: its code, count, denominator and value.
fn bands(summary: &Summary, at: usize) -> Vec<(String, u64, Option<u64>, Option<Decimal>)> {
    summary.disaggregations[at]
        .bands
        .iter()
        .map(|b| {
            (
                b.code.clone(),
                b.summary.count,
                b.summary.denominator,
                b.summary.value,
            )
        })
        .collect()
}

fn band(
    code: &str,
    count: u64,
    denominator: Option<u64>,
    value: i64,
) -> (String, u64, Option<u64>, Option<Decimal>) {
    (
        code.to_owned(),
        count,
        denominator,
        Some(Decimal::from(value)),
    )
}

#[tokio::test]
async fn anc_first_visits_by_moh_anc_age_bands_add_up_to_the_total() {
    let resource = anc_measure("anc-first-visits");
    // By hand, March 2026, age in whole years on 31 March: Akello (born 15 June 2008) 17,
    // Nakato (2 November 2002) 23, Namubiru (30 September 1994) 31 and Nansubuga (3 July 1999)
    // 26.
    let march = evaluated(&resource, PeriodType::Monthly, "2026-03", &anc_patients())
        .await
        .expect("evaluated");
    assert_eq!(march.summary.disaggregations[0].name, ANC_BANDS);
    assert_eq!(
        bands(&march.summary, 0),
        [
            band("10-14", 0, None, 0),
            band("15-19", 1, None, 1),
            band("20-24", 1, None, 1),
            band("25+", 2, None, 2),
            band(NOT_RECORDED, 0, None, 0),
        ]
    );
    for (kind, id) in [
        (PeriodType::Monthly, "2026-03"),
        (PeriodType::Monthly, "2026-04"),
        (PeriodType::Quarterly, "2026Q1"),
        (PeriodType::Quarterly, "2026Q2"),
        (PeriodType::Annual, "2026"),
    ] {
        let evaluation = evaluated(&resource, kind, id, &anc_patients())
            .await
            .expect("evaluated");
        let parts: u64 = evaluation.summary.disaggregations[0]
            .bands
            .iter()
            .map(|b| b.summary.count)
            .sum();
        assert!(evaluation.summary.count > 0, "{id}");
        assert_eq!(parts, evaluation.summary.count, "{id}");
    }
    // A birth date missing is not recorded; still the bands add up.
    let mut patients = anc_patients();
    for patient in &mut patients {
        if patient.reference() == "Patient/nakato" {
            patient.patient["birthDate"] = Value::Null;
        }
    }
    let march = evaluated(&resource, PeriodType::Monthly, "2026-03", &patients)
        .await
        .expect("evaluated");
    assert_eq!(
        bands(&march.summary, 0)[2..],
        [
            band("20-24", 0, None, 0),
            band("25+", 2, None, 2),
            band(NOT_RECORDED, 1, None, 1),
        ]
    );
}

#[tokio::test]
async fn a_pinned_set_that_is_not_published_cannot_be_disaggregated_by() {
    let mut resource = anc_measure("anc-first-visits");
    resource["group"][0]["stratifier"][0]["extension"][0]["valueCanonical"] =
        json!("https://health.go.ug/fhir/Library/age-bands-anc|9.0.0");
    assert_eq!(
        evaluated(&resource, PeriodType::Monthly, "2026-03", &anc_patients())
            .await
            .err()
            .as_deref(),
        Some(
            "https://health.go.ug/fhir/Measure/anc-first-visits|1.0.0 cannot be disaggregated: no published Age Band Set is https://health.go.ug/fhir/Library/age-bands-anc|9.0.0"
        )
    );
}

/// A woman with a first ANC visit on 10 March 2026 and, if given, her HIV status that day.
fn woman(id: &str, status: Option<&str>) -> PatientRecords {
    let mut records = vec![json!({
        "resourceType": "Encounter", "id": format!("{id}-visit"), "status": "completed",
        "type": [{ "coding": [{ "system": "https://health.go.ug/fhir/CodeSystem/anc-visit-type", "code": "first" }] }],
        "subject": { "reference": format!("Patient/{id}") },
        "actualPeriod": { "start": "2026-03-10T09:00:00+03:00" }
    })];
    if let Some(status) = status {
        records.push(json!({
            "resourceType": "Observation", "id": format!("{id}-hiv"), "status": "final",
            "code": { "coding": [{ "system": "http://loinc.org", "code": "55277-8" }] },
            "subject": { "reference": format!("Patient/{id}") },
            "effectiveDateTime": "2026-03-10T09:00:00+03:00",
            "valueCodeableConcept": { "coding": [{ "system": "https://health.go.ug/fhir/CodeSystem/hiv-status", "code": status }] }
        }));
    }
    PatientRecords {
        patient: json!({ "resourceType": "Patient", "id": id, "gender": "female", "birthDate": "2000-01-01" }),
        records,
    }
}

/// ANC 1st visits disaggregated by HIV status, the code of the patient's HIV status
/// Observation.
fn by_hiv_status(mut resource: Value) -> Value {
    resource["group"][0]["stratifier"] = json!([{
        "code": { "coding": [{ "code": "hiv-status" }], "text": "HIV status" },
        "criteria": {
            "language": "text/fhirpath",
            "expression": "%records.entry.resource.ofType(Observation).where(code.coding.code = '55277-8').value.coding.code.first()"
        }
    }]);
    resource
}

#[tokio::test]
async fn a_patient_with_no_hiv_status_lands_in_not_recorded() {
    let resource = by_hiv_status(anc_measure("anc-first-visits"));
    let patients = [
        woman("a", Some("positive")),
        woman("b", Some("negative")),
        woman("c", Some("negative")),
        woman("d", None),
    ];
    let evaluation = evaluated(&resource, PeriodType::Monthly, "2026-03", &patients)
        .await
        .expect("evaluated");
    assert_eq!(evaluation.errors().count(), 0);
    assert_eq!(evaluation.summary.count, 4);
    assert_eq!(evaluation.summary.disaggregations[0].name, "hiv-status");
    assert_eq!(
        bands(&evaluation.summary, 0),
        [
            band("negative", 2, None, 2),
            band("positive", 1, None, 1),
            band(NOT_RECORDED, 1, None, 1),
        ]
    );
    let d = &evaluation.members[3];
    assert_eq!(d.records[0].strata, [Ok(None)]);
    assert_eq!(
        evaluation.members[0].records[0].strata,
        [Ok(Some("positive".to_owned()))]
    );
}

#[tokio::test]
async fn percentages_per_band_use_the_denominator_within_that_band() {
    // ANC 1st visit before 12 weeks, by MoH's ANC age bands: in March, Akello (15–19) is in
    // the denominator only, and Nakato (20–24), Namubiru and Nansubuga (25+) are in both.
    let mut resource = anc_measure("anc-first-visit-before-12-weeks");
    resource["group"][0]["stratifier"] = json!([{
        "extension": [{ "url": AGE_BAND_SET, "valueCanonical": ANC_BANDS }]
    }]);
    let march = evaluated(&resource, PeriodType::Monthly, "2026-03", &anc_patients())
        .await
        .expect("evaluated");
    assert_eq!(
        (march.summary.count, march.summary.denominator),
        (3, Some(4))
    );
    assert_eq!(
        bands(&march.summary, 0),
        [
            ("10-14".to_owned(), 0, Some(0), None),
            band("15-19", 0, Some(1), 0),
            band("20-24", 1, Some(1), 100),
            band("25+", 2, Some(2), 100),
            (NOT_RECORDED.to_owned(), 0, Some(0), None),
        ]
    );
    // Rolled up from two facilities' summaries, each band from its own summed parts.
    let rolled = Summary::roll_up(
        march.summary.calculation,
        &[march.summary.clone(), march.summary.clone()],
    )
    .expect("rolled up");
    assert_eq!(bands(&rolled, 0)[1], band("15-19", 0, Some(2), 0));
    assert_eq!(bands(&rolled, 0)[3], band("25+", 4, Some(4), 100));
}

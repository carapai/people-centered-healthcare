use health_analytics::PeriodType;
use serde_json::json;

use super::*;

const PROGRAM: &str = "https://health.go.ug/fhir/CodeSystem/program";
const LOINC: &str = "http://loinc.org";

fn anc() -> ProgramType {
    ProgramType {
        url: "https://health.go.ug/fhir/PlanDefinition/anc".into(),
        system: Some(PROGRAM.into()),
        code: Some("anc".into()),
        text: "Antenatal care".into(),
    }
}

fn march() -> Period {
    Period::parse(PeriodType::Monthly, "2026-03").expect("period")
}

fn episode(id: &str, code: &str, start: &str, end: Option<&str>) -> Value {
    let mut period = json!({ "start": start });
    if let Some(end) = end {
        period["end"] = json!(end);
    }
    json!({
        "resourceType": "EpisodeOfCare", "id": id, "status": "active",
        "type": [{ "coding": [{ "system": PROGRAM, "code": code }] }],
        "period": period
    })
}

fn visit(id: &str, episode: &str, kind: &str, on: Option<&str>) -> Value {
    let mut visit = json!({
        "resourceType": "Encounter", "id": id, "status": "completed",
        "type": [{ "coding": [{ "system": "https://health.go.ug/fhir/CodeSystem/anc-visit-type", "code": kind, "display": kind }] }],
        "episodeOfCare": [{ "reference": format!("EpisodeOfCare/{episode}") }],
        "meta": { "lastUpdated": "2026-04-03T08:05:00+03:00" }
    });
    if let Some(on) = on {
        visit["actualPeriod"] = json!({ "start": on });
    }
    visit
}

fn ga(id: &str, visit: &str, on: &str, weeks: u32) -> Value {
    json!({
        "resourceType": "Observation", "id": id, "status": "final",
        "code": { "coding": [{ "system": LOINC, "code": "18185-9" }] },
        "encounter": { "reference": format!("Encounter/{visit}") },
        "effectiveDateTime": on,
        "valueQuantity": { "value": weeks, "unit": "weeks" }
    })
}

fn patient(id: &str, birth: &str, records: Vec<Value>) -> PatientRecords {
    PatientRecords {
        patient: json!({
            "resourceType": "Patient", "id": id,
            "name": [{ "given": [id], "family": "Test" }],
            "gender": "female", "birthDate": birth,
            "telecom": [{ "system": "email", "value": "x@y" }, { "system": "phone", "value": "+256700000001" }]
        }),
        records,
    }
}

/// Nakato enrolled 4 March, visits 4 and 28 March, then an April visit after the period;
/// Auma enrolled in January, visits in January and March; Atim's enrollment ended in February;
/// Okello is in another program.
fn patients() -> Vec<PatientRecords> {
    vec![
        patient(
            "nakato",
            "2002-11-02",
            vec![
                episode("nakato-anc", "anc", "2026-03-04", None),
                visit(
                    "nakato-1",
                    "nakato-anc",
                    "first",
                    Some("2026-03-04T09:10:00+03:00"),
                ),
                ga("nakato-1-ga", "nakato-1", "2026-03-04T09:10:00+03:00", 10),
                visit(
                    "nakato-2",
                    "nakato-anc",
                    "follow-up",
                    Some("2026-03-28T10:00:00+03:00"),
                ),
                ga("nakato-2-ga", "nakato-2", "2026-03-28T10:00:00+03:00", 14),
                visit(
                    "nakato-3",
                    "nakato-anc",
                    "follow-up",
                    Some("2026-04-20T10:00:00+03:00"),
                ),
                ga("nakato-3-ga", "nakato-3", "2026-04-20T10:00:00+03:00", 17),
                visit("nakato-planned", "nakato-anc", "follow-up", None),
            ],
        ),
        patient(
            "auma",
            "1990-03-31",
            vec![
                episode("auma-anc", "anc", "2026-01-07", None),
                visit(
                    "auma-1",
                    "auma-anc",
                    "first",
                    Some("2026-01-07T09:00:00+03:00"),
                ),
                visit(
                    "auma-2",
                    "auma-anc",
                    "follow-up",
                    Some("2026-03-10T09:00:00+03:00"),
                ),
            ],
        ),
        patient(
            "atim",
            "1999-06-01",
            vec![episode("atim-anc", "anc", "2025-11-01", Some("2026-02-20"))],
        ),
        patient(
            "okello",
            "1980-01-01",
            vec![episode("okello-hiv", "hiv", "2026-03-02", None)],
        ),
    ]
}

fn list(row_per: RowPer, columns: Value) -> LineListSection {
    serde_json::from_value(json!({
        "id": "clients", "title": "Clients", "rowPer": row_per,
        "program": "https://health.go.ug/fhir/PlanDefinition/anc",
        "columns": columns
    }))
    .expect("list")
}

fn values(cell: &[CellValue]) -> Vec<&str> {
    cell.iter().map(|v| v.value.as_str()).collect()
}

#[test]
fn an_enrollment_is_one_of_the_programs_by_its_type_or_its_program_extension() {
    let program = anc();
    assert!(program.enrolls(&episode("a", "anc", "2026-03-01", None)));
    assert!(!program.enrolls(&episode("b", "hiv", "2026-03-01", None)));
    let mut erred = episode("c", "anc", "2026-03-01", None);
    erred["status"] = json!("entered-in-error");
    assert!(!program.enrolls(&erred));
    let named = json!({
        "resourceType": "EpisodeOfCare", "id": "d", "status": "active",
        "extension": [{ "url": EPISODE_PROGRAM, "valueCanonical": "https://health.go.ug/fhir/PlanDefinition/anc|1.0.0" }],
        "type": [{ "text": "something else" }]
    });
    assert!(program.enrolls(&named));
    // A program whose episode type has only text matches it by text.
    let texted = ProgramType {
        system: None,
        code: None,
        text: "HIV care".into(),
        ..anc()
    };
    assert!(
        texted
            .enrolls(&json!({ "resourceType": "EpisodeOfCare", "type": [{ "text": "HIV care" }] }))
    );
    assert!(!texted.enrolls(&episode("e", "anc", "2026-03-01", None)));
}

#[test]
fn an_enrollment_list_shows_those_active_during_or_started_in_the_period_with_their_attributes() {
    let mut active = list(
        RowPer::Enrollment,
        json!([
            { "id": "name", "label": "Name", "attribute": "person.name" },
            { "id": "age", "label": "Age", "attribute": "person.age" },
            { "id": "phone", "label": "Phone", "attribute": "person.phone" },
            { "id": "start", "label": "Start", "attribute": "enrollment.start" },
            { "id": "visits", "label": "Visits", "attribute": "enrollment.visits" },
            { "id": "last", "label": "Last visit", "attribute": "enrollment.lastVisit" },
            { "id": "date", "label": "Visit date", "attribute": "visit.date" }
        ]),
    );
    active.enrollments = Some(Enrollments::Active);
    let rows = rows(&active, &anc(), &patients(), &march(), None);
    let records: Vec<&str> = rows.iter().map(|r| r.record.as_str()).collect();
    // Atim's ended before March; Okello's is another program.
    assert_eq!(
        records,
        ["EpisodeOfCare/nakato-anc", "EpisodeOfCare/auma-anc"]
    );
    let nakato = &rows[0];
    assert_eq!(nakato.patient, "Patient/nakato");
    let cells: Vec<Vec<&str>> = nakato.cells.iter().map(|c| values(c)).collect();
    // 23 at the period's end; the April visit and the undated one are not among her visits yet.
    assert_eq!(
        cells,
        [
            vec!["nakato Test"],
            vec!["23"],
            vec!["+256700000001"],
            vec!["2026-03-04"],
            vec!["2"],
            vec!["2026-03-28"],
            vec![]
        ]
    );
    // Auma turned 36 on 31 March.
    assert_eq!(values(&rows[1].cells[1]), ["36"]);

    let mut started = active.clone();
    started.enrollments = Some(Enrollments::Started);
    let rows = super::rows(&started, &anc(), &patients(), &march(), None);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].record, "EpisodeOfCare/nakato-anc");
}

#[test]
fn a_concept_column_picks_the_latest_first_all_or_count_of_the_enrollments_observations() {
    let column = |pick: &str| json!({ "id": pick, "label": pick, "concept": { "system": LOINC, "code": "18185-9" }, "pick": pick });
    let mut enrolled = list(
        RowPer::Enrollment,
        json!([
            column("latest"),
            column("first"),
            column("all"),
            column("count")
        ]),
    );
    enrolled.enrollments = Some(Enrollments::Active);
    let rows = rows(&enrolled, &anc(), &patients(), &march(), None);
    let nakato = &rows[0];
    // The April value is after the period's end.
    assert_eq!(values(&nakato.cells[0]), ["14 weeks"]);
    assert_eq!(values(&nakato.cells[1]), ["10 weeks"]);
    assert_eq!(values(&nakato.cells[2]), ["10 weeks", "14 weeks"]);
    assert_eq!(
        nakato.cells[2]
            .iter()
            .map(|v| v.date.map(|d| d.to_string()))
            .collect::<Vec<_>>(),
        [Some("2026-03-04".to_owned()), Some("2026-03-28".to_owned())]
    );
    assert_eq!(values(&nakato.cells[3]), ["2"]);
    // Auma has none recorded.
    assert!(rows[1].cells[0].is_empty());
    assert_eq!(values(&rows[1].cells[3]), ["0"]);
}

#[test]
fn a_visit_list_shows_the_programs_visits_in_the_period_of_its_types_with_their_own_values() {
    let mut visits = list(
        RowPer::Visit,
        json!([
            { "id": "name", "label": "Name", "attribute": "person.name" },
            { "id": "date", "label": "Date", "attribute": "visit.date" },
            { "id": "type", "label": "Type", "attribute": "visit.type" },
            { "id": "recorded", "label": "Recorded", "attribute": "visit.recorded" },
            { "id": "start", "label": "Enrolled", "attribute": "enrollment.start" },
            { "id": "ga", "label": "GA", "concept": { "system": LOINC, "code": "18185-9" } }
        ]),
    );
    let all = rows(&visits, &anc(), &patients(), &march(), None);
    let records: Vec<&str> = all.iter().map(|r| r.record.as_str()).collect();
    assert_eq!(
        records,
        [
            "Encounter/nakato-1",
            "Encounter/nakato-2",
            "Encounter/auma-2"
        ]
    );
    let cells: Vec<Vec<&str>> = all[1].cells.iter().map(|c| values(c)).collect();
    assert_eq!(
        cells,
        [
            vec!["nakato Test"],
            vec!["2026-03-28"],
            vec!["follow-up"],
            vec!["2026-04-03"],
            vec!["2026-03-04"],
            vec!["14 weeks"]
        ]
    );
    visits.visit_types = vec![Code {
        system: "https://health.go.ug/fhir/CodeSystem/anc-visit-type".into(),
        code: "first".into(),
    }];
    let first = rows(&visits, &anc(), &patients(), &march(), None);
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].record, "Encounter/nakato-1");
}

#[test]
fn only_those_counted_keeps_rows_whose_record_or_enrollment_was_counted() {
    let counted: BTreeSet<String> = ["Encounter/nakato-1".to_owned()].into();
    let visits = list(
        RowPer::Visit,
        json!([{ "id": "name", "label": "Name", "attribute": "person.name" }]),
    );
    let kept = rows(&visits, &anc(), &patients(), &march(), Some(&counted));
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].record, "Encounter/nakato-1");
    // An enrollment is kept when one of its visits was counted.
    let mut enrolled = list(
        RowPer::Enrollment,
        json!([{ "id": "name", "label": "Name", "attribute": "person.name" }]),
    );
    enrolled.enrollments = Some(Enrollments::Active);
    let kept = rows(&enrolled, &anc(), &patients(), &march(), Some(&counted));
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].record, "EpisodeOfCare/nakato-anc");
    // A visit is kept when its enrollment was counted (an indicator counting enrollments).
    let counted: BTreeSet<String> = ["EpisodeOfCare/auma-anc".to_owned()].into();
    let kept = rows(&visits, &anc(), &patients(), &march(), Some(&counted));
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].record, "Encounter/auma-2");
}

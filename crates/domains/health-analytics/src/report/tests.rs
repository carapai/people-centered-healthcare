use serde_json::{Value, json};

use super::*;
use crate::indicator::Aggregate;

const FIRST: &str = "https://health.go.ug/fhir/Measure/anc-first-visits|1.0.0";
const MEDIAN: &str = "https://health.go.ug/fhir/Measure/anc-gestational-age-at-first-visit|1.0.0";
const DISTINCT: &str = "https://health.go.ug/fhir/Measure/anc-districts|1.0.0";
const BANDS: &str = "https://health.go.ug/fhir/Library/age-bands-anc|1.0.0";

fn catalogue() -> Catalogue {
    Catalogue {
        indicators: [
            (FIRST.to_owned(), Calculation::Count),
            (MEDIAN.to_owned(), Calculation::Value(Aggregate::Median)),
            (
                DISTINCT.to_owned(),
                Calculation::Value(Aggregate::DistinctCount),
            ),
        ]
        .into(),
        age_band_sets: [BANDS.to_owned()].into(),
        ..Catalogue::default()
    }
}

fn report(levels: Value, sections: Value) -> ReportDefinition {
    serde_json::from_value(json!({
        "id": "hmis-105-anc",
        "title": "HMIS 105 · 2 Antenatal care",
        "periodType": "monthly",
        "levels": levels,
        "sections": sections,
    }))
    .expect("a Report")
}

fn table(rows: Value) -> Value {
    json!({
        "kind": "table",
        "id": "attendance",
        "title": "2.1 ANC attendance",
        "disaggregations": [{ "ageBandSet": BANDS, "axis": "columns" }],
        "rows": rows,
        "rowTotals": true,
        "change": true,
        "sendToDhis2": true,
    })
}

fn line_list() -> Value {
    json!({
        "kind": "lineList",
        "id": "clients",
        "title": "ANC clients",
        "rowPer": "enrollment",
        "program": "https://health.go.ug/fhir/PlanDefinition/anc",
        "enrollments": "active",
        "countedBy": FIRST,
        "columns": [
            { "id": "name", "label": "Name", "attribute": "person.name" },
            { "id": "ga", "label": "Gestational age", "concept": { "system": "http://loinc.org", "code": "18185-9" }, "pick": "latest" },
        ],
    })
}

#[test]
fn a_report_reads_as_written_and_back() {
    let written = json!({
        "id": "hmis-105-anc",
        "title": "HMIS 105 · 2 Antenatal care",
        "periodType": "monthly",
        "levels": ["facility", "district"],
        "sections": [
            {
                "kind": "table",
                "id": "attendance",
                "title": "2.1 ANC attendance",
                "disaggregations": [
                    { "code": "hiv-status", "expression": "%patient.extension.value", "axis": "rows" },
                    { "ageBandSet": BANDS, "axis": "columns" },
                ],
                "rows": [{ "id": "first", "indicator": FIRST, "label": "ANC 1st visit for women" }],
                "columnTotals": true,
                "rowTotals": true,
                "change": false,
                "sendToDhis2": true,
            },
        ],
    });
    let read: ReportDefinition = serde_json::from_value(written.clone()).expect("a Report");
    assert_eq!(read.period_type, PeriodType::Monthly);
    assert_eq!(read.levels, [Level::Facility, Level::District]);
    let Section::Table(section) = &read.sections[0] else {
        panic!("a table section")
    };
    assert_eq!(section.disaggregations[1].axis, Axis::Columns);
    assert!(section.send_to_dhis2);
    assert_eq!(serde_json::to_value(&read).expect("json"), written);
    assert_eq!(read.problems(&catalogue()), Vec::<String>::new());
}

#[test]
fn a_median_row_at_district_level_fails_with_the_reason() {
    let at = |levels| {
        report(
            levels,
            json!([table(json!([
                { "id": "first", "indicator": FIRST, "label": "ANC 1st visit for women" },
                { "id": "ga", "indicator": MEDIAN, "label": "Median gestational age" },
            ]))]),
        )
    };
    assert_eq!(
        at(json!(["facility", "district"])).problems(&catalogue()),
        [
            "section 1 (2.1 ANC attendance)'s row 2 (Median gestational age) is a median, which cannot be rebuilt from facility summaries because patient-level data never leaves the facility; the Report runs at district level: run it at facility level only, or move the row to a facility Report"
        ]
    );
    assert!(at(json!(["facility"])).problems(&catalogue()).is_empty());
}

#[test]
fn a_distinct_count_row_above_facility_fails_too() {
    let r = report(
        json!(["region", "national", "district"]),
        json!([table(
            json!([{ "id": "d", "indicator": DISTINCT, "label": "Districts" }])
        )]),
    );
    assert_eq!(
        r.problems(&catalogue()),
        [
            "section 1 (2.1 ANC attendance)'s row 1 (Districts) is a distinct count, which cannot be rebuilt from facility summaries because patient-level data never leaves the facility; the Report runs at district, region and national levels: run it at facility level only, or move the row to a facility Report"
        ]
    );
}

#[test]
fn a_line_list_at_national_level_fails() {
    let at = |levels| report(levels, json!([line_list()]));
    assert_eq!(
        at(json!(["national"])).problems(&catalogue()),
        [
            "section 1 (ANC clients) is a line list, which shows patient-level records, and they never leave the facility; the Report runs at national level: run it at facility level only, or move the line list to a facility Report"
        ]
    );
    assert!(at(json!(["facility"])).problems(&catalogue()).is_empty());
}

#[test]
fn indicators_and_age_band_sets_are_pinned() {
    let mut section = table(json!([
        { "id": "a", "indicator": "https://health.go.ug/fhir/Measure/anc-first-visits", "label": "Unpinned" },
        { "id": "b", "indicator": "https://health.go.ug/fhir/Measure/anc-first-visits|9.0.0", "label": "Unknown" },
    ]));
    section["disaggregations"] = json!([
        { "ageBandSet": "https://health.go.ug/fhir/Library/age-bands-anc", "axis": "columns" },
        { "ageBandSet": "https://health.go.ug/fhir/Library/age-bands-anc|2.0.0", "axis": "rows" },
    ]);
    let r = report(json!(["facility"]), json!([section]));
    assert_eq!(
        r.problems(&catalogue()),
        [
            "section 1 (2.1 ANC attendance)'s disaggregation 1: the Age Band Set https://health.go.ug/fhir/Library/age-bands-anc is not pinned to a version (url|version)",
            "section 1 (2.1 ANC attendance)'s disaggregation 2: no published Age Band Set is https://health.go.ug/fhir/Library/age-bands-anc|2.0.0",
            "section 1 (2.1 ANC attendance)'s row 1 (Unpinned): the indicator https://health.go.ug/fhir/Measure/anc-first-visits is not pinned to a version (url|version)",
            "section 1 (2.1 ANC attendance)'s row 2 (Unknown): no published indicator is https://health.go.ug/fhir/Measure/anc-first-visits|9.0.0",
        ]
    );
}

#[test]
fn totals_need_a_disaggregation_on_their_side() {
    let mut section = table(json!([{ "id": "a", "indicator": FIRST, "label": "First" }]));
    section["columnTotals"] = json!(true);
    section["disaggregations"] = json!([
        { "code": "hiv-status", "axis": "columns" },
        { "code": "hiv-status", "expression": "x", "axis": "columns" },
        { "axis": "columns" },
        { "ageBandSet": BANDS, "code": "age", "axis": "columns" },
    ]);
    section["rowTotals"] = json!(true);
    let r = report(json!(["facility"]), json!([section]));
    assert_eq!(
        r.problems(&catalogue()),
        [
            "section 1 (2.1 ANC attendance)'s disaggregation 1 (hiv-status) has no expression giving a record's code",
            "section 1 (2.1 ANC attendance) is broken down by hiv-status twice",
            "section 1 (2.1 ANC attendance)'s disaggregation 3 names neither an Age Band Set nor a coded value",
            "section 1 (2.1 ANC attendance)'s disaggregation 4 is both by an Age Band Set and by a coded value",
            "section 1 (2.1 ANC attendance) shows column totals, which need a disaggregation on rows",
        ]
    );
}

#[test]
fn a_report_needs_its_parts() {
    let r: ReportDefinition = serde_json::from_value(json!({
        "id": " ",
        "title": "",
        "periodType": "quarterly",
    }))
    .expect("a Report");
    assert_eq!(
        r.problems(&catalogue()),
        [
            "the Report has no id",
            "the Report has no title",
            "the Report runs for no level: choose facility, district, region or national",
            "the Report has no sections",
        ]
    );
    let twice = report(
        json!(["facility"]),
        json!([
            table(json!([])),
            table(json!([{ "id": "", "indicator": FIRST, "label": "" }]))
        ]),
    );
    assert_eq!(
        twice.problems(&catalogue()),
        [
            "section 1 (2.1 ANC attendance) has no indicator rows",
            "section 2 (2.1 ANC attendance) has the id attendance, which another section has",
            "section 2 (2.1 ANC attendance)'s row 1 has no id",
            "section 2 (2.1 ANC attendance)'s row 1 has no label",
        ]
    );
}

#[test]
fn line_list_columns_fit_their_rows() {
    let list = json!({
        "kind": "lineList",
        "id": "visits",
        "title": "Visits",
        "rowPer": "visit",
        "program": "",
        "enrollments": "started",
        "visitTypes": [{ "system": "", "code": "first" }],
        "countedBy": "https://health.go.ug/fhir/Measure/anc-first-visits",
        "columns": [
            { "id": "a", "label": "Shoe size", "attribute": "person.shoeSize" },
            { "id": "a", "label": "Name", "attribute": "person.name", "pick": "latest" },
            { "id": "c", "label": "GA", "concept": { "system": "http://loinc.org", "code": "18185-9" }, "pick": "first" },
            { "id": "d", "label": "Both", "attribute": "visit.date", "concept": { "system": "s", "code": "c" } },
            { "id": "e", "label": "Neither" },
        ],
    });
    let r = report(json!(["facility"]), json!([list]));
    assert_eq!(
        r.problems(&catalogue()),
        [
            "section 1 (Visits) names no program",
            "section 1 (Visits) lists visits, so it does not choose enrollments",
            "section 1 (Visits) has a visit type without its system and code",
            "section 1 (Visits) lists only those counted by https://health.go.ug/fhir/Measure/anc-first-visits: the indicator https://health.go.ug/fhir/Measure/anc-first-visits is not pinned to a version (url|version)",
            "section 1 (Visits)'s column 1 (Shoe size): person.shoeSize is not an attribute of the person, enrollment or visit",
            "section 1 (Visits)'s column 2 (Name) has the id a, which another column has",
            "section 1 (Visits)'s column 2 (Name) is an attribute, which has one value: it picks none",
            "section 1 (Visits)'s column 3 (GA) is on a visit list, where a concept has the visit's own value: it picks none",
            "section 1 (Visits)'s column 4 (Both) is both an attribute and a concept",
            "section 1 (Visits)'s column 5 (Neither) is neither an attribute nor a concept",
        ]
    );
    let mut enrollments = line_list();
    enrollments["enrollments"] = Value::Null;
    enrollments["columns"] = json!([
        { "id": "v", "label": "Visit date", "attribute": "visit.date" },
        { "id": "g", "label": "GA", "concept": { "system": "http://loinc.org", "code": "18185-9" } },
    ]);
    let r = report(json!(["facility"]), json!([enrollments]));
    assert_eq!(
        r.problems(&catalogue()),
        [
            "section 1 (ANC clients) does not say which enrollments it lists: started in the period, or active during it",
            "section 1 (ANC clients)'s column 1 (Visit date): visit.date is a visit's, and a row is an enrollment, which has many visits",
            "section 1 (ANC clients)'s column 2 (GA) does not say which of an enrollment's values it shows: the latest, the first, all, or how many times it was recorded",
        ]
    );
}

#[test]
fn each_problem_says_which_section_it_is_in() {
    let mut untitled = table(json!([{ "id": "first", "indicator": FIRST, "label": "" }]));
    untitled["id"] = json!("timing");
    let located = report(
        json!([]),
        json!([
            table(json!([{ "id": "first", "indicator": FIRST, "label": "ANC 1st visit" }])),
            untitled
        ]),
    )
    .located_problems(&catalogue());
    let at: Vec<(Option<usize>, &str)> = located
        .iter()
        .map(|p| (p.section, p.message.as_str()))
        .collect();
    assert_eq!(
        at,
        [
            (
                None,
                "the Report runs for no level: choose facility, district, region or national"
            ),
            (
                Some(1),
                "section 2 (2.1 ANC attendance)'s row 1 has no label"
            ),
        ]
    );
}

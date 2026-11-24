use serde_json::{Value, json};

use super::*;
use crate::report::ReportDefinition;

const FIRST: &str = "https://health.go.ug/fhir/Measure/anc-first-visits|1.0.0";
const EARLY: &str = "https://health.go.ug/fhir/Measure/anc-first-visit-before-12-weeks|1.0.0";
const BANDS: &str = "https://health.go.ug/fhir/Library/age-bands-anc|1.0.0";

/// A recorded copy of an instance's `dataSets` export: HMIS 105 (monthly), whose ANC 1st visit
/// data element takes the ANC age bands, and HMIS 106a (quarterly).
fn export() -> Value {
    let ages = json!({ "categoryOptionCombos": [
        { "id": "coc10to14", "name": "10-14 Years" },
        { "id": "coc15to19", "name": "15-19 Years" },
        { "id": "coc20to24", "name": "20-24 Years" },
        { "id": "coc25plus", "name": "25 Years and above" },
    ] });
    json!({ "dataSets": [
        {
            "id": "hmis105", "name": "HMIS 105", "periodType": "Monthly",
            "dataSetElements": [
                { "dataElement": { "id": "an01a", "name": "105-AN01a. ANC 1st visit for women", "categoryCombo": ages } },
                {
                    "dataElement": { "id": "an02", "name": "105-AN02. ANC visits", "categoryCombo": { "categoryOptionCombos": [{ "id": "default", "name": "default" }] } },
                    "categoryCombo": ages,
                },
            ],
        },
        {
            "id": "hmis106a", "name": "HMIS 106a", "periodType": "Quarterly",
            "dataSetElements": [{ "dataElement": { "id": "art01", "name": "106a-ART01", "categoryCombo": { "categoryOptionCombos": [{ "id": "default" }] } } }],
        },
    ] })
}

fn metadata() -> Dhis2Metadata {
    serde_json::from_value::<DataSetsExport>(export())
        .expect("an export")
        .metadata()
}

fn catalogue(dhis2: Option<Dhis2Metadata>) -> Catalogue {
    Catalogue {
        indicators: [
            (FIRST.to_owned(), Calculation::Count),
            (EARLY.to_owned(), Calculation::Percentage),
        ]
        .into(),
        age_band_sets: [BANDS.to_owned()].into(),
        age_band_codes: [(
            BANDS.to_owned(),
            ["10-14", "15-19", "20-24", "25+"]
                .map(str::to_owned)
                .to_vec(),
        )]
        .into(),
        dhis2,
        ..Catalogue::default()
    }
}

/// A mapping of `row` at age band `band` to ANC 1st visits' category option combo `coc`.
fn mapping(row: &str, band: &str, coc: &str, from: &str, to: Option<&str>) -> Value {
    let mut m = json!({
        "row": row,
        "bands": { BANDS: band },
        "dataSet": "hmis105",
        "dataElement": "an01a",
        "categoryOptionCombo": coc,
        "from": from,
    });
    if let Some(to) = to {
        m["to"] = json!(to);
    }
    m
}

fn every_band() -> Vec<Value> {
    [
        ("10-14", "coc10to14"),
        ("15-19", "coc15to19"),
        ("20-24", "coc20to24"),
        ("25+", "coc25plus"),
    ]
    .iter()
    .map(|(band, coc)| mapping("first", band, coc, "2024-07-01", None))
    .collect()
}

fn report(rows: Value, mappings: Vec<Value>) -> ReportDefinition {
    serde_json::from_value(json!({
        "id": "hmis-105-anc",
        "title": "HMIS 105 · 2 Antenatal care",
        "periodType": "monthly",
        "levels": ["facility", "district"],
        "sections": [{
            "kind": "table",
            "id": "attendance",
            "title": "2.1 ANC attendance",
            "disaggregations": [{ "ageBandSet": BANDS, "axis": "columns" }],
            "rows": rows,
            "rowTotals": true,
            "sendToDhis2": true,
            "mappings": mappings,
        }],
    }))
    .expect("a Report")
}

fn first_row() -> Value {
    json!([{ "id": "first", "indicator": FIRST, "label": "ANC 1st visit for women" }])
}

const SECTION: &str = "section 1 (2.1 ANC attendance)";

#[test]
fn a_fully_mapped_section_passes_and_reads_back_as_written() {
    let r = report(first_row(), every_band());
    assert_eq!(
        r.problems(&catalogue(Some(metadata()))),
        Vec::<String>::new()
    );
    let written = serde_json::to_value(&r).expect("json");
    assert_eq!(written["sections"][0]["mappings"][3], every_band()[3]);
}

#[test]
fn an_unmapped_count_cell_in_a_sent_section_fails_validation() {
    let mut mappings = every_band();
    mappings.remove(1);
    assert_eq!(
        report(first_row(), mappings).problems(&catalogue(Some(metadata()))),
        [format!(
            "{SECTION}: row 1 (ANC 1st visit for women) at 15-19 is not mapped to DHIS2"
        )]
    );
    // A section that is not sent needs no mappings.
    let mut unsent = report(first_row(), Vec::new());
    let crate::report::Section::Table(table) = &mut unsent.sections[0] else {
        panic!("a table")
    };
    table.send_to_dhis2 = false;
    assert!(unsent.problems(&catalogue(Some(metadata()))).is_empty());
}

#[test]
fn a_cell_whose_mappings_all_end_is_unmapped_after_the_last() {
    let mut mappings = every_band();
    mappings[0] = mapping(
        "first",
        "10-14",
        "coc10to14",
        "2019-07-01",
        Some("2024-06-30"),
    );
    assert_eq!(
        report(first_row(), mappings).problems(&catalogue(Some(metadata()))),
        [format!(
            "{SECTION}: row 1 (ANC 1st visit for women) at 10-14 has no DHIS2 mapping after 2024-06-30: map it until further notice"
        )]
    );
}

#[test]
fn a_mapping_to_a_data_element_missing_from_the_instance_fails_validation() {
    let mut mappings = every_band();
    mappings[0]["dataElement"] = json!("an01x");
    mappings[1]["dataElement"] = json!("art01");
    mappings[2]["categoryOptionCombo"] = json!("default");
    mappings[3]["dataSet"] = json!("hmis106a");
    mappings[3]["dataElement"] = json!("art01");
    mappings[3]["categoryOptionCombo"] = json!("default");
    assert_eq!(
        report(first_row(), mappings).problems(&catalogue(Some(metadata()))),
        [
            format!(
                "{SECTION}'s DHIS2 mapping 1 (row 1 (ANC 1st visit for women) at 10-14): the data element an01x is not in the DHIS2 instance's metadata"
            ),
            format!(
                "{SECTION}'s DHIS2 mapping 2 (row 1 (ANC 1st visit for women) at 15-19): the data element art01 is not in the data set hmis105 (HMIS 105)"
            ),
            format!(
                "{SECTION}'s DHIS2 mapping 3 (row 1 (ANC 1st visit for women) at 20-24): the category option combo default is not one of the data element an01a (105-AN01a. ANC 1st visit for women)"
            ),
            format!(
                "{SECTION}'s DHIS2 mapping 4 (row 1 (ANC 1st visit for women) at 25+): the data set hmis106a (HMIS 106a) is Quarterly, and the Report is Monthly"
            ),
        ]
    );
}

#[test]
fn a_data_set_override_of_the_category_combo_is_what_counts() {
    let mut mappings = every_band();
    for m in &mut mappings {
        m["dataElement"] = json!("an02");
    }
    assert!(
        report(first_row(), mappings)
            .problems(&catalogue(Some(metadata())))
            .is_empty()
    );
}

#[test]
fn without_a_recorded_copy_of_the_metadata_mappings_cannot_be_checked() {
    assert_eq!(
        report(first_row(), every_band()).problems(&catalogue(None)),
        [format!(
            "{SECTION}'s DHIS2 mappings cannot be checked: no copy of the DHIS2 instance's metadata is recorded at central"
        )]
    );
}

#[test]
fn two_mappings_in_effect_on_the_same_dates_fail_validation() {
    let mut mappings = every_band();
    mappings.push(mapping(
        "first",
        "15-19",
        "coc15to19",
        "2019-07-01",
        Some("2024-07-01"),
    ));
    assert_eq!(
        report(first_row(), mappings.clone()).problems(&catalogue(Some(metadata()))),
        [format!(
            "{SECTION}: two DHIS2 mappings of row 1 (ANC 1st visit for women) at 15-19 are in effect on the same dates (2024-07-01 to further notice and 2019-07-01 to 2024-07-01)"
        )]
    );
    // One after the other is a revision of the form.
    mappings.last_mut().expect("added")["to"] = json!("2024-06-30");
    assert!(
        report(first_row(), mappings)
            .problems(&catalogue(Some(metadata())))
            .is_empty()
    );
}

#[test]
fn only_count_rows_are_mapped() {
    let rows = json!([
        { "id": "first", "indicator": FIRST, "label": "ANC 1st visit for women" },
        { "id": "early", "indicator": EARLY, "label": "Before 12 weeks (%)" },
    ]);
    // A percentage needs no mapping…
    assert!(
        report(rows.clone(), every_band())
            .problems(&catalogue(Some(metadata())))
            .is_empty()
    );
    // …and takes none.
    let mut mappings = every_band();
    mappings.push(mapping("early", "15-19", "coc15to19", "2024-07-01", None));
    assert_eq!(
        report(rows, mappings).problems(&catalogue(Some(metadata()))),
        [format!(
            "{SECTION}'s DHIS2 mapping 5 (row 2 (Before 12 weeks (%)) at 15-19) maps a row that is not a count, and DHIS2's data elements on these forms hold counts: send the counts it is built from instead"
        )]
    );
}

#[test]
fn a_mapping_names_a_cell_of_the_section() {
    let mut mappings = every_band();
    mappings.push(mapping("first", "30-34", "coc25plus", "2024-07-01", None));
    mappings.push(json!({ "row": "nobody", "dataSet": "hmis105", "dataElement": "an01a", "categoryOptionCombo": "coc25plus", "from": "July" }));
    // A Total (no column band) is a cell, as the section shows row totals.
    mappings.push(json!({ "row": "first", "dataSet": "hmis105", "dataElement": "an01a", "categoryOptionCombo": "coc25plus", "from": "2024-07-01", "to": "2024-01-01" }));
    assert_eq!(
        report(first_row(), mappings).problems(&catalogue(Some(metadata()))),
        [
            format!(
                "{SECTION}'s DHIS2 mapping 5 (row 1 (ANC 1st visit for women) at 30-34): 30-34 is not a band of {BANDS} (its bands are 10-14, 15-19, 20-24, 25+)"
            ),
            format!(
                "{SECTION}'s DHIS2 mapping 6 (row nobody) is of the row nobody, which the section does not have"
            ),
            format!(
                "{SECTION}'s DHIS2 mapping 6 (row nobody) has no date it is in effect from (YYYY-MM-DD)"
            ),
            format!(
                "{SECTION}'s DHIS2 mapping 7 (row 1 (ANC 1st visit for women)) is in effect to 2024-01-01, before it is in effect from 2024-07-01"
            ),
        ]
    );
}

#[test]
fn a_coded_disaggregation_is_sent_by_the_codes_mapped() {
    let crossed = |mappings: Vec<Value>| -> ReportDefinition {
        let mut r = report(first_row(), mappings);
        let crate::report::Section::Table(table) = &mut r.sections[0] else {
            panic!("a table")
        };
        table.disaggregations.push(
            serde_json::from_value(
                json!({ "code": "hiv-status", "expression": "%patient.id", "axis": "rows" }),
            )
            .expect("a disaggregation"),
        );
        r
    };
    assert_eq!(
        crossed(Vec::new()).problems(&catalogue(Some(metadata()))),
        [format!(
            "{SECTION} is sent to DHIS2 by hiv-status, but no mapping names one of its codes: map the codes DHIS2 takes"
        )]
    );
    // Positive is mapped at one age band: the other three are not.
    let mut one = mapping("first", "10-14", "coc10to14", "2024-07-01", None);
    one["bands"]["hiv-status"] = json!("positive");
    let problems = crossed(vec![one]).problems(&catalogue(Some(metadata())));
    assert_eq!(
        problems,
        ["15-19", "20-24", "25+"].map(|band| format!(
            "{SECTION}: row 1 (ANC 1st visit for women) at {band} × positive is not mapped to DHIS2"
        ))
    );
}

#[test]
fn a_section_not_sent_maps_nothing() {
    let mut r = report(first_row(), every_band());
    let crate::report::Section::Table(table) = &mut r.sections[0] else {
        panic!("a table")
    };
    table.send_to_dhis2 = false;
    assert_eq!(
        r.problems(&catalogue(Some(metadata()))),
        [format!(
            "{SECTION} maps cells to DHIS2 but is not sent to DHIS2: send it, or remove its mappings"
        )]
    );
}

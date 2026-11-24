//! Indicator summaries on the wire (#555; analytics.md § Sync; ADR 0068): only summaries leave
//! the facility. An edge sends `POST /sync/v1/indicator-summaries` a list of
//! [`IndicatorSummary`]s: each summary `MeasureReport` whose figures central does not hold yet,
//! and each one forgotten since (no `report`). Central keeps them per facility and rolls them up.
//!
//! [`summary_problem`] is what central holds each one to: a summary `MeasureReport` for the
//! indicator, period and facility it is sent as, built only of the elements a summary's figures
//! need ([`SUMMARY_ELEMENTS`]), so no subject, evaluated resource, contained resource or
//! reference to a patient can travel in one.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One facility's summary of one indicator and period, or its removal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndicatorSummary {
    /// The indicator, `url|version`.
    pub measure: String,
    /// The period's type (`monthly`, `financialJuly`, …).
    pub period_type: String,
    /// The period's id (`2026-03`).
    pub period: String,
    /// The facility it is for.
    pub facility: String,
    /// The hash of its figures, as the edge computed it; empty for a removal.
    pub hash: String,
    /// The summary `MeasureReport`; none when the summary was forgotten (no care in the period
    /// counts any more).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report: Option<Value>,
}

/// The JSON names a summary `MeasureReport` may hold, at any depth: what its figures, period,
/// facility and computed-at time need, and nothing that could name a patient. A crossing of
/// Disaggregations (#557) is a stratifier naming them in `valueString` extensions, its strata's
/// bands as `component`s.
pub const SUMMARY_ELEMENTS: &[&str] = &[
    "resourceType",
    "meta",
    "tag",
    "system",
    "code",
    "coding",
    "text",
    "extension",
    "url",
    "valueCode",
    "valueString",
    "valueDecimal",
    "valueInteger64",
    "status",
    "type",
    "measure",
    "date",
    "reporter",
    "identifier",
    "value",
    "period",
    "start",
    "end",
    "group",
    "population",
    "count",
    "measureScoreQuantity",
    "stratifier",
    "stratum",
    "component",
];

/// The facility tag and reporter identifier system (ADR 0057).
const FACILITY: &str = "urn:health:facility";

fn foreign(value: &Value, at: &str) -> Option<String> {
    match value {
        Value::Object(map) => map.iter().find_map(|(name, inner)| {
            if SUMMARY_ELEMENTS.contains(&name.as_str()) {
                foreign(inner, name)
            } else {
                Some(format!("it holds `{name}`, which a summary does not"))
            }
        }),
        Value::Array(items) => items.iter().find_map(|item| foreign(item, at)),
        Value::String(text) if text.starts_with("Patient/") => {
            Some(format!("its `{at}` names a patient"))
        }
        _ => None,
    }
}

/// Why `summary` cannot be taken as one facility's summary, if it cannot.
pub fn summary_problem(summary: &IndicatorSummary) -> Option<String> {
    let Some(report) = &summary.report else {
        return None;
    };
    if report["resourceType"] != "MeasureReport" || report["type"] != "summary" {
        return Some("it is not a summary MeasureReport".into());
    }
    if report["measure"] != summary.measure.as_str() {
        return Some(format!("it is not of {}", summary.measure));
    }
    if report["reporter"]["identifier"]["system"] != FACILITY
        || report["reporter"]["identifier"]["value"] != summary.facility.as_str()
    {
        return Some(format!("it is not reported by {}", summary.facility));
    }
    if report["date"].as_str().is_none() || report["period"]["start"].as_str().is_none() {
        return Some("it has no computed-at time or period".into());
    }
    foreign(report, "")
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn summary(report: Value) -> IndicatorSummary {
        IndicatorSummary {
            measure: "https://x/Measure/anc1|1.0.0".into(),
            period_type: "monthly".into(),
            period: "2026-03".into(),
            facility: "KAM013".into(),
            hash: "h".into(),
            report: Some(report),
        }
    }

    fn report() -> Value {
        json!({
            "resourceType": "MeasureReport",
            "meta": { "tag": [{ "system": FACILITY, "code": "KAM013" }] },
            "extension": [{ "url": "https://x/period-type", "valueCode": "monthly" }],
            "status": "complete",
            "type": "summary",
            "measure": "https://x/Measure/anc1|1.0.0",
            "date": "2026-04-01T06:00:00Z",
            "reporter": { "identifier": { "system": FACILITY, "value": "KAM013" } },
            "period": { "start": "2026-03-01", "end": "2026-03-31" },
            "group": [{
                "population": [{ "code": { "coding": [{ "code": "numerator" }] }, "count": 3 }],
                "measureScoreQuantity": { "value": 75 },
                "stratifier": [{ "code": { "text": "age" }, "stratum": [{ "value": { "text": "15-19" } }] }]
            }]
        })
    }

    #[test]
    fn a_summary_and_a_removal_are_taken() {
        assert_eq!(summary_problem(&summary(report())), None);
        let removal = IndicatorSummary {
            report: None,
            ..summary(report())
        };
        assert_eq!(summary_problem(&removal), None);
        // A crossing of Disaggregations (#557).
        let mut crossed = report();
        crossed["group"][0]["stratifier"] = json!([{
            "extension": [
                { "url": "https://x/indicator-crossed", "valueString": "age" },
                { "url": "https://x/indicator-crossed", "valueString": "hiv-status" }
            ],
            "code": { "text": "age × hiv-status" },
            "stratum": [{
                "component": [
                    { "code": { "text": "age" }, "value": { "text": "15-19" } },
                    { "code": { "text": "hiv-status" }, "value": { "text": "positive" } }
                ],
                "population": [{ "code": { "coding": [{ "code": "numerator" }] }, "count": 1 }]
            }]
        }]);
        assert_eq!(summary_problem(&summary(crossed.clone())), None);
        crossed["group"][0]["stratifier"][0]["extension"][0]["valueString"] = json!("Patient/p-1");
        assert_eq!(
            summary_problem(&summary(crossed)).as_deref(),
            Some("its `valueString` names a patient")
        );
        let wire = serde_json::to_value(&removal).expect("json");
        assert!(wire.get("report").is_none(), "{wire}");
    }

    #[test]
    fn no_patient_level_data_is_taken() {
        let mut subject = report();
        subject["subject"] = json!({ "reference": "Patient/p-1" });
        let mut evaluated = report();
        evaluated["evaluatedResource"] = json!([{ "reference": "Encounter/e-1" }]);
        let mut results = report();
        results["group"][0]["population"][0]["subjectResults"] = json!({ "reference": "List/l" });
        let mut named = report();
        named["group"][0]["stratifier"][0]["stratum"][0]["value"]["text"] = json!("Patient/p-1");
        for (report, why) in [
            (subject, "it holds `subject`, which a summary does not"),
            (
                evaluated,
                "it holds `evaluatedResource`, which a summary does not",
            ),
            (
                results,
                "it holds `subjectResults`, which a summary does not",
            ),
            (named, "its `text` names a patient"),
        ] {
            assert_eq!(summary_problem(&summary(report)).as_deref(), Some(why));
        }
    }

    #[test]
    fn a_summary_is_of_what_it_is_sent_as() {
        let mut other = summary(report());
        other.facility = "MUL001".into();
        assert_eq!(
            summary_problem(&other).as_deref(),
            Some("it is not reported by MUL001")
        );
        let mut individual = report();
        individual["type"] = json!("individual");
        assert!(summary_problem(&summary(individual)).is_some());
    }
}

//! An indicator's summary as a FHIR R5 summary `MeasureReport` (analytics.md § Store; ADR 0068;
//! #554): what a node keeps per indicator, period and facility, what dashboards read and what
//! syncs to central to be rolled up.
//!
//! - `measure` is the indicator's `url|version`, `period` its first and last days, and
//!   `reporter` and the `meta.tag` (`urn:health:facility`, ADR 0057) name the facility.
//!   [`PERIOD_TYPE`] says which type of period it is (`monthly`, `financialJuly`, …).
//! - The first group's populations carry the count (`initial-population` for a count,
//!   `numerator` for a percentage, rate or ratio, `measure-population` for a value) and the
//!   denominator, `measureScore` the figure, and [`VALUES`] the values' count, sum, minimum and
//!   maximum, so central recomputes the figure from summed parts.
//! - Each disaggregation is a stratifier (its name as `code.text`), each band a stratum (its code
//!   as `value.text`) with the same parts.
//! - Each crossing of two or more disaggregations (#557) is a stratifier naming them, in order,
//!   in its [`CROSSED`] extensions, each combination of bands something was counted in a stratum
//!   whose `component`s give its band of each (`code.text` the disaggregation, `value.text` the
//!   band), with the same parts. A crossing with no strata counted nothing.
//!
//! Decimals are written exactly, as JSON numbers.

use health_analytics::{
    Band, Calculation, CrossedCell, Crossing, Disaggregated, Period, PeriodType, PopulationKind,
    Summary, Values,
};
use rust_decimal::Decimal;
use serde_json::{Value, json};

/// On a `MeasureReport`: the type of its period (`valueCode`: `weekly`, `monthly`,
/// `quarterly`, `semiAnnual`, `annual` or `financialJuly`).
pub const PERIOD_TYPE: &str = "https://carapai.github.io/people-centered-healthcare/fhir/StructureDefinition/indicator-period-type";
/// On a group or stratum: the values a continuous-variable indicator took, with the
/// sub-extensions `count` (`valueInteger64`), `sum`, `minimum` and `maximum` (`valueDecimal`).
pub const VALUES: &str = "https://carapai.github.io/people-centered-healthcare/fhir/StructureDefinition/indicator-values";
/// The tag system naming a record's facility (ADR 0057).
pub const FACILITY_TAG: &str = "urn:health:facility";

/// On a stratifier: one of the disaggregations it crosses (`valueString`, its name), in order.
pub const CROSSED: &str = "https://carapai.github.io/people-centered-healthcare/fhir/StructureDefinition/indicator-crossed";

const POPULATION: &str = "http://terminology.hl7.org/CodeSystem/measure-population";

fn number(decimal: Decimal) -> Value {
    serde_json::from_str(&decimal.normalize().to_string()).unwrap_or(Value::Null)
}

fn decimal(value: &Value) -> Result<Decimal, String> {
    match value {
        Value::Number(n) => n
            .to_string()
            .parse::<Decimal>()
            .or_else(|_| Decimal::from_scientific(&n.to_string()))
            .map_err(|_| format!("{n} is not a decimal")),
        other => Err(format!("{other} is not a number")),
    }
}

fn population(kind: PopulationKind, count: u64) -> Value {
    json!({
        "code": { "coding": [{ "system": POPULATION, "code": kind.code() }] },
        "count": count,
    })
}

/// The population a summary's count is kept as.
fn counted(calculation: Calculation) -> PopulationKind {
    match calculation {
        Calculation::Count => PopulationKind::InitialPopulation,
        Calculation::Value(_) => PopulationKind::MeasurePopulation,
        Calculation::Percentage | Calculation::Rate { .. } | Calculation::Ratio => {
            PopulationKind::Numerator
        }
    }
}

/// The parts of `summary` as a group or a stratum holds them.
fn parts(summary: &Summary) -> serde_json::Map<String, Value> {
    let mut populations = vec![population(counted(summary.calculation), summary.count)];
    if let Some(denominator) = summary.denominator {
        populations.push(population(PopulationKind::Denominator, denominator));
    }
    let mut out = serde_json::Map::new();
    if let Some(values) = &summary.values {
        let mut parts = vec![json!({ "url": "count", "valueInteger64": values.count.to_string() })];
        for (name, part) in [
            ("sum", values.sum),
            ("minimum", values.minimum),
            ("maximum", values.maximum),
        ] {
            if let Some(part) = part {
                parts.push(json!({ "url": name, "valueDecimal": number(part) }));
            }
        }
        out.insert(
            "extension".into(),
            json!([{ "url": VALUES, "extension": parts }]),
        );
    }
    out.insert("population".into(), Value::Array(populations));
    if let Some(value) = summary.value {
        out.insert(
            "measureScoreQuantity".into(),
            json!({ "value": number(value) }),
        );
    }
    out
}

/// A crossing as a stratifier.
fn crossing_stratifier(crossing: &Crossing) -> Value {
    let strata: Vec<Value> = crossing
        .cells
        .iter()
        .map(|cell| {
            let mut stratum = parts(&cell.summary);
            stratum.insert(
                "component".into(),
                crossing
                    .names
                    .iter()
                    .zip(&cell.codes)
                    .map(|(name, code)| json!({ "code": { "text": name }, "value": { "text": code } }))
                    .collect(),
            );
            Value::Object(stratum)
        })
        .collect();
    json!({
        "extension": crossing
            .names
            .iter()
            .map(|name| json!({ "url": CROSSED, "valueString": name }))
            .collect::<Vec<_>>(),
        "code": { "text": crossing.names.join(" × ") },
        "stratum": strata,
    })
}

/// The crossing a stratifier written by [`crossing_stratifier`] holds, if it is one.
fn read_crossing(stratifier: &Value, calculation: Calculation) -> Result<Option<Crossing>, String> {
    let names: Vec<String> = stratifier["extension"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|e| e["url"] == CROSSED)
        .map(|e| {
            e["valueString"]
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| "a crossing names no disaggregation".to_owned())
        })
        .collect::<Result<_, _>>()?;
    if names.is_empty() {
        return Ok(None);
    }
    let mut cells = Vec::new();
    for stratum in stratifier["stratum"].as_array().into_iter().flatten() {
        let components = stratum["component"]
            .as_array()
            .map_or(&[][..], Vec::as_slice);
        let codes: Vec<String> = names
            .iter()
            .map(|name| {
                components
                    .iter()
                    .find(|c| c["code"]["text"] == name.as_str())
                    .and_then(|c| c["value"]["text"].as_str())
                    .map(str::to_owned)
                    .ok_or_else(|| format!("a crossed stratum has no band of {name}"))
            })
            .collect::<Result<_, _>>()?;
        cells.push(CrossedCell {
            codes,
            summary: summary_part(stratum, calculation)?,
        });
    }
    Ok(Some(Crossing { names, cells }))
}

/// `summary` of the indicator `measure` (`url|version`) for `period` at `facility`, computed at
/// `date` (an RFC 3339 instant), as a summary `MeasureReport`.
pub fn measure_report(
    measure: &str,
    period: &Period,
    facility: &str,
    summary: &Summary,
    date: &str,
) -> Value {
    let mut group = parts(summary);
    if !summary.disaggregations.is_empty() {
        let stratifiers: Vec<Value> = summary
            .disaggregations
            .iter()
            .map(|by| {
                let strata: Vec<Value> = by
                    .bands
                    .iter()
                    .map(|band| {
                        let mut stratum = parts(&band.summary);
                        stratum.insert("value".into(), json!({ "text": band.code }));
                        Value::Object(stratum)
                    })
                    .collect();
                json!({ "code": { "text": by.name }, "stratum": strata })
            })
            .chain(summary.crossed.iter().map(crossing_stratifier))
            .collect();
        group.insert("stratifier".into(), Value::Array(stratifiers));
    }
    let kind = serde_json::to_value(period.kind).unwrap_or(Value::Null);
    json!({
        "resourceType": "MeasureReport",
        "meta": { "tag": [{ "system": FACILITY_TAG, "code": facility }] },
        "extension": [{ "url": PERIOD_TYPE, "valueCode": kind }],
        "status": "complete",
        "type": "summary",
        "measure": measure,
        "date": date,
        "reporter": { "identifier": { "system": FACILITY_TAG, "value": facility } },
        "period": { "start": period.start.to_string(), "end": period.end.to_string() },
        "group": [group],
    })
}

/// The period a summary `MeasureReport` is for: its [`PERIOD_TYPE`] and its first day.
pub fn report_period(report: &Value) -> Result<Period, String> {
    let kind = report["extension"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|e| e["url"] == PERIOD_TYPE)
        .map(|e| e["valueCode"].clone())
        .ok_or("the MeasureReport has no period type")?;
    let kind: PeriodType =
        serde_json::from_value(kind).map_err(|_| "the period type is not one".to_owned())?;
    let start = report["period"]["start"]
        .as_str()
        .and_then(|s| s.parse().ok())
        .ok_or("the MeasureReport has no period start")?;
    Ok(Period::of(kind, start))
}

fn count_of(part: &Value, kind: PopulationKind) -> Result<Option<u64>, String> {
    let Some(found) = part["population"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|p| p["code"]["coding"][0]["code"] == kind.code())
    else {
        return Ok(None);
    };
    found["count"]
        .as_u64()
        .map(Some)
        .ok_or_else(|| format!("the {} has no count", kind.code()))
}

fn summary_part(part: &Value, calculation: Calculation) -> Result<Summary, String> {
    let count = count_of(part, counted(calculation))?.unwrap_or_default();
    let denominator = count_of(part, PopulationKind::Denominator)?;
    let values = match part["extension"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|e| e["url"] == VALUES)
    {
        None => None,
        Some(values) => {
            let sub = |name: &str| {
                values["extension"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .find(|e| e["url"] == name)
            };
            let decimal_of =
                |name: &str| sub(name).map(|e| decimal(&e["valueDecimal"])).transpose();
            Some(Values {
                count: sub("count")
                    .and_then(|e| e["valueInteger64"].as_str())
                    .and_then(|n| n.parse().ok())
                    .ok_or("the values have no count")?,
                sum: decimal_of("sum")?,
                minimum: decimal_of("minimum")?,
                maximum: decimal_of("maximum")?,
            })
        }
    };
    let value = match &part["measureScoreQuantity"]["value"] {
        Value::Null => None,
        value => Some(decimal(value)?),
    };
    Ok(Summary {
        calculation,
        value,
        count,
        denominator,
        values,
        disaggregations: Vec::new(),
        crossed: Vec::new(),
    })
}

/// The summary a `MeasureReport` written by [`measure_report`] holds, for an indicator that
/// calculates `calculation`.
pub fn report_summary(report: &Value, calculation: Calculation) -> Result<Summary, String> {
    let group = &report["group"][0];
    let mut summary = summary_part(group, calculation)?;
    for stratifier in group["stratifier"].as_array().into_iter().flatten() {
        if let Some(crossing) = read_crossing(stratifier, calculation)? {
            summary.crossed.push(crossing);
            continue;
        }
        let mut bands = Vec::new();
        for stratum in stratifier["stratum"].as_array().into_iter().flatten() {
            bands.push(Band {
                code: stratum["value"]["text"]
                    .as_str()
                    .ok_or("a stratum has no band")?
                    .to_owned(),
                summary: summary_part(stratum, calculation)?,
            });
        }
        summary.disaggregations.push(Disaggregated {
            name: stratifier["code"]["text"]
                .as_str()
                .ok_or("a stratifier has no name")?
                .to_owned(),
            bands,
        });
    }
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use health_analytics::Aggregate;

    use super::*;

    fn summary(calculation: Calculation, count: u64, denominator: Option<u64>) -> Summary {
        Summary {
            calculation,
            value: Some(Decimal::new(3333, 2)),
            count,
            denominator,
            values: None,
            disaggregations: Vec::new(),
            crossed: Vec::new(),
        }
    }

    #[test]
    fn a_summary_reads_back_from_its_measure_report() {
        let march = Period::parse(PeriodType::Monthly, "2026-03").expect("march");
        let mut percentage = summary(Calculation::Percentage, 1, Some(3));
        percentage.disaggregations = vec![Disaggregated {
            name: "age".into(),
            bands: vec![
                Band {
                    code: "15-19".into(),
                    summary: summary(Calculation::Percentage, 1, Some(2)),
                },
                Band {
                    code: "not-recorded".into(),
                    summary: Summary {
                        value: Some(Decimal::ZERO),
                        ..summary(Calculation::Percentage, 0, Some(1))
                    },
                },
            ],
        }];
        percentage.crossed = vec![
            Crossing {
                names: vec!["age".into(), "hiv-status".into()],
                cells: vec![CrossedCell {
                    codes: vec!["15-19".into(), "positive".into()],
                    summary: summary(Calculation::Percentage, 1, Some(2)),
                }],
            },
            // Nothing counted in any combination.
            Crossing {
                names: vec!["age".into(), "visit-type".into()],
                cells: Vec::new(),
            },
        ];
        let average = Summary {
            values: Some(Values {
                count: 4,
                sum: Some(Decimal::new(12345678901234567, 3)),
                minimum: Some(Decimal::new(105, 1)),
                maximum: Some(Decimal::new(4, 0)),
            }),
            ..summary(Calculation::Value(Aggregate::Average), 4, None)
        };
        for written in [percentage, average, summary(Calculation::Count, 7, None)] {
            let report = measure_report(
                "https://x/Measure/a|1.0.0",
                &march,
                "KAM013",
                &written,
                "2026-04-03T08:00:00Z",
            );
            assert_eq!(report["measure"], "https://x/Measure/a|1.0.0");
            assert_eq!(report["period"]["start"], "2026-03-01");
            assert_eq!(report["meta"]["tag"][0]["code"], "KAM013");
            assert_eq!(report_period(&report), Ok(march.clone()));
            // Central takes it as written, crossings included (#557).
            let sent = health_sync_protocol::IndicatorSummary {
                measure: "https://x/Measure/a|1.0.0".into(),
                period_type: "monthly".into(),
                period: "2026-03".into(),
                facility: "KAM013".into(),
                hash: "h".into(),
                report: Some(report.clone()),
            };
            assert_eq!(health_sync_protocol::summary_problem(&sent), None);
            assert_eq!(
                report_summary(&report, written.calculation).as_ref(),
                Ok(&written),
                "{report}"
            );
        }
    }
}

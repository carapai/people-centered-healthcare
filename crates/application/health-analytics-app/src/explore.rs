//! The data explorer (analytics.md § Dashboards; ADR 0075; #599): chosen indicators × places ×
//! a range of standard periods, each figure read from stored summary `MeasureReport`s, never
//! from patient-level records.
//!
//! - **Where.** At central for a district, region or the national place, over the summaries the
//!   facilities' edges synced ([`roll_up`]); on a Facility Edge Node for its own facility,
//!   offline, over the summaries it computed itself ([`OwnSummaries`]).
//! - **Places.** Each place is the facilities its figures roll up over: a facility is itself, a
//!   district everything in its area (`health_facility_registry::Places::facilities_in`). Which
//!   places a session may pick is the caller's: the session's place and those below it.
//! - **Periods.** Whole standard periods only (`Period::range`): each one is stored as it is,
//!   since every edge computes every standard period type.
//! - **Estimates** (#553; ADR 0071). An indicator whose denominator is a Population Estimate
//!   type is its place's summed count over the place's estimate for the period, pro rata, from
//!   the year's Population package ([`PopulationEstimates`]); a place below the figures given has
//!   *no estimate* and no figure, never a guessed share.
//! - **Completeness.** Each figure carries the facilities that reported and those missing with
//!   their last sync, never counted as zero; a median or a distinct count over more than one
//!   facility has no figure, and says why ([`Figure::FacilityOnly`]).

use std::collections::BTreeMap;

use health_analytics::{
    Aggregate, Calculation, DisaggregationSource, FacilitySummaries, FacilitySummary, Indicator,
    IndicatorStore, Period, PopulationEstimates, RolledUp, Summary, over_estimate,
};
use health_persistence::StoreError;
use jiff::Timestamp;
use rust_decimal::{Decimal, RoundingStrategy};
use serde_json::{Value, json};

use crate::roll_up;

/// A place picked, with the facilities its figures roll up over.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplorePlace {
    /// Its code.
    pub code: String,
    /// The codes of the facilities in it; a facility's is itself.
    pub facilities: Vec<String>,
}

/// One figure, or why there is none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Figure {
    /// Rolled up from the facilities' summaries, with its completeness.
    Rolled(Box<RolledUp>),
    /// A median or a distinct count over more than one facility: it shows at facility level
    /// only, since it cannot be rebuilt from the facilities' summaries.
    FacilityOnly(Aggregate),
}

/// One indicator's figure for one period and place.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    /// The indicator, `url|version`.
    pub indicator: String,
    /// The period's id.
    pub period: String,
    /// The place's code.
    pub place: String,
    /// Its figure.
    pub figure: Figure,
    /// The estimate its denominator is, for an indicator over a Population Estimate type.
    pub estimate: Option<Estimated>,
}

/// The Population Estimate a cell's denominator is (#553).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Estimated {
    /// The estimate type's code (`expected-pregnancies`).
    pub kind: String,
    /// What a person reads for it.
    pub title: String,
    /// The place's estimate for the period, pro rata; none when it has *no estimate*.
    pub value: Option<Decimal>,
}

/// Every `indicators` × `periods` × `places` figure from the summaries `store` keeps, indicator
/// by indicator, then period, then place. An indicator over a Population Estimate type takes
/// each place's from `estimates`.
pub async fn explore<S: FacilitySummaries>(
    store: &mut S,
    indicators: &[Indicator],
    periods: &[Period],
    places: &[ExplorePlace],
    estimates: &PopulationEstimates,
) -> Result<Vec<Cell>, String> {
    let mut cells = Vec::new();
    for indicator in indicators {
        let facility_only = match indicator.calculation {
            Calculation::Value(aggregate) if !aggregate.rolls_up() => Some(aggregate),
            _ => None,
        };
        for period in periods {
            for place in places {
                let figure = match facility_only {
                    Some(aggregate) if place.facilities.len() != 1 => {
                        Figure::FacilityOnly(aggregate)
                    }
                    _ => Figure::Rolled(Box::new(
                        roll_up(store, indicator, period, &place.facilities).await?,
                    )),
                };
                let (figure, estimate) =
                    over_estimates(indicator, period, place, figure, estimates);
                cells.push(Cell {
                    indicator: indicator.canonical(),
                    period: period.id.clone(),
                    place: place.code.clone(),
                    figure,
                    estimate,
                });
            }
        }
    }
    Ok(cells)
}

/// `figure` of `indicator` at `place` over `period`, its summed count over the place's estimate
/// when its denominator is a Population Estimate type, with that estimate.
fn over_estimates(
    indicator: &Indicator,
    period: &Period,
    place: &ExplorePlace,
    mut figure: Figure,
    estimates: &PopulationEstimates,
) -> (Figure, Option<Estimated>) {
    let Some(kind) = indicator.estimate.as_deref() else {
        return (figure, None);
    };
    let value = estimates.estimate(indicator, &place.code, period);
    if let Figure::Rolled(rolled) = &mut figure
        && let Some(summary) = rolled.summary.as_mut()
    {
        summary.value = over_estimate(indicator.calculation, summary.count, value);
    }
    let estimate = Estimated {
        kind: kind.to_owned(),
        title: estimates.title(kind, period),
        value,
    };
    (figure, Some(estimate))
}

/// A Facility Edge Node's own summaries, read as central reads synced ones: the facility has
/// always just synced, since it computed them itself, offline.
pub struct OwnSummaries<'a, S> {
    store: &'a mut S,
    facility: String,
    now: Timestamp,
}

impl<'a, S> OwnSummaries<'a, S> {
    /// `facility`'s summaries in `store`, as of `now`.
    pub fn new(store: &'a mut S, facility: &str, now: Timestamp) -> Self {
        Self {
            store,
            facility: facility.to_owned(),
            now,
        }
    }
}

impl<S: IndicatorStore + Send> FacilitySummaries for OwnSummaries<'_, S> {
    async fn facility_summaries(
        &mut self,
        measure: &str,
        period: &Period,
    ) -> Result<Vec<FacilitySummary>, StoreError> {
        let now = self.now;
        Ok(self
            .store
            .summaries(period.kind, &period.id)
            .await?
            .into_iter()
            .filter(|s| s.measure == measure && s.facility == self.facility)
            .map(|s| {
                let computed_at = serde_json::from_str::<Value>(&s.report)
                    .ok()
                    .and_then(|r| r["date"].as_str().and_then(|d| d.parse().ok()))
                    .unwrap_or(now);
                FacilitySummary {
                    facility: s.facility,
                    report: s.report,
                    computed_at,
                }
            })
            .collect())
    }

    async fn summaries_synced(&mut self) -> Result<BTreeMap<String, Timestamp>, StoreError> {
        Ok(BTreeMap::from([(self.facility.clone(), self.now)]))
    }
}

/// What an indicator calculates, as the explorer names it: `count`, `percentage`, `rate`,
/// `ratio`, or a value's aggregate (`sum`, `average`, `median`, …).
pub fn calculation_code(calculation: Calculation) -> &'static str {
    match calculation {
        Calculation::Count => "count",
        Calculation::Percentage => "percentage",
        Calculation::Rate { .. } => "rate",
        Calculation::Ratio => "ratio",
        Calculation::Value(aggregate) => aggregate.code(),
    }
}

/// An indicator as the explorer lists it, with its threshold bands lowest first, each bound
/// exact decimal text (#600).
pub fn indicator_json(indicator: &Indicator) -> Value {
    let per = match indicator.calculation {
        Calculation::Rate { per } => Some(per),
        _ => None,
    };
    json!({
        "measure": indicator.canonical(),
        "title": indicator.title,
        "calculation": calculation_code(indicator.calculation),
        "per": per,
        "disaggregations": indicator
            .disaggregations
            .iter()
            .map(|d| json!({
                "name": d.name,
                "by": match d.source {
                    DisaggregationSource::AgeBands(_) => "age",
                    DisaggregationSource::Coded(_) => "code",
                },
            }))
            .collect::<Vec<_>>(),
        "denominatorEstimate": indicator.estimate,
        "rollsUp": !matches!(
            indicator.calculation,
            Calculation::Value(aggregate) if !aggregate.rolls_up()
        ),
        "thresholds": indicator
            .thresholds
            .iter()
            .map(|band| json!({
                "from": band.from.map(|v| v.normalize().to_string()),
                "to": band.to.map(|v| v.normalize().to_string()),
                "tone": band.tone.code(),
            }))
            .collect::<Vec<_>>(),
    })
}

fn figure_text(summary: &Summary) -> Value {
    json!(summary.value.map(|v| v.normalize().to_string()))
}

/// `cells` as the explorer reads them, naming each missing facility from `names`. A figure is
/// exact decimal text. Split by the disaggregation named `by` (#601), each rolled-up figure also
/// carries its `bands`, in the summary's order (an Age Band Set's, or the codes found, then
/// *not recorded*): each band's figure recomputed from its own summed parts, never an average.
pub fn cells_json(cells: &[Cell], names: &BTreeMap<String, String>, by: Option<&str>) -> Value {
    let name = |code: &str| names.get(code).cloned().unwrap_or_else(|| code.to_owned());
    let cells: Vec<Value> = cells
        .iter()
        .map(|cell| {
            let mut out = json!({
                "indicator": cell.indicator,
                "period": cell.period,
                "place": cell.place,
            });
            match &cell.figure {
                Figure::FacilityOnly(aggregate) => {
                    out["value"] = Value::Null;
                    out["facilityOnly"] = json!(aggregate.code());
                }
                Figure::Rolled(rolled) => {
                    let (reported, expected) = rolled.completeness();
                    out["value"] = rolled.summary.as_ref().map_or(Value::Null, figure_text);
                    if let Some(by) = by {
                        out["bands"] = rolled
                            .summary
                            .iter()
                            .flat_map(|s| &s.disaggregations)
                            .filter(|d| d.name == by)
                            .flat_map(|d| &d.bands)
                            .map(|b| json!({ "code": b.code, "value": figure_text(&b.summary) }))
                            .collect();
                    }
                    if cell.estimate.is_some() {
                        out["count"] = json!(rolled.summary.as_ref().map(|s| s.count));
                    }
                    out["reported"] = json!(reported);
                    out["expected"] = json!(expected);
                    out["computedAt"] = json!(rolled.computed_at.map(|t| t.to_string()));
                    out["missing"] = rolled
                        .missing
                        .iter()
                        .map(|m| {
                            json!({
                                "facility": m.facility,
                                "name": name(&m.facility),
                                "lastSync": m.last_sync.map(|t| t.to_string()),
                            })
                        })
                        .collect();
                }
            }
            if let Some(estimate) = &cell.estimate {
                out["estimate"] = json!({
                    "type": estimate.kind,
                    "title": estimate.title,
                    "value": estimate.value.map(|v| v
                        .round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero)
                        .normalize()
                        .to_string()),
                });
            }
            out
        })
        .collect();
    Value::Array(cells)
}

/// The most indicators, places and figures one question may ask for.
pub const MOST_INDICATORS: usize = 12;
/// See [`MOST_INDICATORS`].
pub const MOST_PLACES: usize = 60;
/// See [`MOST_INDICATORS`].
pub const MOST_CELLS: usize = 6000;

/// What the explorer asks: `{"indicators": ["url|version"], "periodType": "monthly", "from":
/// "2026-01", "to": "2026-06", "places": ["district-kampala"]}`, optionally split by one
/// disaggregation every indicator asked has: `"disaggregation": "age"` (#601).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Question {
    /// The indicators, from those offered, in the order asked.
    pub indicators: Vec<Indicator>,
    /// The periods, first to last.
    pub periods: Vec<Period>,
    /// The places' codes, in the order asked, each once; whether the session may see them is
    /// the caller's to check.
    pub places: Vec<String>,
    /// The disaggregation each figure is split by, by name; every indicator has it.
    pub disaggregation: Option<String>,
}

/// Reads `body` as a [`Question`] over the indicators `offered`, or says why it cannot be asked.
pub fn read_question(body: &Value, offered: &[Indicator]) -> Result<Question, String> {
    let texts = |name: &str| -> Result<Vec<String>, String> {
        let mut out: Vec<String> = Vec::new();
        for item in body[name]
            .as_array()
            .ok_or(format!("`{name}` is not a list"))?
        {
            let text = item.as_str().ok_or(format!("`{name}` holds a non-text"))?;
            if !out.iter().any(|t| t == text) {
                out.push(text.to_owned());
            }
        }
        Ok(out)
    };
    let measures = texts("indicators")?;
    let places = texts("places")?;
    if measures.is_empty() || places.is_empty() {
        return Err("pick at least one indicator and one place".into());
    }
    if measures.len() > MOST_INDICATORS || places.len() > MOST_PLACES {
        return Err(format!(
            "pick at most {MOST_INDICATORS} indicators and {MOST_PLACES} places"
        ));
    }
    let indicators = measures
        .iter()
        .map(|m| {
            offered
                .iter()
                .find(|i| i.canonical() == *m)
                .cloned()
                .ok_or(format!("{m} is not an indicator offered here"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let disaggregation = match &body["disaggregation"] {
        Value::Null => None,
        Value::String(name) => {
            if let Some(lacking) = indicators
                .iter()
                .find(|i| !i.disaggregations.iter().any(|d| d.name == *name))
            {
                return Err(format!("{} is not disaggregated by {name}", lacking.title));
            }
            Some(name.clone())
        }
        _ => return Err("`disaggregation` is not a name".into()),
    };
    let kind = serde_json::from_value(body["periodType"].clone())
        .map_err(|_| "`periodType` is not a period type".to_owned())?;
    let end = |name: &str| {
        body[name]
            .as_str()
            .ok_or(format!("`{name}` is not a period"))
    };
    let periods = Period::range(kind, end("from")?, end("to")?).map_err(|e| e.to_string())?;
    if indicators.len() * periods.len() * places.len() > MOST_CELLS {
        return Err(format!(
            "that is more than {MOST_CELLS} figures: ask for fewer"
        ));
    }
    Ok(Question {
        indicators,
        periods,
        places,
        disaggregation,
    })
}

/// `periods` as the explorer labels them: each one's id and first and last days.
pub fn periods_json(periods: &[Period]) -> Value {
    periods
        .iter()
        .map(|p| json!({ "id": p.id, "start": p.start.to_string(), "end": p.end.to_string() }))
        .collect()
}

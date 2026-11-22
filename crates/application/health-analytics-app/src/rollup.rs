//! Roll-up at central (analytics.md § Roll up; ADR 0068, ADR 0075; #555): an indicator's figure
//! for a set of facilities from the summary `MeasureReport`s their edges synced, read back
//! ([`crate::report::report_summary`]) and rolled up from their summed parts
//! (`health_analytics::roll_up_facilities`), with the facilities that have not reported shown as
//! missing, never as zero.
//!
//! Which facilities make up a district, region or the country is the facility registry's
//! (ADR 0094); the caller gives them.

use std::collections::BTreeMap;

use health_analytics::{FacilitySummaries, Indicator, Period, RolledUp, roll_up_facilities};
use serde_json::Value;

use crate::report::report_summary;

/// `indicator`'s figure for `period` over `facilities`, from the summaries `store` keeps.
pub async fn roll_up<S: FacilitySummaries>(
    store: &mut S,
    indicator: &Indicator,
    period: &Period,
    facilities: &[String],
) -> Result<RolledUp, String> {
    let measure = indicator.canonical();
    let mut summaries = BTreeMap::new();
    for kept in store
        .facility_summaries(&measure, period)
        .await
        .map_err(|e| e.to_string())?
    {
        if !facilities.contains(&kept.facility) {
            continue;
        }
        let report: Value = serde_json::from_str(&kept.report)
            .map_err(|_| format!("{}'s summary of {measure} is not JSON", kept.facility))?;
        let summary = report_summary(&report, indicator.calculation)
            .map_err(|why| format!("{}'s summary of {measure}: {why}", kept.facility))?;
        summaries.insert(kept.facility, (summary, kept.computed_at));
    }
    let synced = store.summaries_synced().await.map_err(|e| e.to_string())?;
    roll_up_facilities(
        indicator.calculation,
        period,
        facilities,
        &summaries,
        &synced,
    )
    .map_err(|e| e.to_string())
}

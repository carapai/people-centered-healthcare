//! Indicators computed on a node as records change (analytics.md § Compute; ADR 0068; #554).
//!
//! - **Who changed.** Each change on the node's change log concerns the Patient it is, or the
//!   Patients the version and the one before it reference ([`Indicators::patients_changed`]), so
//!   a record moved to another patient, or deleted, updates both.
//! - **Membership.** Each changed patient is evaluated again, alone: for each indicator, its
//!   records' period dates place care in periods of each period type the node computes
//!   (`health_analytics::periods_with_care`), and the patient's membership is kept for exactly
//!   those, evaluated over each. A visit entered in April for a March date changes March's
//!   membership, not April's. A membership in a period the patient no longer has care in is
//!   forgotten.
//! - **Summaries.** Each period whose membership changed is calculated again from every
//!   patient's kept membership and stored as a summary `MeasureReport` ([`crate::report`]), or
//!   forgotten when no patient has care in it any more, so the incremental path gives what
//!   evaluating every period afresh gives ([`Indicators::recompute`]).
//! - **This facility's care.** A record of an indicator's basis tagged with another facility
//!   (`urn:health:facility`) is not counted here: that facility counts it. Untagged records, and
//!   every record criteria read through `%records`, are this node's.
//!
//! An indicator that cannot be evaluated (a pinned Age Band Set or a referred indicator the node
//! does not hold) is left out, and said so in [`Refreshed::problems`].

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use health_analytics::{
    Indicator, IndicatorStore, PatientOutcome, Period, PeriodType, StoredSummary, calculate,
    periods_with_care,
};
use health_fhir_validation::FhirPathEngine;
use health_persistence::{Store, Transaction};
use serde_json::Value;

use crate::report::{FACILITY_TAG, measure_report};
use crate::{Library, PatientRecords, outcomes};

/// What a refresh did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Refreshed {
    /// How many patients were evaluated again.
    pub patients: usize,
    /// How many summaries were calculated again.
    pub summaries: usize,
    /// Each indicator that could not be evaluated, with why.
    pub problems: BTreeSet<String>,
}

/// The indicators a node computes, and for which period types.
#[derive(Clone)]
pub struct Indicators {
    engine: Arc<dyn FhirPathEngine>,
    library: Library,
    indicators: Vec<Indicator>,
    kinds: Vec<PeriodType>,
    facility: String,
}

impl std::fmt::Debug for Indicators {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Indicators")
            .field("indicators", &self.indicators.len())
            .field("kinds", &self.kinds)
            .field("facility", &self.facility)
            .finish_non_exhaustive()
    }
}

fn failed(error: impl std::fmt::Display) -> String {
    error.to_string()
}

/// Every `Patient/<id>` reference in `value`, by id.
fn patients_referenced(value: &Value, found: &mut BTreeSet<String>) {
    match value {
        Value::Object(map) => {
            for (key, inner) in map {
                if key == "reference"
                    && let Some(id) = inner.as_str().and_then(|r| r.strip_prefix("Patient/"))
                    && !id.is_empty()
                    && !id.contains('/')
                {
                    found.insert(id.to_owned());
                }
                patients_referenced(inner, found);
            }
        }
        Value::Array(items) => items.iter().for_each(|i| patients_referenced(i, found)),
        _ => {}
    }
}

fn facility_of(resource: &Value) -> Option<&str> {
    resource["meta"]["tag"]
        .as_array()?
        .iter()
        .find(|t| t["system"] == FACILITY_TAG)?["code"]
        .as_str()
}

impl Indicators {
    /// `indicators` (with the indicators and Age Band Sets they refer to in `library`), computed
    /// for each period type of `kinds` at `facility`, on `engine`.
    pub fn new(
        engine: Arc<dyn FhirPathEngine>,
        library: Library,
        indicators: Vec<Indicator>,
        kinds: Vec<PeriodType>,
        facility: impl Into<String>,
    ) -> Self {
        Self {
            engine,
            library,
            indicators,
            kinds,
            facility: facility.into(),
        }
    }

    /// The indicators computed.
    pub fn indicators(&self) -> &[Indicator] {
        &self.indicators
    }

    /// The Age Band Sets they may pin: what a Report run here labels its bands with (#557).
    pub fn age_band_sets(&self) -> &[health_analytics::AgeBandSet] {
        self.library.age_band_sets()
    }

    /// The facility they are computed for.
    pub fn facility(&self) -> &str {
        &self.facility
    }

    /// The ids of the Patients that `changes` (each a resource's type, id and version) concern:
    /// the Patient itself, or those its changed version and the one before reference.
    pub async fn patients_changed<S>(
        store: &S,
        changes: &[(String, String, u64)],
    ) -> Result<BTreeSet<String>, String>
    where
        S: Store,
        S::Tx: IndicatorStore,
    {
        let mut found = BTreeSet::new();
        let mut tx = store.begin().await.map_err(failed)?;
        for (resource_type, id, version) in changes {
            if resource_type == "Patient" {
                found.insert(id.clone());
                continue;
            }
            for json in tx
                .changed_versions(resource_type, id, *version)
                .await
                .map_err(failed)?
            {
                if let Ok(value) = serde_json::from_str::<Value>(&json) {
                    patients_referenced(&value, &mut found);
                }
            }
        }
        Ok(found)
    }

    /// The patient `id` and their records, if they are here.
    async fn records<T: IndicatorStore>(
        tx: &mut T,
        id: &str,
    ) -> Result<Option<PatientRecords>, String> {
        let Some(mut json) = tx.patient_records(id).await.map_err(failed)? else {
            return Ok(None);
        };
        if json.is_empty() {
            return Ok(None);
        }
        let patient = serde_json::from_str(&json.remove(0)).map_err(failed)?;
        let records = json
            .iter()
            .filter_map(|r| serde_json::from_str(r).ok())
            .collect();
        Ok(Some(PatientRecords { patient, records }))
    }

    /// The patient's outcome over `period`, leaving out records of the basis another facility
    /// counts.
    async fn outcome(
        &self,
        indicator: &Indicator,
        period: &Period,
        patient: &PatientRecords,
    ) -> Result<PatientOutcome, String> {
        let elsewhere: BTreeSet<String> = patient
            .records
            .iter()
            .filter(|r| r["resourceType"] == indicator.basis.as_str())
            .filter(|r| facility_of(r).is_some_and(|f| f != self.facility))
            .map(|r| {
                format!(
                    "{}/{}",
                    r["resourceType"].as_str().unwrap_or_default(),
                    r["id"].as_str().unwrap_or_default()
                )
            })
            .collect();
        let mut outcome = outcomes(
            self.engine.as_ref(),
            &self.library,
            indicator,
            period,
            std::slice::from_ref(patient),
        )
        .await?
        .pop()
        .ok_or("no outcome")?;
        outcome
            .records
            .retain(|r| !elsewhere.contains(&r.reference));
        Ok(outcome)
    }

    /// Evaluates each of `patients` (Patient ids) again, keeping their membership in each
    /// indicator for the periods they have care in, then calculates every period whose
    /// membership changed again. `now` is the facility's local time: a cumulative indicator
    /// is kept up to its period.
    pub async fn refresh<S>(
        &self,
        store: &S,
        patients: &BTreeSet<String>,
        now: &jiff::Zoned,
    ) -> Result<Refreshed, String>
    where
        S: Store,
        S::Tx: IndicatorStore,
    {
        let today = now.date();
        let mut done = Refreshed::default();
        let mut touched: BTreeSet<(usize, PeriodType, String)> = BTreeSet::new();
        let mut periods: BTreeMap<(PeriodType, String), Period> = BTreeMap::new();
        for id in patients {
            let reference = format!("Patient/{id}");
            let mut tx = store.begin().await.map_err(failed)?;
            let patient = Self::records(&mut tx, id).await?;
            let kept = tx.memberships_of(&reference).await.map_err(failed)?;
            for (at, indicator) in self.indicators.iter().enumerate() {
                let measure = indicator.canonical();
                let mut now_in: BTreeSet<(PeriodType, String)> = BTreeSet::new();
                if let Some(patient) = &patient {
                    let probe = Period::of(PeriodType::Monthly, today);
                    let placed = match self.outcome(indicator, &probe, patient).await {
                        Ok(placed) => placed,
                        Err(why) => {
                            done.problems.insert(format!("{measure}: {why}"));
                            continue;
                        }
                    };
                    for kind in &self.kinds {
                        for period in periods_with_care(indicator, *kind, &placed, today) {
                            let outcome = self.outcome(indicator, &period, patient).await?;
                            tx.put_member(&measure, &period, &outcome)
                                .await
                                .map_err(failed)?;
                            now_in.insert((period.kind, period.id.clone()));
                            touched.insert((at, period.kind, period.id.clone()));
                            periods.insert((period.kind, period.id.clone()), period);
                        }
                    }
                }
                for key in kept.iter().filter(|k| k.measure == measure) {
                    if now_in.contains(&(key.kind, key.period.clone())) {
                        continue;
                    }
                    let Ok(period) = Period::parse(key.kind, &key.period) else {
                        continue;
                    };
                    tx.remove_member(&measure, &period, &reference)
                        .await
                        .map_err(failed)?;
                    touched.insert((at, period.kind, period.id.clone()));
                    periods.insert((period.kind, period.id.clone()), period);
                }
            }
            tx.commit().await.map_err(failed)?;
            done.patients += 1;
        }
        let date = now.timestamp().to_string();
        let mut tx = store.begin().await.map_err(failed)?;
        for (at, kind, id) in &touched {
            let indicator = &self.indicators[*at];
            let period = &periods[&(*kind, id.clone())];
            let measure = indicator.canonical();
            let members = tx.members(&measure, period).await.map_err(failed)?;
            if members.is_empty() {
                tx.remove_summary(&measure, period, &self.facility)
                    .await
                    .map_err(failed)?;
                done.summaries += 1;
                continue;
            }
            let summary =
                calculate(indicator, period, &members, self.library.age_band_sets()).summary;
            let report = measure_report(&measure, period, &self.facility, &summary, &date);
            tx.put_summary(&StoredSummary {
                measure,
                period: period.clone(),
                facility: self.facility.clone(),
                report: report.to_string(),
            })
            .await
            .map_err(failed)?;
            done.summaries += 1;
        }
        tx.commit().await.map_err(failed)?;
        Ok(done)
    }

    /// Computes everything again from scratch: every membership and summary is forgotten, and
    /// every patient evaluated again. What the incremental path must always agree with, and what
    /// a node runs when its indicators, its change log's epoch or the day changes.
    pub async fn recompute<S>(&self, store: &S, now: &jiff::Zoned) -> Result<Refreshed, String>
    where
        S: Store,
        S::Tx: IndicatorStore,
    {
        let mut tx = store.begin().await.map_err(failed)?;
        tx.clear_indicators().await.map_err(failed)?;
        let patients: BTreeSet<String> = tx
            .patient_ids()
            .await
            .map_err(failed)?
            .into_iter()
            .collect();
        tx.commit().await.map_err(failed)?;
        self.refresh(store, &patients, now).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_change_concerns_every_patient_it_references() {
        let mut found = BTreeSet::new();
        patients_referenced(
            &serde_json::json!({
                "resourceType": "Encounter",
                "subject": { "reference": "Patient/a" },
                "participant": [{ "actor": { "reference": "Practitioner/x" } }],
                "partOf": { "reference": "Patient/b/_history/2" },
                "contained": [{ "patient": { "reference": "Patient/c" } }],
            }),
            &mut found,
        );
        assert_eq!(found, BTreeSet::from(["a".to_owned(), "c".to_owned()]));
    }
}

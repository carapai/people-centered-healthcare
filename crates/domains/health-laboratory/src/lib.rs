//! The laboratory workflow (resource-ownership.md: Specimen, laboratory Observations and
//! DiagnosticReports are laboratory's; #109). The order is a ServiceRequest the orders side owns;
//! laboratory follows it as a [`LabTest`]:
//!
//! ```text
//! ordered → collected → resulted → verified → reviewed
//!              ↓            ↺ (corrected before verification)
//!           rejected (specimen unsatisfactory)      cancelled (before results)
//! ```
//!
//! - **Results** ([`ResultEntry`]) are flagged against their reference and critical ranges, in
//!   exact decimals ([`flag`]). A coded or text result carries the flag the analyst gives it.
//! - **Verification** is a second person's: whoever entered the results cannot verify them.
//! - **Release** ([`LabTest::released`]): the clinician sees results only once verified. A
//!   verified test with a critical result is routed to the clinician's inbox as urgent
//!   ([`LabEvent::Verified`] carries `critical`).
//! - **Review** by the clinician closes the test.

use std::future::Future;
use std::str::FromStr;

use health_persistence::{StoreError, WriteError};
use health_time::Timestamp;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// A coded concept: what a person reads, and its code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Coded {
    /// What a person reads.
    pub text: String,
    /// The code system.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    /// The code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

/// Where a test is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Stage {
    /// Ordered; no specimen yet.
    Ordered,
    /// The specimen was collected.
    Collected,
    /// Results are entered, not yet verified.
    Resulted,
    /// Results are verified and released to the clinician.
    Verified,
    /// The clinician has reviewed them.
    Reviewed,
    /// The specimen was unsatisfactory; the test must be ordered again.
    Rejected,
    /// Withdrawn before results.
    Cancelled,
}

impl Stage {
    /// Its code.
    pub fn code(self) -> &'static str {
        match self {
            Self::Ordered => "ordered",
            Self::Collected => "collected",
            Self::Resulted => "resulted",
            Self::Verified => "verified",
            Self::Reviewed => "reviewed",
            Self::Rejected => "rejected",
            Self::Cancelled => "cancelled",
        }
    }
}

/// How a result compares with its ranges (FHIR `interpretation`, v3 ObservationInterpretation).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Flag {
    /// Within the reference range (`N`).
    Normal,
    /// Below it (`L`).
    Low,
    /// Above it (`H`).
    High,
    /// Below the critical limit (`LL`).
    CriticalLow,
    /// Above the critical limit (`HH`).
    CriticalHigh,
    /// Abnormal, for a coded or text result (`A`).
    Abnormal,
    /// Critically abnormal, for a coded or text result (`AA`).
    Critical,
}

impl Flag {
    /// The v3 ObservationInterpretation code.
    pub fn code(self) -> &'static str {
        match self {
            Self::Normal => "N",
            Self::Low => "L",
            Self::High => "H",
            Self::CriticalLow => "LL",
            Self::CriticalHigh => "HH",
            Self::Abnormal => "A",
            Self::Critical => "AA",
        }
    }

    /// Whether it is critical: the clinician is told at once.
    pub fn is_critical(self) -> bool {
        matches!(
            self,
            Self::CriticalLow | Self::CriticalHigh | Self::Critical
        )
    }
}

/// A range, either end open; decimals as exact text.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Range {
    /// Low limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low: Option<String>,
    /// High limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub high: Option<String>,
}

/// A result's value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum ResultValue {
    /// A measured quantity: exact decimal text in its scale, and its unit.
    Quantity {
        /// The value (`7.4`).
        value: String,
        /// Its unit (`g/dL`).
        unit: String,
    },
    /// A coded finding (`Positive`).
    Coded {
        /// The finding.
        concept: Coded,
    },
    /// Free text.
    Text {
        /// The text.
        text: String,
    },
}

/// One result as the analyst enters it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultEntry {
    /// What was measured.
    pub code: Coded,
    /// The value.
    pub value: ResultValue,
    /// The reference range, for a quantity.
    #[serde(default)]
    pub reference: Option<Range>,
    /// The critical limits, for a quantity.
    #[serde(default)]
    pub critical: Option<Range>,
    /// The analyst's flag, for a coded or text result (none: normal).
    #[serde(default)]
    pub flag: Option<Flag>,
}

/// A result as kept: the entry and its flag.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LabResult {
    /// The entry.
    pub entry: ResultEntry,
    /// Its flag.
    pub flag: Flag,
}

fn decimal(text: &str) -> Result<Decimal, LabRefused> {
    Decimal::from_str(text.trim()).map_err(|_| LabRefused::NotANumber(text.to_owned()))
}

/// The flag of `entry`: a quantity against its critical limits first, then its reference range;
/// a coded or text result as the analyst gave it.
pub fn flag(entry: &ResultEntry) -> Result<Flag, LabRefused> {
    let ResultValue::Quantity { value, .. } = &entry.value else {
        return Ok(match entry.flag {
            Some(Flag::Abnormal | Flag::Low | Flag::High) => Flag::Abnormal,
            Some(f) if f.is_critical() => Flag::Critical,
            _ => Flag::Normal,
        });
    };
    let value = decimal(value)?;
    let below = |range: &Option<Range>| -> Result<bool, LabRefused> {
        match range.as_ref().and_then(|r| r.low.as_deref()) {
            Some(low) => Ok(value < decimal(low)?),
            None => Ok(false),
        }
    };
    let above = |range: &Option<Range>| -> Result<bool, LabRefused> {
        match range.as_ref().and_then(|r| r.high.as_deref()) {
            Some(high) => Ok(value > decimal(high)?),
            None => Ok(false),
        }
    };
    Ok(if below(&entry.critical)? {
        Flag::CriticalLow
    } else if above(&entry.critical)? {
        Flag::CriticalHigh
    } else if below(&entry.reference)? {
        Flag::Low
    } else if above(&entry.reference)? {
        Flag::High
    } else {
        Flag::Normal
    })
}

/// Who did a step, and when.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Done {
    /// Who.
    pub by: String,
    /// When.
    pub at: Timestamp,
}

/// The specimen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Specimen {
    /// Its id (a UUIDv7): the FHIR Specimen's.
    pub id: String,
    /// What it is (`Venous blood`).
    pub kind: Coded,
    /// Who collected it, and when.
    pub collected: Done,
}

/// A test, from its order to its review.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LabTest {
    /// Its id: the ServiceRequest's.
    pub id: String,
    /// The patient, `Patient/{id}`.
    pub patient: String,
    /// The Encounter it was ordered in, if any.
    pub encounter: Option<String>,
    /// The facility.
    pub facility: String,
    /// What was ordered.
    pub test: Coded,
    /// Who ordered it, and when.
    pub ordered: Done,
    /// Where it is.
    pub stage: Stage,
    /// The specimen, once collected.
    pub specimen: Option<Specimen>,
    /// The results, once entered.
    pub results: Vec<LabResult>,
    /// Who entered the results, and when.
    pub resulted: Option<Done>,
    /// Who verified them, and when.
    pub verified: Option<Done>,
    /// Who reviewed them, and when.
    pub reviewed: Option<Done>,
    /// Why the specimen was rejected, or the test cancelled.
    pub reason: Option<String>,
}

/// What happened to a test, on the outbox.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum LabEvent {
    /// The specimen was collected.
    Collected {
        /// The test.
        test: String,
        /// When.
        at: Timestamp,
    },
    /// Results were entered or corrected.
    Resulted {
        /// The test.
        test: String,
        /// When.
        at: Timestamp,
    },
    /// The results were verified and released to the clinician; urgently if `critical`.
    Verified {
        /// The test.
        test: String,
        /// The patient.
        patient: String,
        /// Whether any result is critical.
        critical: bool,
        /// When.
        at: Timestamp,
    },
    /// The clinician reviewed the results.
    Reviewed {
        /// The test.
        test: String,
        /// When.
        at: Timestamp,
    },
    /// The specimen was rejected, or the test cancelled.
    Closed {
        /// The test.
        test: String,
        /// Its stage now.
        stage: Stage,
        /// When.
        at: Timestamp,
    },
}

impl LabEvent {
    /// The event's name on the outbox.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Collected { .. } => "laboratory.collected.v1",
            Self::Resulted { .. } => "laboratory.resulted.v1",
            Self::Verified { .. } => "laboratory.verified.v1",
            Self::Reviewed { .. } => "laboratory.reviewed.v1",
            Self::Closed { .. } => "laboratory.closed.v1",
        }
    }
}

/// Why a laboratory step is refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LabRefused {
    /// The step does not fit where the test is.
    #[error("a test that is {stage} cannot be {step}")]
    Step {
        /// Where the test is.
        stage: &'static str,
        /// The step.
        step: &'static str,
    },
    /// Whoever entered the results cannot verify them.
    #[error("results are verified by someone other than who entered them")]
    SameVerifier,
    /// No results, or a blank one.
    #[error("enter at least one result, each with a value")]
    NoResults,
    /// A quantity or limit that is not a decimal number.
    #[error("{0:?} is not a number")]
    NotANumber(String),
    /// Missing what the step needs.
    #[error("the step needs {0}")]
    Missing(&'static str),
}

fn refused(stage: Stage, step: &'static str) -> LabRefused {
    LabRefused::Step {
        stage: stage.code(),
        step,
    }
}

fn blank(text: &str) -> bool {
    text.trim().is_empty()
}

/// What an order names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Order {
    /// The ServiceRequest's id.
    pub id: String,
    /// The patient.
    pub patient: String,
    /// The Encounter, if any.
    pub encounter: Option<String>,
    /// The facility.
    pub facility: String,
    /// What was ordered.
    pub test: Coded,
    /// Who ordered it, and when.
    pub ordered: Done,
}

/// A test for `order`, awaiting its specimen.
pub fn order(order: Order) -> Result<LabTest, LabRefused> {
    if blank(&order.id)
        || blank(&order.patient)
        || blank(&order.facility)
        || blank(&order.test.text)
    {
        return Err(LabRefused::Missing(
            "an order, a patient, a facility and a test",
        ));
    }
    Ok(LabTest {
        id: order.id,
        patient: order.patient,
        encounter: order.encounter,
        facility: order.facility,
        test: order.test,
        ordered: order.ordered,
        stage: Stage::Ordered,
        specimen: None,
        results: Vec::new(),
        resulted: None,
        verified: None,
        reviewed: None,
        reason: None,
    })
}

impl LabTest {
    /// The specimen collected.
    pub fn collect(&self, specimen: Specimen) -> Result<(Self, LabEvent), LabRefused> {
        if self.stage != Stage::Ordered {
            return Err(refused(self.stage, "collected"));
        }
        if blank(&specimen.id) || blank(&specimen.kind.text) {
            return Err(LabRefused::Missing("the specimen and what it is"));
        }
        let at = specimen.collected.at;
        let next = Self {
            stage: Stage::Collected,
            specimen: Some(specimen),
            ..self.clone()
        };
        Ok((
            next,
            LabEvent::Collected {
                test: self.id.clone(),
                at,
            },
        ))
    }

    /// Results entered, or corrected before verification.
    pub fn result(
        &self,
        entries: Vec<ResultEntry>,
        done: Done,
    ) -> Result<(Self, LabEvent), LabRefused> {
        if !matches!(self.stage, Stage::Collected | Stage::Resulted) {
            return Err(refused(self.stage, "resulted"));
        }
        let empty = |e: &ResultEntry| match &e.value {
            ResultValue::Quantity { value, .. } => blank(value),
            ResultValue::Coded { concept } => blank(&concept.text),
            ResultValue::Text { text } => blank(text),
        };
        if entries.is_empty() || entries.iter().any(|e| empty(e) || blank(&e.code.text)) {
            return Err(LabRefused::NoResults);
        }
        let results = entries
            .into_iter()
            .map(|entry| flag(&entry).map(|flag| LabResult { entry, flag }))
            .collect::<Result<Vec<_>, _>>()?;
        let at = done.at;
        let next = Self {
            stage: Stage::Resulted,
            results,
            resulted: Some(done),
            ..self.clone()
        };
        Ok((
            next,
            LabEvent::Resulted {
                test: self.id.clone(),
                at,
            },
        ))
    }

    /// The results verified by a second person, and released to the clinician.
    pub fn verify(&self, done: Done) -> Result<(Self, LabEvent), LabRefused> {
        if self.stage != Stage::Resulted {
            return Err(refused(self.stage, "verified"));
        }
        if self.resulted.as_ref().is_some_and(|r| r.by == done.by) {
            return Err(LabRefused::SameVerifier);
        }
        let at = done.at;
        let next = Self {
            stage: Stage::Verified,
            verified: Some(done),
            ..self.clone()
        };
        let event = LabEvent::Verified {
            test: self.id.clone(),
            patient: self.patient.clone(),
            critical: next.is_critical(),
            at,
        };
        Ok((next, event))
    }

    /// The results reviewed by the clinician.
    pub fn review(&self, done: Done) -> Result<(Self, LabEvent), LabRefused> {
        if self.stage != Stage::Verified {
            return Err(refused(self.stage, "reviewed"));
        }
        let at = done.at;
        let next = Self {
            stage: Stage::Reviewed,
            reviewed: Some(done),
            ..self.clone()
        };
        Ok((
            next,
            LabEvent::Reviewed {
                test: self.id.clone(),
                at,
            },
        ))
    }

    /// The specimen rejected as unsatisfactory: the test must be ordered again.
    pub fn reject(&self, reason: &str, at: Timestamp) -> Result<(Self, LabEvent), LabRefused> {
        if self.stage != Stage::Collected {
            return Err(refused(self.stage, "rejected"));
        }
        self.close(Stage::Rejected, reason, at)
    }

    /// The test withdrawn before results.
    pub fn cancel(&self, reason: &str, at: Timestamp) -> Result<(Self, LabEvent), LabRefused> {
        if !matches!(self.stage, Stage::Ordered | Stage::Collected) {
            return Err(refused(self.stage, "cancelled"));
        }
        self.close(Stage::Cancelled, reason, at)
    }

    fn close(
        &self,
        stage: Stage,
        reason: &str,
        at: Timestamp,
    ) -> Result<(Self, LabEvent), LabRefused> {
        if blank(reason) {
            return Err(LabRefused::Missing("a reason"));
        }
        let next = Self {
            stage,
            reason: Some(reason.trim().to_owned()),
            ..self.clone()
        };
        Ok((
            next,
            LabEvent::Closed {
                test: self.id.clone(),
                stage,
                at,
            },
        ))
    }

    /// Whether any result is critical.
    pub fn is_critical(&self) -> bool {
        self.results.iter().any(|r| r.flag.is_critical())
    }

    /// Whether the clinician may see the results: only once verified.
    pub fn released(&self) -> bool {
        matches!(self.stage, Stage::Verified | Stage::Reviewed)
    }
}

/// Tests inside the unit of work's transaction (this module's driven port).
pub trait LabStore {
    /// Adds or replaces a test.
    fn put_test(&mut self, test: &LabTest) -> impl Future<Output = Result<(), WriteError>> + Send;
    /// One test.
    fn test(
        &mut self,
        id: &str,
    ) -> impl Future<Output = Result<Option<LabTest>, StoreError>> + Send;
}

#[cfg(test)]
mod tests;

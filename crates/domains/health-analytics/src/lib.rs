//! Indicators (analytics.md; studio.md § Indicators; ADR 0068; #548): an indicator is a FHIR R5
//! `Measure` whose populations are FHIRPath criteria over a patient's records.
//!
//! - **Definition** ([`indicator`]): [`MeasureDefinition`] is what a Measure says, and its
//!   [`MeasureDefinition::problems`] are what Validation refuses. The period date is required:
//!   it is the declared date that places care in a Report's period. A count is scored `cohort`,
//!   its criteria the `initial-population`.
//! - **Periods** ([`period`]): an indicator has no period of its own. A Report chooses the
//!   period type and the period ([`Period::parse`]), and care counts in the period it happened in
//!   ([`local_date`]).
//! - **Counting** ([`count()`]): each record's period date places it in the period or not, the
//!   criteria decide which of those count, and counting people, a person counts once. Each
//!   patient's membership keeps every record's part, which is why they are counted or not.
//!
//! Evaluating the FHIRPath is the application layer's (`health-analytics-app`): this crate takes
//! each record's period date and whether it met the criteria.

pub mod count;
pub mod indicator;
pub mod period;

pub use count::{
    Count, CountError, Member, PatientOutcome, Reason, RecordOutcome, RecordPart, count,
};
pub use indicator::{
    Counts, Expression, FHIRPATH, Indicator, MeasureDefinition, PopulationDefinition,
    PopulationKind, Scoring,
};
pub use period::{Period, PeriodError, PeriodType, local_date};

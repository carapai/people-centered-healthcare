//! Indicators (analytics.md; studio.md § Indicators; ADR 0068; #548, #549): an indicator is a FHIR R5
//! `Measure` whose populations are FHIRPath criteria over a patient's records.
//!
//! - **Definition** ([`indicator`]): [`MeasureDefinition`] is what a Measure says, and its
//!   [`MeasureDefinition::problems`] are what Validation refuses. The period date is required:
//!   it is the declared date that places care in a Report's period. A count is scored `cohort`,
//!   its criteria the `initial-population`.
//! - **Periods** ([`period`]): an indicator has no period of its own. A Report chooses the
//!   period type and the period ([`Period::parse`]), and care counts in the period it happened in
//!   ([`local_date`]).
//! - **Calculating** ([`calculate()`], #549): each record's period date places it in the period
//!   or not (or, cumulative, up to its end), the criteria decide which of those count, and
//!   people and enrollments count once each, visits and observations each one. A percentage,
//!   rate or ratio's numerator is always within its denominator; a sum, average, median,
//!   minimum, maximum or distinct count takes each counted thing's first or last value. The
//!   [`Summary`] carries what roll-up needs ([`Summary::roll_up`]). Each patient's membership
//!   keeps every record's part, which is why they are counted or not.
//!
//! - **Membership in another indicator** ([`membership`], #550): a criterion may require a
//!   patient to be in another indicator's numerator or denominator, pinned to its
//!   `url|version`, written `'<url>|<version>' in %indicators.numerator`. Indicators that refer to
//!   each other in a circle are refused ([`circles`]).
//! - **Dates relative to the period** (#550): criteria compare a declared date with
//!   `%periodStart` and `%periodEnd`, and *in the previous N periods* with the starts of the
//!   periods before ([`Period::previous_starts`]), each of the Report's period type.
//!
//! - **Age Band Sets** ([`age_bands`], #551): a published list of age ranges with the unit and
//!   the date age is taken at. Validation refuses a gap or an overlap between bands. An
//!   indicator pins the sets it disaggregates by (`url|version`) and keeps that version's bands
//!   ([`pinned`]); disaggregating by them is #552.
//!
//! Evaluating the FHIRPath is the application layer's (`health-analytics-app`): this crate takes
//! each record's period date and whether it met the criteria.

pub mod age_bands;
pub mod calculate;
pub mod indicator;
pub mod membership;
pub mod period;

pub use age_bands::{
    AGE_BAND_SET_CONTENT, AGE_BAND_SET_TYPE, AgeAt, AgeBand, AgeBandSet, AgeBandSetDefinition,
    AgeUnit, BandDefinition, PinError, is_pinned, pinned,
};
pub use calculate::{
    Evaluation, Member, Observed, PatientOutcome, Reason, RecordOutcome, RecordPart, RollUpError,
    Summary, Values, calculate,
};
pub use indicator::{
    Aggregate, BetweenDefinition, Calculation, Counts, Expression, FHIRPATH, Indicator,
    IndicatorValue, MeasureDefinition, Pick, PopulationDefinition, PopulationKind, RATE_FACTORS,
    Scoring, TimeUnit, ValueSource,
};
pub use membership::{MEMBERSHIP_VARIABLE, Reference, Side, circles, references};
pub use period::{PREVIOUS_PERIODS, Period, PeriodError, PeriodType, local_date};

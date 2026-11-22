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
//!   ([`pinned`]).
//! - **Disaggregations** ([`disaggregation`], #552): an indicator is broken down by Age Band
//!   Sets or by any coded value, each with a *not recorded* band ([`NOT_RECORDED`]) so the bands
//!   add up to the total. Each counted thing falls in one band, its first record's in the
//!   denominator, and the [`Summary`] carries each band's parts, which roll up like the whole.
//!
//! - **Reports** ([`report`], #556): configuration in a Reporting package laying out indicators
//!   for one period type and the levels it runs for, in table and line list sections.
//!   [`ReportDefinition::problems`] refuses medians, distinct counts and line lists in a Report
//!   that runs above facility level, and indicators or Age Band Sets not pinned to
//!   `url|version`.
//!
//! - **On each node** ([`store`], #554): each patient's membership per indicator and period, kept
//!   for the periods they have care in ([`periods_with_care`]), and a summary per indicator,
//!   period and facility, calculated again from the kept membership whenever a patient changes.
//! - **At central** ([`rollup`], #555): each facility's synced summaries, rolled up over a set of
//!   facilities from their summed parts ([`roll_up_facilities`]), with the facilities that have
//!   not reported shown as missing, never as zero.
//!
//! Evaluating the FHIRPath is the application layer's (`health-analytics-app`): this crate takes
//! each record's period date and whether it met the criteria.

pub mod age_bands;
pub mod calculate;
pub mod disaggregation;
pub mod indicator;
pub mod membership;
pub mod period;
pub mod report;
pub mod rollup;
pub mod store;

pub use age_bands::{
    AGE_BAND_SET_CONTENT, AGE_BAND_SET_TYPE, AgeAt, AgeBand, AgeBandSet, AgeBandSetDefinition,
    AgeUnit, BandDefinition, PinError, is_pinned, pinned,
};
pub use calculate::{
    Band, Disaggregated, Evaluation, Member, Observed, PatientOutcome, Reason, RecordOutcome,
    RecordPart, RollUpError, Summary, Values, calculate,
};
pub use disaggregation::{
    Disaggregation, DisaggregationDefinition, DisaggregationSource, NOT_RECORDED,
};
pub use indicator::{
    Aggregate, BetweenDefinition, Calculation, Counts, Expression, FHIRPATH, Indicator,
    IndicatorValue, MeasureDefinition, Pick, PopulationDefinition, PopulationKind, RATE_FACTORS,
    Scoring, TimeUnit, ValueSource,
};
pub use membership::{MEMBERSHIP_VARIABLE, Reference, Side, circles, references};
pub use period::{
    MOST_PERIODS, PREVIOUS_PERIODS, Period, PeriodError, PeriodType, RangeError, STANDARD_PERIODS,
    local_date,
};
pub use report::{
    ATTRIBUTES, AttributeOf, Axis, Catalogue, Code, Enrollments, IndicatorRow, Level,
    LineListSection, ListColumn, ListPick, REPORTS_FOLDER, ReportDefinition, ReportProblem, RowPer,
    Section, SectionDisaggregation, TableSection, attribute,
};
pub use rollup::{
    FacilitySummaries, FacilitySummary, Missing, Reported, RolledUp, roll_up_facilities,
};
pub use store::{IndicatorStore, MemberKey, Position, StoredSummary, periods_with_care};

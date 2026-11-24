# health-analytics

Layer: **domain** (analytics). Indicators (analytics.md; studio.md § Indicators; ADR 0068; #548, #549, #550, #551, #552): an indicator is a FHIR R5 `Measure` whose populations are FHIRPath criteria over a patient's records.

- **`MeasureDefinition`** is what a Measure says, as written. `problems()` is what Validation refuses:
  - no period date (the declared date that places care in a Report's period), or one that is not FHIRPath;
  - no scoring, or a scoring without the populations it needs (`cohort`: `initial-population`; `proportion` and `ratio`: also `denominator` and `numerator`; `continuous-variable`: `measure-population`), or with others;
  - no basis (the type of record the criteria read), no url, version or title, more than one group, or criteria that are not FHIRPath;
  - a rate factor other than 100, 1000, 10000 or 100000, or on a scoring other than `ratio`;
  - a `continuous-variable` indicator without one `measure-observation` (its aggregate method, and its value as an expression or the time between two dates), or another scoring with one;
  - counting visits from records other than Encounters, or observations from records other than Observations;
  - criteria that read `%indicators` other than as `'<url>|<version>' in %indicators.numerator` (or `.denominator`), a reference not pinned to a version, or one to itself.
  `indicator()` gives the `Indicator` evaluation takes, with its `Calculation`: count, percentage, rate, ratio, or a sum, average, median, minimum, maximum or distinct count of a value.
- **`Period::parse(type, id)`:** weekly `2026W10` (ISO weeks), monthly `2026-03`, quarterly `2026Q1`, semi-annual `2026S1`, annual `2026`, and the financial year from July `2025July`. `local_date` places care by the date it was written for, so 23:30 on 31 March counts toward March.
- **`calculate(indicator, period, patients, sets)`:** each record's period date places it in the period or not (cumulative: up to its end), and the criteria decide which of those count.
  - People and enrollments count once each, visits and observations each one.
  - A percentage, rate or ratio's numerator is always within its denominator.
  - A value is each counted thing's first or last in the period, by the period date.
  - Each `Member` keeps every record's `Reason` (meets, denominator only, does not meet, outside the period, no period date, no enrollment, or an error), its value and whether it was taken.
- **Membership in another indicator** (`membership`, #550): `references(expression)` reads the other indicators a criterion requires the patient to be in, by side, pinned to `url|version`; an `Indicator` keeps them in `references`. `circles(refers)` finds the indicators that refer to each other in a circle, which Validation refuses.
- **`Period::previous_starts(n)`** (#550): the first days of the `n` periods of its type before it, the nearest first, for *in the previous N periods*.
- **`Summary`** carries what roll-up needs: the count (numerator), the denominator, and the values' count, sum, minimum and maximum. `Summary::roll_up` recomputes a district's figure from summed parts and refuses a median or a distinct count, which stay at facility level.
- **Age Band Sets** (`age_bands`, #551; ADR 0071): `AgeBandSetDefinition::problems` refuses a set without a unit (days, weeks, months, years) or the date age is taken at (`period-start`, `period-end`, `period-date`), without bands, a band without its lowest age or going down, and a gap or an overlap between bands, naming the ages left out or counted twice. `AgeBandSet::band(age)` finds an age's band (`from` and `to` both included, the last band optionally open); `AgeUnit::age(birth, at)` counts whole units. A Measure's disaggregations name the sets they pin (`url|version`), an unpinned one being a problem, and `pinned(reference, sets)` finds exactly that version.

- **Disaggregations** (`disaggregation`, #552): an indicator is broken down by a pinned Age Band Set or by a coded value (`DisaggregationSource`), each with a `not-recorded` band (`NOT_RECORDED`), so the bands add up to the total. A stratifier that is both or neither, a coded one without a code, or a name used twice is a problem. Each counted thing falls in the band of its first record in the denominator (or counted), so a band's numerator is within the band's denominator; the `Summary` carries each band's own summary in `disaggregations` (a pinned set's bands in order, counted or not, or the codes found, then not recorded), and `roll_up` rolls them up band by band.

- **Population Estimates** (`estimates`, #553; ADR 0071): a Population package's `EstimatesDefinition` (estimate types given per place or derived as a percentage of another, and figures by place) and its problems; `Estimates` sums figures upward through the place hierarchy, never splitting one down, and derives types; `pro_rata` takes an annual figure for a period (month 1/12, quarter 1/4, half year 1/2, week 7/365); `parse_estimates_csv` reads the HMIS lead's CSV, naming each bad row. A Measure may name an estimate type as its denominator (`denominator_estimate`, `Indicator::estimate`): its records count the numerator only, and `over_estimate` divides it by the place's estimate.

The FHIRPath is evaluated in `health-analytics-app`.

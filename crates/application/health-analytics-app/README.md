# health-analytics-app

Layer: **application** (analytics). Indicators evaluated (analytics.md; ADR 0068; #548, #549, #550, #551), the same on every Facility Edge Node and at central.

- **`measure(resource)`** reads a FHIR R5 `Measure` as a `MeasureDefinition`:
  - `scoring` (a `measure-scoring` code) and `basis` (the type of record the criteria read, e.g. `Encounter`);
  - the first group's populations, each by its `measure-population` code with its `criteria`;
  - `indicator-period-date` (`valueExpression`, required), `indicator-counts` (`valueCode`: `people` when absent), `indicator-rate-per` (`valueInteger`) and `indicator-cumulative` (`valueBoolean`), all under `https://carapai.github.io/people-centered-healthcare/fhir/StructureDefinition/`;
  - on the `measure-observation`: `aggregateMethod`, its value as `criteria` or `indicator-value-between` (`from`, `to`, `unit`), and `indicator-value-pick` (`first` or `last`).
- **`evaluate(engine, library, indicator, period, patients)`** evaluates any indicator for one period. For each record of the basis, the period date, every population's criteria and the value are evaluated with:
  - the record as the root and `%resource`;
  - `%patient` as the Patient;
  - `%records` as a `collection` Bundle of the patient's other resources (`%records.entry.resource.ofType(Observation)`), since a variable holds one item;
  - `%periodStart` and `%periodEnd` as the period's first and last days (`YYYY-MM-DD`; a date condition compares `<date>.toDate()` with `%periodEnd.toDate()`, `%periodEnd.toDate() + 4 weeks`, …);
  - `%previousPeriods.start` as the first days of the 60 periods of the same type before it, the nearest first (*in the previous 3 periods* is `d >= %previousPeriods.start[2].toDate() and d < %periodStart.toDate()`);
  - `%indicators` as the other indicators the patient is in, `url|version` by side (`'<url>|<version>' in %indicators.numerator`). Those the criteria refer to come from the `Library` and are evaluated over the same period and patients first; a missing one, a circle, or the denominator of one without a denominator is an error (#550).
  A period date gives a date or nothing. The criteria give `true`, `false` or nothing; anything else is an error on that record, reported, and it does not count. A value gives one number (or, for a distinct count, a code) or nothing. A record's enrollment is its `episodeOfCare` or its `workflow-episodeOfCare` extension.
- **`age_band_set(resource)`** reads a `Library` of type `age-band-set` (`is_age_band_set`) as an `AgeBandSetDefinition` (#551): its `url`, `version` and `title`, and its inline `application/vnd.health.age-band-set+json` content, `{"unit": "years", "ageAt": "period-end", "bands": [{"from": 10, "to": 14}, {"from": 25}]}`; content that is missing, of another type, not base64 or not JSON is a problem. `measure` reads the sets a Measure disaggregates by from its first group's stratifiers, each with `indicator-age-band-set` (`valueCanonical`, `url|version`).
- **`test_patient(file)`** reads a package's test patient: a `tests/` file of `type` `test-patient`, with a `name` and `resources` holding one Patient and their records.

Tests: `tests/age_band_sets.rs` (#551): MoH's ANC set read from `packages/reference-packages/core`, and unreadable content named; `tests/conditions.rs` (#550): each date condition as the indicator editor's tree writes it, *next appointment within 4 weeks after the period end* over the ANC test patients, *in the previous N periods* by period type, *is in ANC 1st visits' numerator* giving the same people as evaluating it, a denominator side, and the errors; `tests/evaluate.rs`, the ANC reference package's indicators over its test patients (`packages/reference-packages/anc`) against hand calculations: *ANC 1st visits* for March, April, Q1 and an ISO week with each patient's reasons; *ANC 1st visit before 12 weeks* as a percentage, a rate and a ratio; *ANC visits* counting people, enrollments and visits; and the median gestational age at the first visit, first or last value, with ages and a distinct count.

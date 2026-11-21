# health-analytics-app

Layer: **application** (analytics). Indicators evaluated (analytics.md; ADR 0068; #548, #549), the same on every Facility Edge Node and at central.

- **`measure(resource)`** reads a FHIR R5 `Measure` as a `MeasureDefinition`:
  - `scoring` (a `measure-scoring` code) and `basis` (the type of record the criteria read, e.g. `Encounter`);
  - the first group's populations, each by its `measure-population` code with its `criteria`;
  - `indicator-period-date` (`valueExpression`, required), `indicator-counts` (`valueCode`: `people` when absent), `indicator-rate-per` (`valueInteger`) and `indicator-cumulative` (`valueBoolean`), all under `https://carapai.github.io/people-centered-healthcare/fhir/StructureDefinition/`;
  - on the `measure-observation`: `aggregateMethod`, its value as `criteria` or `indicator-value-between` (`from`, `to`, `unit`), and `indicator-value-pick` (`first` or `last`).
- **`evaluate(engine, indicator, period, patients)`** evaluates any indicator for one period. For each record of the basis, the period date, every population's criteria and the value are evaluated with:
  - the record as the root and `%resource`;
  - `%patient` as the Patient;
  - `%records` as a `collection` Bundle of the patient's other resources (`%records.entry.resource.ofType(Observation)`), since a variable holds one item;
  - `%periodStart` and `%periodEnd` as the period's first and last days (`YYYY-MM-DD`).
  A period date gives a date or nothing. The criteria give `true`, `false` or nothing; anything else is an error on that record, reported, and it does not count. A value gives one number (or, for a distinct count, a code) or nothing. A record's enrollment is its `episodeOfCare` or its `workflow-episodeOfCare` extension.
- **`test_patient(file)`** reads a package's test patient: a `tests/` file of `type` `test-patient`, with a `name` and `resources` holding one Patient and their records.

Tests: `tests/evaluate.rs`, the ANC reference package's indicators over its test patients (`packages/reference-packages/anc`) against hand calculations: *ANC 1st visits* for March, April, Q1 and an ISO week with each patient's reasons; *ANC 1st visit before 12 weeks* as a percentage, a rate and a ratio; *ANC visits* counting people, enrollments and visits; and the median gestational age at the first visit, first or last value, with ages and a distinct count.

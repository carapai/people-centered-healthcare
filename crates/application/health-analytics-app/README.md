# health-analytics-app

Layer: **application** (analytics). Indicators evaluated (analytics.md; ADR 0068; #548), the same on every Facility Edge Node and at central.

- **`measure(resource)`** reads a FHIR R5 `Measure` as a `MeasureDefinition`:
  - `scoring` (a `measure-scoring` code) and `basis` (the type of record the criteria read, e.g. `Encounter`);
  - the first group's populations, each by its `measure-population` code with its `criteria`;
  - `indicator-period-date` (`valueExpression`, required) and `indicator-counts` (`valueCode`: `people` when absent), both under `https://carapai.github.io/people-centered-healthcare/fhir/StructureDefinition/`.
- **`evaluate(engine, indicator, period, patients)`** evaluates a count (`cohort`) for one period. For each record of the basis, the period date and the `initial-population` criteria are evaluated with:
  - the record as the root and `%resource`;
  - `%patient` as the Patient;
  - `%periodStart` and `%periodEnd` as the period's first and last days (`YYYY-MM-DD`).
  A period date gives a date or nothing. The criteria give `true`, `false` or nothing; anything else is an error on that record, reported, and it does not count. Other scorings answer `EvaluateError::Scoring` until #549.
- **`test_patient(file)`** reads a package's test patient: a `tests/` file of `type` `test-patient`, with a `name` and `resources` holding one Patient and their records.

Tests: `tests/evaluate.rs`, *ANC 1st visits* over the ANC reference package's test patients (`packages/reference-packages/anc`) against hand counts for March, April, Q1 and an ISO week, with each patient's reasons.

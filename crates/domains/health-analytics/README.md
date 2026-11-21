# health-analytics

Layer: **domain** (analytics). Indicators (analytics.md; studio.md § Indicators; ADR 0068; #548): an indicator is a FHIR R5 `Measure` whose populations are FHIRPath criteria over a patient's records.

- **`MeasureDefinition`** is what a Measure says, as written. `problems()` is what Validation refuses:
  - no period date (the declared date that places care in a Report's period), or one that is not FHIRPath;
  - no scoring, or a scoring without the populations it needs (`cohort`: `initial-population`; `proportion` and `ratio`: also `denominator` and `numerator`; `continuous-variable`: `measure-population`), or with others;
  - no basis (the type of record the criteria read), no url, version or title, more than one group, or criteria that are not FHIRPath.
  `indicator()` gives the `Indicator` evaluation takes.
- **`Period::parse(type, id)`:** weekly `2026W10` (ISO weeks), monthly `2026-03`, quarterly `2026Q1`, semi-annual `2026S1`, annual `2026`, and the financial year from July `2025July`. `local_date` places care by the date it was written for, so 23:30 on 31 March counts toward March.
- **`count(counts, period, patients)`:** each record's period date places it in the period or not, and the criteria decide which of those count. Counting people, a person counts once. Each `Member` keeps every record's `Reason` (meets, does not meet, outside the period, no period date, or an error), which is why they are counted or not. Enrollments, visits and observations come with #549.

The FHIRPath is evaluated in `health-analytics-app`.

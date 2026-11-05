# health-laboratory-app

Layer: **application** (laboratory). The laboratory workflow at the facility authority (#109).

`Laboratory` offers `order`, `collect`, `result`, `verify`, `review`, `reject` and `cancel`. Each is one transaction holding the test (`lab_test`), its FHIR projections and its outbox event:
- **Specimen:** written once collected, and `unsatisfactory` once rejected (with the reason as its condition).
- **Observations (laboratory):** one per result, `preliminary` until verified, then `final`.
  - They carry the value (a decimal keeps its scale), the reference range and the interpretation (`N`, `L`, `H`, `LL`, `HH`, `A`, `AA`).
  - An Observation a correction leaves out becomes `entered-in-error`; it is never deleted.
- **DiagnosticReport (`LAB`):**
  - `registered` once collected;
  - `preliminary` once resulted;
  - `final` once verified, with the verifier as `resultsInterpreter`;
  - `cancelled` when the specimen is rejected or the test cancelled.

Ids are derived from the test (`report_id`, `result_id`), so every step versions the same resources. Each resource is tagged with its facility and names its ServiceRequest.

Tests: `tests/laboratory.rs`, over SurrealDB in memory.

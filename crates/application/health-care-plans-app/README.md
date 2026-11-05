# health-care-plans-app

Layer: **application** (care-plans). Care plans at the facility authority (#110).

- **`fhir` and `care_plan`:** the R5 CarePlan (`intent` `plan`, facility-tagged) is the module's record.
  - The Conditions it addresses are `addresses`.
  - Its activities are `activity.plannedActivityReference`.
  - The author is a platform extension.
- **`CarePlans`:** `draft`, `transition`, `address`, `plan` and `care_plan`. Each change is one transaction holding the CarePlan and `care-plans.changed.v1`.
- **Linked records:** a Condition or request must exist, be about the plan's patient, and be at the plan's facility (`NotThePatients` otherwise).

Tests: `tests/care_plans.rs`, over SurrealDB in memory.

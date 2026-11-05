# health-medications-app

Layer: **application** (medications). Prescribing at the facility authority (#108).

- **`fhir` and `prescription`:** a prescription as an R5 MedicationRequest and back.
  - It is tagged with its facility.
  - The dosage is `dosageInstruction.text`.
  - The course is `dispenseRequest.expectedSupplyDuration`, in days.
- **`Medications::prescribe(request, patient, checks, acknowledged)`** runs the safety checks first.
  - Unacknowledged warnings come back as `Unacknowledged`, and nothing is written.
  - Once acknowledged, the MedicationRequest and `medications.prescribed.v1` are written in one transaction.
  - A retried id is `AlreadyWritten`.
  - Nothing is reserved or moved.
- **`Medications::transition`:** a prescription's status, with `medications.status-changed.v1`.
- **`patient(resources)`:** what the checks know of a patient: their open prescriptions, and current allergies (not resolved, refuted or entered in error).
- **`worklist(resources, facility)`:** the pharmacy worklist from MedicationRequests.

Tests: `tests/prescribing.rs`, over SurrealDB in memory.

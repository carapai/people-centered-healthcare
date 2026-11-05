# health-shared-record-app

Layer: **application** (shared-record). The patient summary from FHIR R5 resources (#104). `health-fhir-server` serves it as `GET /Patient/{id}/$summary`.

- **`linked(patient, golden)`:** whose records make up the summary. It is the patient and, through the golden record its `refer` link names, every patient record that golden record links (`seealso`): the same person registered elsewhere (patient-identity.md).
- **`SECTIONS`:** the types searched, one search per section and patient (`{type}?patient=`). They are AllergyIntolerance, Condition, MedicationRequest, Observation, Encounter and EpisodeOfCare.
- **`record(found)` and `summary(found)`:** FHIR mapped into `health-shared-record`'s records, then summarized.
  - Vitals come only from vital-signs Observations, with each value in its own scale (`61.50`).
  - A resource missing what its section needs is left out.

Tests: `tests/summary.rs`.

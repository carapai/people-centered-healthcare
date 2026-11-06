# health-care-journeys-app

Layer: **application** (care-journeys). The Care Journey from FHIR R5 resources (#115). `health-fhir-server` serves it as `GET /Patient/{id}/$journey`.

- **`TYPES`:** the seven types searched (`{type}?patient=`) across the person's linked records, each under the reader's own search policy. They are EpisodeOfCare, Encounter, Appointment, ServiceRequest, DiagnosticReport, MedicationRequest and Task.
- **`read(found)`** maps them and builds the journey:
  - **Programs:** EpisodeOfCare.
  - **Steps:** Encounters, Appointments, ServiceRequests (a referral when categorised SNOMED CT `3457005`, `REFERRAL`), DiagnosticReports and MedicationRequests. Each goes in its program by `episodeOfCare` or the `workflow-episodeOfCare` extension.
  - **Work:** Tasks, by `focus`, open until completed, cancelled, failed, rejected or entered in error.
- **What stays off:** anything entered in error, unverified results (`registered`, `partial`, `preliminary`), and draft orders and prescriptions.
- **Not yet:** registration, follow-up and communication steps. Which communications appear is fog in care-journeys.md.

Tests: `tests/journey.rs`.

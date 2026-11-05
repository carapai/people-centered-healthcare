# health-programs

Layer: **domain** (programs). Programs (program-designer.md; care-journeys.md; #116): ART, TB, ANC and the others. Each is a published ProgramPlanDefinition whose enrolment opens an EpisodeOfCare.

- **`enrol`** opens an `active` episode for an eligible patient.
  - **`open_guard(patient, program)`:** a patient has at most one open episode of a program.
  - **Concurrency:** a patient may be in several programs at once (ADR 0042). ART and TB together are two episodes and two guards.
- **`Episode::transition`** follows R5's statuses:
  - planned or waitlisted, then active (or cancelled);
  - active and on hold move between each other, then finished;
  - any open episode may be entered in error.

  An episode that ends releases its guard.
- **`Episode::transfer_out`** is the hand-over between facilities (write-authority.md).
  - The old facility finishes its episode, naming where the patient went, and puts `programs.transferred-out.v1` on the outbox.
  - The new facility enrols the patient in its own episode, which names the old one (`transferred_from`).
- **`unapproved_regimens`:** every regimen a program uses must cite one of the program's approved protocol sources.
- **Events:** `programs.enrolled.v1`, `programs.status-changed.v1` and `programs.transferred-out.v1`.

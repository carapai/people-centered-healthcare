# health-programs-app

Layer: **application** (programs). Programs at the facility authority (#116).

- **`program(plan)`:** reads a ProgramPlanDefinition (ADR 0049):
  - its type must be `clinical-protocol`;
  - `program-episode-type` gives the EpisodeOfCare type, and `program-eligibility` the eligibility;
  - `citation`, `documentation` and `justification` artifacts are the approved protocol sources;
  - `composed-of` artifacts name its regimens (`ActivityDefinition`), Workflow Definitions (`Library`) and forms (`Questionnaire`).
- **`eligible(engine, program, patient)`:** the eligibility FHIRPath over the patient, with `%patient` as the Patient.
  - Only a single `true` is eligible.
  - Nothing is not eligible.
  - Any other result is an error.
  - A program without eligibility takes everyone.
- **`Programs`:** `enrol`, `transition`, `transfer_out` and `episode`. Each step is one transaction holding:
  - the episode's guard (`AlreadyEnrolled` when held);
  - its FHIR R5 EpisodeOfCare, tagged with its facility;
  - its outbox event.

  The EpisodeOfCare is the module's record: `fhir` writes it, and `episode` reads it back. Its extensions are `episode-program`, `episode-enrolled-by`, `episode-transferred-from` and `episode-transferred-to`.
- **Workflows:** starting the program's workflows with the episode as their context belongs to the node, since workflow use cases are another context's.

Tests: `tests/programs.rs`, over SurrealDB in memory:
- ART and TB at once, with a second ART enrolment refused;
- the lifecycle;
- the transfer hand-over;
- definitions and eligibility.

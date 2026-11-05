# health-clinical-documentation-app

Layer: **application** (clinical-documentation). Completing a QuestionnaireResponse (questionnaire-extraction.md; ADR 0022, 0023; #93): what a completion writes besides the response. The node's write path (`health-fhir-server`'s `with_extraction`) calls these in the response's own transaction, so completion is all or nothing.

- **`choose`:** the newest Extraction Mapping for the response's Questionnaire whose version range (`in_range`: `>=1.0.0 <2.0.0`, `=1.4.0`, …) takes the Questionnaire's version.
  - The response records it with `record_mapping` (`MAPPING_EXTENSION`, `valueCanonical` `url|version`), and later amendments reuse it.
  - Without a mapping, a Questionnaire produces nothing.
- **`admit`:** the Owner use case a template names must accept the planned resource (`OPT_INS`):
  - `clinical-data.record-observation` takes Observations other than laboratory ones;
  - `laboratory.record-point-of-care` takes laboratory Observations only;
  - Condition, MedicationRequest, ServiceRequest and Appointment each go to their Owner's use case.

  The list lives here until the Owners' own crates declare it.
- **`reconcile`:** the plan against the current version of each resource and the last run's targets.
  - A managed target is written when new or changed (ignoring `meta`), and revived if it was retracted.
  - A managed target the plan no longer has gets its template's retraction status (a Condition's verification status). It is never deleted.
  - A seed target is created once. An amendment that would change it gives a `Review` step instead.
- **`provenance`:** one Provenance per response, its id derived from the response (`provenance_id`), with a new version per run. It names every target version, the response version (`source`), the mapping (`instantiates`), the response's author and the extractor. `targets` reads the last run's targets back from it.

- **`consultation`** (#104): the consultation's commands as FHIR R5, each in its Encounter and tagged with its facility, with the client's UUIDv7 id. The UI never assembles them.
  - `note`: a final Composition, its text escaped into the narrative;
  - `diagnosis`: an encounter-diagnosis Condition, confirmed or provisional;
  - `service_order`: a ServiceRequest; a `laboratory` one carries that category (SNOMED 108252007), and laboratory follows it (#109).

  Prescriptions are the medications module's (`health-medications-app`, #108).

  Blank text or concepts are refused, as is a code without its system.

- **`notes`** (#107): the signed-note rule on FHIR resources. A note's status is a Composition's `status` or a DocumentReference's `docStatus`.
  - `check_replace(current, next)` refuses an in-place change to a signed note, unless it marks the note entered in error with its content unchanged (`meta` and the status aside).
  - `check_delete(current)` refuses deleting a signed note.

  `health-fhir-server` applies both on every write, and the Authority Node will apply them to versions arriving by sync.
  - `fhir(note, facility)` and `note(composition)` map a note to its Composition and back. Sections are in the narrative, and the signature is an `attester` (mode `legal`). An amendment carries `relatesTo` (`replaces` or `appends`).
  - `provenance(id, act, target, source, by, at)` builds the Provenance of a signature (activity `LA`, legally authenticated) or of an amendment (activity `UPDATE`, with the original's version as the `revision` entity).
- **Consultation notes** are saved final, and signed by their author as they are saved (`attester`, mode `legal`).

Tests: `tests/notes.rs`, `tests/consultation.rs`, and `tests/completion.rs` (Owner opt-ins, version ranges, the choice of mapping, recording it, managed and seed reconciliation with retraction, and the Provenance), over the extractor's ANC fixtures.

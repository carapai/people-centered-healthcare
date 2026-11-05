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

Tests: `tests/completion.rs` (Owner opt-ins, version ranges, the choice of mapping, recording it, managed and seed reconciliation with retraction, and the Provenance), over the extractor's ANC fixtures.

# Extracted resources are either managed by their form or seeded once, and each is written by its own Owner

Every target in an Extraction Mapping declares one of two modes.
- **`managed`** (e.g. vitals and exam Observations):
  - The QuestionnaireResponse stays the source of truth, and the resource is read-only elsewhere.
  - An amendment re-extracts and writes new versions.
  - A target that no longer appears in the output is set to the type's retraction status, such as `entered-in-error`. It is never deleted.
- **`seed`** (e.g. Condition, MedicationRequest, ServiceRequest, Appointment):
  - Extraction creates the resource once, and its own workflow owns it from then on.
  - A later amendment raises a "source form amended" review item instead of rewriting it.

**Writing.** The pure extractor produces a plan, and the `CompleteQuestionnaireResponse` use case hands each planned resource to its **Owner's own use case**, as given by the ownership table: for example, a laboratory-category Observation to laboratory, and a MedicationRequest to medications. The Owner's domain rules therefore apply.

**Opting in.** Each Owner declares which types and profiles it accepts from extraction. Laboratory accepts only point-of-care results.

We chose this because some extracted facts are form readings that the form should keep correcting, while others start a lifecycle of their own, such as a diagnosis being resolved or a prescription being stopped. Rewriting those from a form amendment would silently undo clinical work. Routing writes through Owners keeps ADR 0006's isolation intact.

## Considered Options

- **Every extracted resource managed by its form.** Rejected: a form amendment would revert lifecycle changes made later.
- **Every extracted resource detached after creation.** Rejected: amending a mistyped weight would leave the wrong Observation in place.
- **The extractor writing resources directly.** Rejected: it would bypass Owners' rules and break table ownership.

## Consequences

- The UI routes edits of managed resources to "amend form".
- Bulk re-extraction with a newer mapping is an explicit, audited admin job that touches managed targets only. Publishing a mapping never re-extracts existing responses.

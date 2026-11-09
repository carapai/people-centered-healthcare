# Triage

The OPD triage screen (#103; patient-flow.md; questionnaire-extraction.md).

- **`TriagePage`** (`/triage`):
  - **The queue:** the patients waiting at the `triage` service point, refreshed every 15 seconds. Choosing one calls them and starts seeing them, as consultation does.
  - **The form:** the newest active `opd-triage` Questionnaire (`ug.moh.opd`, `packages/reference-packages/opd`). It is rendered by the shared `QuestionnaireRenderer`, with the visit's patient and encounter and a UUIDv7 id. Vital signs are decimal and integer answers, and the triage category is required.
  - **Submit:**
    1. The response is stored `completed` with `authored` now. In the same transaction the node extracts each answered vital sign as a `vital-signs` Observation, with the response's Provenance. Blood pressure is one panel with systolic and diastolic components.
    2. The visit moves to consultation, and the screen is back at the queue with a status message.
- **Who:** the triage role needs:
  - `QuestionnaireResponse` create and update;
  - create and update on what the form produces: `Observation`, and `Provenance` for the extraction record;
  - `queues.work` and `encounters.manage`.
- **Not yet:**
  - **The triage category's effect on the visit:** sending it as the visit's workflow event comes with the OPD Workflow Definition (#105), which records it as `urgency`. Urgent and emergency visits then never wait at a Financial Gate. Until then the category is kept on the response only.
  - **Blood pressure in the summary:** the patient summary lists the panel but not its components.

## Tests

`e2e/triage.spec.ts`:
- a clerk checks a patient in at triage;
- a nurse records vital signs and a category, keyboard only and with axe;
- the patient leaves the triage queue;
- the clinician's summary shows the extracted vital signs.

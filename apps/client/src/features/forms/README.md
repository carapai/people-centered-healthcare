# Forms

The shared Questionnaire renderer (client-application.md § Forms; ADRs 0038, 0052, 0054; #92).

- **`QuestionnaireRenderer`** renders a Questionnaire with its Form Layout, on TanStack Form.
  - **Answers:** boolean (radio or checkbox), coding from `answerOption`, or from the codes the caller gives for its `answerValueSet` (`answers`, by binding), as select or radio (a drop-down beyond 7 choices unless the Form Layout names a widget), integer and decimal, date and dateTime, and string and text.
  - **Exact numbers:** decimals keep the text as typed (`1.50`) as `LosslessNumber`s (ADR 0052).
  - **Groups and repeats:** groups render as fieldsets, and repeating groups can be added and removed. Every repeat keeps a stable `item.id`.
  - **Live behaviour, with fhirpath.js in lossless precise mode** (the configuration shared with `@health/fhirpath-conformance`):
    - `enableWhen` (with `enableBehavior`) and `enableWhenExpression` hide disabled questions;
    - `calculatedExpression` fills read-only answers;
    - `questionnaire-constraint` errors and warnings show at their question;
    - required questions are flagged on submit.
  - **Saving and submitting:** `onSave` and `onSubmit` receive the settled response, with calculated values applied and disabled questions removed. Submit is blocked while a required question or an error constraint is unmet.
- **`FormsPage`** (`/forms`):
  - pick an active Questionnaire, then start a response or continue a saved draft;
  - **Save draft** stores it on the server (`platform.forms.saveResponse`, a `PUT` with its UUIDv7 id, and `If-Match` after the first save), never in the browser (ADR 0037). A draft saved elsewhere meanwhile fails with 412;
  - **Submit** stores it `completed`.
- **Bound questions (#843):** `answers.ts`'s `useBoundAnswers` expands each ValueSet a form binds through the node's own `ValueSet/$expand` (its active codes, the first 1000), central's or, offline, the facility edge's, from the active packages' terminology. `FormsPage` and triage pass the codes to the renderer; a binding the node cannot expand shows an alert. The form designer's preview expands through the Studio's terminology instead.
- **`model.ts`** is the logic, as pure functions: initial response, enablement, calculation, constraints, required questions, settling and repeats.
- **`layout.ts`** handles Form Layouts (`$schema` `…/schema/form-layout/1`, keyed to a Questionnaire `url|version`):
  - titled sections of top-level items, with widgets and hints per question;
  - a layout never hides a question: items it does not place show after its sections, and a widget is used only where it fits the question's type.
- **Previews only:** everything the browser computes is a preview. The server re-validates every submission against its Questionnaire, refusing what the screen should have stopped, and stores its own calculated values (ADR 0052; `health-fhir-server`'s questionnaire checks). fhirpath.js keeps values exact but not the scale of arithmetic results (`61.6` where the server gives `61.60`).
- **Not yet:**
  - opening a form from an encounter or task, with its subject;
  - Form Layouts from the active configuration (the screen renders without one until activation, #89);
  - a search-as-you-type widget for large ValueSets (Form Layout v1 has none), attachments, quantities and population.

## Tests

- `test/forms.test.ts`: the model (enableWhen, enableWhenExpression, calculated values, constraints, required questions, settling, repeats).
- `test/form-layout.test.ts`: sections and widgets.
- `test/forms-bound-answers.test.ts`: the `$expand` asked for a binding, the codes read from it, the ValueSets a form binds, and a large ValueSet as a drop-down.
- `e2e/forms.spec.ts`: a nurse fills the ANC visit form. It checks enableWhenExpression and enableWhen in the browser and the required-question block, then a draft saved on the server, continued and submitted, with axe.
- `e2e/edge/bound-form.spec.ts`: on an edge with no central, a form whose questions bind ValueSets of 3 and 14 codes offers them as radio buttons (the retired code left out) and a drop-down, and is submitted.

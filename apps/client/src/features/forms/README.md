# Forms

The shared Questionnaire renderer (client-application.md § Forms; ADRs 0038, 0052, 0054; #92).

- **`QuestionnaireRenderer`** renders a Questionnaire with its Form Layout, on TanStack Form.
  - **Answers:** boolean (radio or checkbox), coding from `answerOption` (select or radio), integer and decimal, date and dateTime, and string and text.
  - **Exact numbers:** decimals keep the text as typed (`1.50`) as `LosslessNumber`s (ADR 0052).
  - **Groups and repeats:** groups render as fieldsets, and repeating groups can be added and removed. Every repeat keeps a stable `item.id`.
  - **Live behaviour, with fhirpath.js in lossless precise mode** (the configuration shared with `@health/fhirpath-conformance`):
    - `enableWhen` (with `enableBehavior`) and `enableWhenExpression` hide disabled questions;
    - `calculatedExpression` fills read-only answers;
    - `questionnaire-constraint` errors and warnings show at their question;
    - required questions are flagged on submit.
  - **Saving and submitting:** `onSave` and `onSubmit` receive the settled response, with calculated values applied and disabled questions removed. Submit is blocked while a required question or an error constraint is unmet.
- **`model.ts`** is the logic, as pure functions: initial response, enablement, calculation, constraints, required questions, settling and repeats.
- **`layout.ts`** handles Form Layouts (`$schema` `…/schema/form-layout/1`, keyed to a Questionnaire `url|version`):
  - titled sections of top-level items, with widgets and hints per question;
  - a layout never hides a question: items it does not place show after its sections, and a widget is used only where it fits the question's type.
- **Previews only:** everything the browser computes is a preview. The server re-validates every submission and recomputes calculated values (ADR 0052). fhirpath.js keeps values exact but not the scale of arithmetic results (`61.6` where the server gives `61.60`).
- **Not yet:**
  - the screens that open a Questionnaire, with drafts saved on the server;
  - the server's re-validation;
  - `answerValueSet`, attachments, quantities and population.

## Tests

- `test/forms.test.ts`: the model (enableWhen, enableWhenExpression, calculated values, constraints, required questions, settling, repeats).
- `test/form-layout.test.ts`: sections and widgets.

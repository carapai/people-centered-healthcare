# @health/fhirpath-conformance

The shared FHIRPath conformance suite (ADRs 0038, 0054; #92). Live forms evaluate FHIRPath in the browser with **fhirpath.js**, and the server, which stays authoritative, evaluates it with **octofhir-fhirpath**. Any expression a published Questionnaire uses must evaluate identically in both, so the same fixtures run against both engines in CI.

- **`fixtures/*.json`:** each file is a group with a `resource` and its `cases` (`name`, `expression`, `expected`). Expected items are `{ "boolean": … }`, `{ "number": "<exact text>" }`, `{ "text": … }` (strings, codes and dates, as written) or `{ "quantity": { "value", "unit" } }`.
  - `forms.json`: what live forms read in a QuestionnaireResponse, for enableWhen, calculated values and answer constraints.
  - `arithmetic.json`: decimal-exact arithmetic for calculated values.
- **`src/index.ts`:** fhirpath.js 5.2 in lossless precise mode, as the client must run it:
  - resources are parsed with `lossless-json`, each decimal becoming fhirpath.js's exact decimal from its source text;
  - `preciseMath` and `keepDecimalTypes` are on;
  - results are normalized to the fixtures' shape.
- **Runners:**
  - `test/conformance.test.ts` (`bun run check`) runs the fixtures on fhirpath.js;
  - `crates/contracts/health-fhir-validation/tests/fhirpath_conformance.rs` runs them on the server engine.

## What the engines differ on

These constructs stay out of published Questionnaires' expressions, so they are not in the fixtures:

- **Integer and decimal types:** in precise mode fhirpath.js returns every number as its decimal type. Numbers are therefore compared by their exact text, and an expression must not depend on telling an Integer from a Decimal.
- **The scale of arithmetic results:** `61.5 - 1.5` is `60.0` on the server and `60` in fhirpath.js. Calculated values are previews in the browser, and the server recomputes them on submit (ADR 0052).
- **From the spike** ([docs/research/frontend.md](../../docs/research/frontend.md)): document functions, `precision()`, `conformsTo()`, boundary functions, mixed-precision and unit-mismatch comparisons, date arithmetic offsets, `distinct`, and `%sct`, `%loinc`, `%vs-*` and `%ext-*`.

## Licences

fhirpath.js, its UCUM library and `argparse` are per-package licence exceptions (deployment.md § Licences).

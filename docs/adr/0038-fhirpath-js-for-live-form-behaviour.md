# Live form behaviour uses fhirpath.js in the browser, and the server re-validates every submission

The shared Questionnaire renderer, built on TanStack Form, evaluates these expressions client-side with **fhirpath.js**, HL7's JavaScript FHIRPath implementation, which supports R5:
- `enableWhenExpression`;
- `calculatedExpression`;
- answer constraints;
- population previews.

The server (`octofhir-fhirpath` in Rust) stays authoritative. It re-validates every submission and runs extraction (ADR 0022).

A shared FHIRPath conformance fixture suite runs against both engines in CI. Any expression used by a published Questionnaire must evaluate identically in both, or the publication gate fails.

We chose this because form logic evaluated on every keystroke cannot afford a network round trip on a busy LAN, and fhirpath.js is the mature, HL7-maintained JavaScript engine.

## Considered Options

- **Server round trips for every expression.** Rejected: too slow on facility LANs and Wi-Fi.
- **Compiling the Rust FHIRPath engine to WASM now.** Deferred: the async API and the bundle size are unverified. It would give one engine and is worth a later spike.
- **Hand-coded form logic per form.** Rejected: forms are configuration (ADR 0024).

## Consequences

- **Refined by ADR 0054:** fhirpath.js runs in lossless precise mode, and published expressions are limited to constructs passing in both engines.
- **Refined by ADR 0052:** the server recomputes every `calculatedExpression` on submit, and its value is stored.
- Two engines can diverge. The conformance suite and the publication gate catch divergence before a form goes live.

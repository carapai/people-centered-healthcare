# `fhir-r5` is the adopted R5 model crate, pinned exactly and guarded by a corpus round-trip in CI

We adopt **`fhir-r5 = "=4.2.3"`** behind `health-fhir-models`, as ADR 0015 requires. We take it under its MIT or Apache-2.0 option.

**Guards:**
- **Corpus round-trip in CI.** Every resource in the official R5 `examples-json.zip` must parse and re-serialize to identical JSON. A short list of known-invalid official examples is the only exception.
  - The list holds one file today: `questionnaireresponse-example-f201-lifelines.json`, which is missing the required `questionnaire`.
  - The round-trip runs on every bump of the crate version.
- **No leaking types.** No `fhir-r5` type appears in the public API of `health-fhir-models`, so a future swap stays inside the facade.
- **One crate names `Resource`.** Only rarely changing crates may name the whole `Resource` enum, because its serde code is rebuilt in whichever crate uses it, which costs 8–12 minutes in a release build. Release CI runners need at least 8 GiB of RAM.

We chose this because it was the clear winner of the spike [Which R5 model crate passes the round-trip spike: helios-fhir or fhir-r5?](https://github.com/carapai/people-centered-healthcare/issues/35). That means ADR 0015's default to `helios-fhir` does not apply: the default was only for an inconclusive result.
- **Round-trip.** `fhir-r5` round-tripped 2,823 of the 2,824 official examples exactly, with no invalid output.
- **Invalid JSON.** `helios-fhir 0.2.6` wrote invalid JSON for 40 of them, whenever a choice element is the first field in a backbone element. It also writes `integer64` as a JSON number instead of a string.
- **Build cost** is about the same for both crates.
- **Dependencies.** `fhir-r5` brings 91 packages against helios's 256, and no build-time HTTP or TLS dependencies.

## Considered Options

- **`helios-fhir` with workarounds:** always serialize through `serde_json::Value`, and patch `integer64` output. It has a far larger maintainer base, but we would ship around known serializer bugs in the facade. Rejected. The two bugs are worth reporting upstream.

## Consequences

- **Maintainer risk.** `fhir-r5` has one maintainer, is about two and a half months old, and has had fast major-version churn. The exact pin, the facade and the CI corpus contain that risk, and the permissive licence option lets us fork.
- **Strict parsing.** `fhir-r5` refuses any resource that is missing a required element. How inbound data handles that is decided in ADR 0051: tier 1 runs on the raw JSON first.
- **Unknown elements are dropped.** Typed parsing silently drops unknown elements, and both crates do this. Typed parsing is therefore never the only check on inbound data: tier 1 validation runs on the raw JSON first and rejects unknown elements (ADR 0051).
- **Lost `_id` extension.** An extension on `Resource.id` (`_id`) is dropped. The corpus contains none. The stored form is the validated input JSON (ADR 0051), so the loss affects only typed domain logic.
- **R4** comes from the sibling crate `fhir-r4 =4.2.2`, used only in `health-fhir-mapping`.
- **FHIRPath precision.** The spike found that `octofhir-fhirpath` loses decimal precision. ADR 0052 fixes this with a patched fork.
- **Profile validator.** The spike found that `helios-fhir-validator` evaluates only pattern discriminators. ADR 0053 builds our own tier 3 validator instead.

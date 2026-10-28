# FHIR R5 models come from an adopted crate behind our own facade; search and persistence are ours

We adopt one generated R5 model crate, pinned to an exact version and with only R5 enabled. It sits behind our `health-fhir-models` facade, and domain crates import only the facade. FHIRPath runs behind a `FhirPathEngine` trait, with `octofhir-fhirpath` (which passes the full official suite) as the default. Search-parameter indexing, history and versioning are built in-house on SurrealDB from the official `SearchParameter` definitions.

The reasons:
- Maintained R5 crates already handle the costly edge cases: primitive `_field` extensions, exact decimals, choice types and recursive elements.
- No Rust FHIR search or persistence layer has a SurrealDB backend, and adopting one would bring a competing schema and tenancy model.
- Every candidate crate is young or pre-1.0, so the facade keeps a future swap local.

The crate (`helios-fhir` or `fhir-r5`) is chosen by the spike task [Which R5 model crate passes the round-trip spike: helios-fhir or fhir-r5?](https://github.com/carapai/people-centered-healthcare/issues/35). If the spike is inconclusive, the default is `helios-fhir`.

**Resolved:** the spike picked `fhir-r5 =4.2.3` (ADR 0050).

## Considered Options

- **Our own code generator from `hl7.fhir.r5.core`:** full control, but it re-solves problems the crates have already paid for, before any clinical feature exists. Rejected for now. It stays possible later behind the facade.
- **Adopting the Helios stack wholesale** (models, validator, persistence, REST): pre-1.0, no SurrealDB backend, and its persistence would compete with resource ownership (§3.4). Rejected.

## Consequences

- Domain code works on typed models. Canonical R5 JSON is the stored resource of record.
- R4 never enters storage. Conversion lives in `health-fhir-mapping`, which only boundary adapters use.
- The generated model crate is costly to compile. It is isolated in its own workspace member and cached in CI.

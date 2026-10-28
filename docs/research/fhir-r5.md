# Research: FHIR R5 in Rust

This condenses the research ticket [What are the options for FHIR R5 resource models, validation and search in Rust?](https://github.com/carapai/people-centered-healthcare/issues/5). The full findings, with citations, are on the unmerged branch `research/fhir-r5`. Decisions: [docs/architecture/fhir-r5-model.md](../architecture/fhir-r5-model.md), ADRs 0015 and 0016.

## Model crates (as of 2026-10-27)

| Crate | R5 | Latest | Primitive extensions | Validation | Notes |
|---|---|---|---|---|---|
| `helios-fhir` (HeliosSoftware/hfs, MIT) | yes | 0.2.6 | Single `Element<V, E>` value with derive-generated `_field` serde; precise decimals | `helios-fhir-validator` (profiles, invariants, required bindings) | Pre-1.0, very active; R4 is the default feature; docs.rs build of 0.2.6 failed |
| `fhir` / `fhir-r5` (fhir-rust, multi-licence) | yes | 4.2.4 | Transparent newtypes; `_field` mechanism unverified | Structure, cardinality, regex, required bindings, 3 of 314 invariants | `fhir-r5` crate is about 2.5 months old; `Coded<E>`, `Vec1<T>` |
| `fhir-model` / `fhir-sdk` (MIT) | yes | 0.13 / 0.16 | Parallel `x_ext` fields | None | High compile time and memory; 5-month release gap |
| `fhirbolt` | yes | 0.4 (2023) | — | None | Unmaintained |

## FHIRPath

- `octofhir-fhirpath` 0.4.57 passes **1176/1176** official tests (2026-10-21). It works over JSON, with pluggable model and terminology providers, and has an async API.
- `helios-fhirpath` 0.2.6 runs over typed Helios models. It is missing `conformsTo`, `slice`, `%factory` and others.

## Validation

- Full validation has four layers: structure and cardinality, FHIRPath invariants (314 in R5 core), profiles and slicing, and terminology bindings.
- Only `helios-fhir-validator` covers all four, with an off / log / enforce write-path mode. It is lightly used, and its conformance against the HL7 suite is unverified.
- `octofhir-fhirschema` is experimental.
- The HL7 Java validator suits use as a CI oracle (common practice, not sourced).

## Search

- **R5 search surface:**
  - nine parameter types;
  - prefixes `eq ne gt lt ge le sa eb ap`, with implicit ranges from precision;
  - modifiers, where a server "SHALL reject" a modifier it does not support on a parameter it advertises;
  - AND across repeated parameters and OR within commas;
  - `.` chaining, `_has`, `_include` / `_revinclude` / `:iterate`;
  - `_sort`, `_count`, `_total`, `_summary`, `_elements`;
  - search via both GET and POST is mandatory;
  - unknown parameters SHOULD be ignored unless `Prefer: handling=strict`.
- `SearchParameter` definitions ship in `hl7.fhir.r5.search` and `definitions.json.zip`. Each carries a FHIRPath `expression`, so extraction comes down to evaluating it and normalizing the result.
- No Rust crate offers search on SurrealDB. Helios's storage trait split (`ResourceStorage`, `VersionedStorage`, `SearchProvider`, `ConditionalStorage`, `TransactionProvider`) is a useful template.
- `octofhir-ucum` 0.8 handles quantity normalization.

## REST and versioning obligations (R5 http)

- On create the server SHALL set `id`, `meta.versionId` and `meta.lastUpdated`, and SHALL ignore a client-supplied id. Update ignores the client's `versionId` and `lastUpdated`.
- Concurrency: weak ETag equal to `versionId`. `If-Match` mismatch → 412; a detected conflict may → 409.
- Deleted → 410, unknown → 404.
- Conditional create: 0 matches → create, 1 → 200, more than 1 → 412. Conditional update and delete follow the matrices in the full findings.
- History Bundles are sorted oldest-last and include deletes. They take `_since`, `_at` and `_count`.
- Transactions are all-or-nothing and independent of entry order.

## Codegen

- `hl7.fhir.r5.core` 5.0.0 is the only R5 package. `definitions.json.zip` is "the first choice" for generating implementation artefacts.
- Writing our own generator is estimated at 2–4k lines (unverified). The ongoing cost is the edge cases the existing crates have already solved: primitive extensions, choice types, recursion, decimals, reserved words, compile time.

## Open items

- Crate choice and validator evaluation: [Which R5 model crate passes the round-trip spike: helios-fhir or fhir-r5?](https://github.com/carapai/people-centered-healthcare/issues/35).

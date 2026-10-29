# Research: FHIR R5 in Rust

This condenses the research ticket [What are the options for FHIR R5 resource models, validation and search in Rust?](https://github.com/carapai/people-centered-healthcare/issues/5). The full findings, with citations, are on the unmerged branch `research/fhir-r5`. Decisions: [docs/architecture/fhir-r5-model.md](../architecture/fhir-r5-model.md), ADRs 0015, 0016 and 0050.

## Model crates (as of 2026-10-27)

| Crate | R5 | Latest | Primitive extensions | Validation | Notes |
|---|---|---|---|---|---|
| `helios-fhir` (HeliosSoftware/hfs, MIT) | yes | 0.2.6 | Single `Element<V, E>` value with derive-generated `_field` serde; precise decimals | `helios-fhir-validator` (profiles, invariants, required bindings) | Pre-1.0, very active; R4 is the default feature; docs.rs build of 0.2.6 failed |
| `fhir` / `fhir-r5` (fhir-rust, multi-licence) | yes | `fhir-r5` 4.2.3 (4.2.4 is the `fhir` facade) | Transparent newtypes; `_field` round-trips (spike below) | Structure, cardinality, regex, required bindings, 3 of 314 invariants | `fhir-r5` crate is about 2.5 months old; `Coded<E>`, `Vec1<T>` |
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

## Model-crate spike (2026-10-28)

The spike was [Which R5 model crate passes the round-trip spike: helios-fhir or fhir-r5?](https://github.com/carapai/people-centered-healthcare/issues/35). It was run in a throwaway scratch workspace, outside the repo.

**Setup:**
- Each crate built in its own workspace with R5 only.
- `serde_json` with `arbitrary_precision`.
- Host: x86_64 i9-9980HK with 32 GB, macOS, rustc 1.98.0.

**Pick: `fhir-r5 =4.2.3`** (ADR 0050).

**Corpus.** The official `examples-json.zip` has 2,824 files covering 157 resource types. It contains:
- 81 `_field` primitive extensions;
- 32 nulls inside arrays;
- 329 decimals with trailing zeros;
- `contained` resources in 312 files;
- 86,033 choice keys.

| Measure | `helios-fhir =0.2.6` | `fhir-r5 =4.2.3` |
|---|---|---|
| Exact JSON round-trip | 2,769 (2,804 if serialized through `serde_json::Value`) | **2,823** |
| Parse failures | 0 | 1 (an official example missing the required `QuestionnaireResponse.questionnaire`) |
| Invalid JSON written | **40** | 0 |
| `integer64` written as a JSON number (must be a string) | yes (all 37 diffs that remain via `Value`) | no |
| Decimal text kept (1.50, 0.010, 30- and 36-digit values, 1.0e3, -0.0) | yes | yes |
| `_field` extensions, nulls in arrays, contained, partial dates, unknown codes | kept | kept |
| Unknown elements | dropped silently | dropped silently |
| `_id` on `Resource.id` | kept | dropped |
| Missing required element | accepted | parse error |
| Lockfile packages | 256 (exact pins on `serde` and `serde_json`; build-time HTTP and TLS) | 91 |
| Debug build, clean / touch | 199–235 s / 2.6–2.8 s | 180–195 s / 2.8–3.1 s |
| Release build, clean / touch | 995–1,056 s / 540–550 s | 777–1,237 s / 507–730 s |
| Peak rustc memory | 5.3 GiB | 6.2 GiB |
| R4 | feature of the same crate | sibling crate `fhir-r4` 4.2.2 |
| Licence | MIT | MIT OR Apache-2.0 OR BSD-3-Clause OR GPL-2.0-only OR GPL-3.0-only (recipient chooses) |
| Activity (2026-10-28) | 81k downloads; 52 releases; about 8 active contributors | 816 downloads; 10 releases since 2026-08-10; one maintainer |

**The helios invalid-JSON bug.** When a choice element is the first field in a backbone element, helios writes `[{},"contentReference":…`. It affects Communication, MedicationRequest dosing, Questionnaire and QuestionnaireResponse, and others. The bug is in its streaming serializer: going through `Value` gives valid output.

**Notes on the build timings:**
- The release timings vary by about ±50%, probably from thermal throttling on the laptop.
- A `touch` costs minutes in release because the serde code for `Resource` is built in the crate that uses it.

### Side checks

- **`octofhir-fhirpath` 0.4.57.**
  - Paths, extensions and `ofType` gave the same results over JSON from both crates. `ofType` needs `octofhir-fhirschema` 0.3.30's `EmbeddedSchemaProvider::r5()`.
  - The README's example code does not compile against 0.4.57.
  - **It loses decimal precision:** 0.010 became 0.0100000000000000002081668171, and a 30-digit value became `"1.2345678901234568e+29"`. The cause is that 14 places in 9 files go through `as_f64()` and `Decimal::from_f64_retain`. ADR 0052 fixes it with a patched fork.
- **`helios-fhir-validator` 0.2.6** works on raw JSON.
  - On a hand-written profile with a `pattern` discriminator, it reports cardinality, slices and unknown elements correctly.
  - On the official vital-signs profile, which uses `value` discriminators, it gave a false positive. Its documentation says only `pattern` discriminators are evaluated.
  - It cannot share a build with `fhir-r5` or `octofhir-fhirpath`: Cargo fails on its `serde =1.0.224` pin (checked 2026-10-29).
- **`octofhir-fhirschema` 0.3.30** (checked 2026-10-29) embeds 307 R5 schemas and resolves alongside `fhir-r5` and `octofhir-fhirpath`.
  - **What works:** base checks catch unknown elements and missing required elements; invariants run through the FHIRPath engine; about 0.3 ms per Observation.
  - **What fails:**
    - `merge_elements` drops `slicing` on elements that exist in the base, so a missing vital-signs category or a duplicated slice passes;
    - `fixed` and `pattern` values are never enforced;
    - two `value[x]` fields are missed;
    - it gives false positives on `integer64` strings and on extension-only primitives;
    - 485 of the 2,824 official examples are flagged, nearly all wrongly.
  - **Other concerns:** heavy dependencies (two `reqwest` versions, `rusqlite`), one maintainer, and no LICENSE file.
  - **Outcome:** ADR 0053 builds our own tier 3 validator.

### Not run

- Building or round-tripping R4.
- XML.
- An aarch64 host.
- Incremental builds after a real code edit (only `touch`).

## Open items

- Handling of strict-parse failures and dropped elements: decided in ADR 0051.

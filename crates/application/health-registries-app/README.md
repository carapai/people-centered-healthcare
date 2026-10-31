# health-registries-app

Layer: **application** (Registries). The client registry's use cases over any `health_persistence::Store` (ADR 0027).

- **`ClientRegistry::link(source, golden, actor)`:** links a source Patient. With `golden` as `None`, it creates a golden Patient with a fresh ECID.
  - The golden record takes the source's values by survivorship. Disagreements come back as review items, for the review queue (#70).
  - The source gains `refer`, the golden record `seealso`, and the source loses its `provisional` tag.
- **`unlink(source, reviewer)`:** separates a source.
  - The golden record's values are recomputed without it, and it can never be re-linked there.
  - A golden record with no sources becomes inactive.
- **`merge_preview(source, target)` / `merge(plan)`:** `$merge` for two sources of the same facility, after a person approves the preview.
  - The duplicate becomes `active: false` and `replaced-by` the other, which `replaces` it.
  - Nothing that references the duplicate is rewritten.
  - The plan is checked again inside the merge's transaction.
- **Every use case** is one transaction with `If-Match` version checks. Each version it writes passes through the `AuditTrail` port in that transaction; the runtime implements it over `health-audit` (#197).

## Matching adapters (`matching.rs`)

- **`DoubleMetaphone`** implements the domain's `Phonetic` port with `rphonetic`, a port of Apache commons-codec. Its `enum-iterator` dependency is 0BSD, which `deny.toml` allows as part of ADR 0045's BSD family.
- **`person(patient, config)`** turns a Patient into a matching `Person`.
  - A year-only `birthDate` counts as an estimate.
  - The ECID is always verified; a NIN is verified only when NIRA has verified it.

## `$match` (`matching.rs`)

- **`RegistryBlocking`** implements `health_persistence::DerivedKeys`: the ADR 0055 keys of active source Patients. Golden and merged records are never candidates. A runtime gives it to `SurrealStore::with_blocking`.
- **`Matcher::find(query, count, only_certain)`:**
  1. plans the query's keys by the stored `block_count`, skipping oversized blocks by count, never by scanning with a limit;
  2. reads the members of each used block;
  3. scores the candidates;
  4. returns the ones graded possible or better (or auto-link only), best first, with the used and skipped key counts.
- **`bundle(matched)`** builds the FHIR answer.
  - A searchset Bundle whose `search.score` is the match weight as a probability, with the `match-grade` extension.
  - Or, when nothing could be looked up, an OperationOutcome (`incomplete`) pointing to the manual registry search.
- **Not here yet:**
  - mounting the `Patient/$match` route, which comes with the runtime's routes (#188, #197);
  - loading the Matching Rules from the configuration package (Phase 6), so for now it uses `MatchingRules::platform_default`;
  - term frequencies from counts (only the floors apply until then).

## FHIR mapping (`fhir.rs`)

- **Tags:** golden Patients carry the `urn:health:registry` tag `golden`, and unreconciled sources carry `provisional`.
- **Source trust:** read from the `urn:health:registry:trust` tag, staff-entered by default. A NIN with the `urn:health:registry:verification` extension `nira-verified` is trusted at level 1.
- **Golden Patients carry:**
  - the ECID (`urn:health:ecid`), and the NIN with its verification;
  - each field's surviving value;
  - an `urn:health:registry:origin` extension per field: field, source, trust, time and value;
  - an `urn:health:registry:separated` extension per adjudication marker.
- **Provenance:** storing a Provenance per value is [#196](https://github.com/carapai/people-centered-healthcare/issues/196). Provenance has no table until its storage discriminator is decided.

## Tests

`tests/registry.rs` runs over SurrealDB:
- linking three sources builds one golden record;
- staff-verified beats staff-entered, a NIRA-verified NIN survives an unverified one, and a self-reported name never overwrites, with the last two reviewed instead;
- links on both sides, and the provisional tag removed;
- every write audited;
- unlink splits, and the pair is never re-linked;
- a merge needs the same facility and an approval, and leaves an Observation pointing at the duplicate unchanged, with the same version;
- the golden and merged Patients pass tier 1.

`tests/matching_quality.rs` is the CI corruption benchmark and the precision/recall report (ADR 0055). It takes 60,000 synthetic patients and 2,000 duplicates, each corrupted once or twice: typos, swapped names, ages off by up to five years (recorded as estimates beyond one), a dropped NIN, a changed phone. It asserts:
- blocking recall ≥ 99%;
- every candidate set under 2,000;
- auto-link precision ≥ 99.9%.

It prints the skipped-key share, and the true and false counts at auto-link and review.

On 2026-10-31 it measured:
- blocking recall 99.85%, with candidate sets at p95 19 and at most 97, and no keys skipped;
- auto-link: 863 true and 0 false, so 43.1% of duplicates link without review;
- 91.1% of duplicates found at auto-link or review.

HTTP operations (`Patient/$merge`, link and unlink) are [#197](https://github.com/carapai/people-centered-healthcare/issues/197). The blocking tables and `Patient/$match` are [#199](https://github.com/carapai/people-centered-healthcare/issues/199), and the 45M load test is [#200](https://github.com/carapai/people-centered-healthcare/issues/200).

`tests/match_operation.rs` runs `$match` over SurrealDB with 2,000 synthetic patients. For 50 queries (a family-name typo and no NIN), the candidates and their counts match the in-memory pipeline exactly. A household's shared phone and family blocks are skipped by their stored counts after a recount, while the given-name keys still find the person, and a query whose every key is skipped gets the insufficient-data outcome.

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

HTTP operations (`Patient/$merge`, link and unlink) are [#197](https://github.com/carapai/people-centered-healthcare/issues/197).

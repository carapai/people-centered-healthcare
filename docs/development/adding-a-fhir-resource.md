# Adding a FHIR resource type

> Status: planned layout. No code exists yet; paths are confirmed when the scaffold lands.

## When to use

Storing, serving or searching an R5 resource type (or a new profile/category of an existing type) that the platform did not yet support.

## Steps

1. **Owner.** Assign the type, or the type + profile/category, to one Module in [resource-ownership.md](../architecture/resource-ownership.md). Add its authority row to [write-authority.md](../architecture/write-authority.md).
2. **Model.** The type comes from the adopted R5 model crate. Re-export it through `health-fhir-models`; domain code never imports the model crate directly. Add typed extension accessors there if the type carries platform extensions ([docs/fhir/README.md](../fhir/README.md)).
3. **Profile.** If the type needs a national, program or locally pinned profile, it ships in a package (national core or program kind) and validates in tier 3 ([fhir-r5-model.md](../architecture/fhir-r5-model.md)).
4. **Tables.** Add the head table `<type>`, the history table `<type>_history` and the hot projections for its common query paths through a migration ([adding-a-migration.md](adding-a-migration.md), [persistence.md](../architecture/persistence.md)).
5. **Search.** Choose which SearchParameters are supported. Only parameters declared supported in the CapabilityStatement are indexed into the `idx_*` tables. Unsupported parameters return 400 unless `Prefer: handling=lenient`.
6. **Repository** in `health-surrealdb`, owned by the Owner's repository only. Declare the tables for the table-ownership test.
7. **REST.** Expose read, vread, search, history and the allowed interactions in `health-fhir-server`, with the versioning rules (ETag, `If-Match` → 412, 410/404).
8. **References.** Add its reference-integrity rules (must resolve, or allowed-unresolved) per persistence.md.
9. **Sync.** Give it a conflict class ([adding-a-sync-resource.md](adding-a-sync-resource.md)).
10. **Client types** are regenerated from the R5 definitions by the TypeScript codegen; nothing is hand-written.
11. **R4 boundary.** If integrations send it as R4, add the conversion in `health-fhir-mapping` (with Provenance to the source).

## Expected files

- `crates/contracts/health-fhir-models/src/…` (re-export, helpers)
- `crates/infrastructure/health-surrealdb/…` (migration, repository, search translation)
- `crates/infrastructure/health-fhir-server/…` (handlers, CapabilityStatement entry)
- `crates/contracts/health-fhir-search/…` (registry entries come from the core SearchParameters; custom ones from packages)
- Fixture resources for conformance tests (location to be fixed by the scaffold)

## Contracts

`health-fhir-models` facade types; `FhirPathEngine`; the SearchParameter registry and search AST; the Owner's repository port; the CapabilityStatement.

## Tests

- **FHIR conformance:** round-trip of the R5 examples for the type; profile validation of fixtures, with the HL7 Java validator as the nightly oracle.
- **Repository:** history range scans; one `EXPLAIN` check per main query shape, so a full scan fails.
- **API contract** for the REST interactions.
- **Security:** authorization filters on search; sensitive labels.
- **Offline and sync** if it is written outside central.

## Governing decisions

[ADR 0015](../adr/0015-adopted-r5-model-crate-behind-a-facade.md), [ADR 0016](../adr/0016-validate-on-the-writing-node-and-again-at-authority.md), [ADR 0017](../adr/0017-head-history-and-range-keyed-index-tables.md), [ADR 0018](../adr/0018-version-id-is-the-op-id.md), [fhir-r5-model.md](../architecture/fhir-r5-model.md), [persistence.md](../architecture/persistence.md).

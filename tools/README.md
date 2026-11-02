# tools

Workspace tooling, such as `tools/architecture-checks`, `tools/fhir-element-table`, `tools/codegen` (the Bun generators of `@health/fhir-types` and `@health/api-client`) `tools/pki` (the PKI's ceremonies and issuance) `tools/synthetic-data` (synthetic Ugandan patients and facility data, loaded through the FHIR API) and `tools/github` (repository settings, and `prune-target.sh`, which leaves only the dependencies in `target/` before CI saves its Cargo cache). Created lazily, when the first task needs them ([crate-catalogue.md](../docs/architecture/crate-catalogue.md)).

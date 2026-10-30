# health-fhir-server

Layer: **infrastructure**. FHIR R5 REST handlers, the CapabilityStatement and Bundle assembly over `health-surrealdb` (persistence.md § REST semantics implemented here, § In code).

- **`FhirServer::new(store, base_url, policy).router()`** returns the routes relative to the FHIR base, to nest under e.g. `/fhir`. They cover `metadata`, read, vread, create, update, delete, conditional create/update/delete, transaction and batch Bundles (`POST /`), instance/type/system `_history` and `search-type` over GET and POST. The write steps live in `ops` and run inside a caller's transaction, so single requests and Bundles share them.
- **Responses:** FHIR JSON bodies with weak ETags (`W/"<versionId>"`) and `Last-Modified`. Errors are OperationOutcomes. Stored bytes are embedded as they are stored (ADR 0051).
- **Validation:** request bodies pass tier 1 (structure) and tier 2 (core invariants) before anything else reads them. `with_fhirpath_engine` replaces the default engine.
- **Not mounted in `central-server` yet:** there is no authentication or authorization until #63–#65.
- **Reference integrity:** local references must resolve (422), and a referenced resource cannot be deleted (409). `with_reference_scope` lets a node allow out-of-scope targets (persistence.md § Reference integrity).

## Tests

`tests/rest.rs` runs every fixture in `tests/fixtures/rest/` and checks that the CapabilityStatement matches behaviour:
- **Fixtures:** R5 REST conformance scenarios as request/expectation steps.
- **CapabilityStatement:** every declared interaction on all 84 types (conditional ones where `_id` is supported), and every declared search parameter. `concurrent_conditional_creates_make_one_resource` races 8 conditional creates. Without the condition lock it fails with 8 resources.

All of it runs on the SurrealDB memory engine through `oneshot`.

`examples/contract_server.rs` serves this API for the client's contract suite (`apps/client`, `bun run test:contract`). It runs two nodes, "central" and "edge", each on its own memory database and ephemeral local port. It prints their FHIR base URLs as one JSON line. It has no authentication and is only for development and CI.

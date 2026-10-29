# health-fhir-server

Layer: **infrastructure**. FHIR R5 REST handlers, the CapabilityStatement and Bundle assembly over `health-surrealdb` (persistence.md § REST semantics implemented here, § In code).

- **`FhirServer::new(store, base_url, policy).router()`** returns the routes relative to the FHIR base, to nest under e.g. `/fhir`. They cover `metadata`, read, vread, create, update, delete, instance/type/system `_history` and `search-type` over GET and POST.
- **Responses:** FHIR JSON bodies with weak ETags (`W/"<versionId>"`) and `Last-Modified`. Errors are OperationOutcomes. Stored bytes are embedded as they are stored (ADR 0051).
- **Not mounted in `central-server` yet:** there is no authentication or authorization until #63–#65, and request bodies are checked only for shape until validation tiers 1–2 (#59).
- **Follow-ups:** conditional interactions (#157), transaction and batch Bundles (#158) and reference integrity (#159).

## Tests

`tests/rest.rs` runs every fixture in `tests/fixtures/rest/` and checks that the CapabilityStatement matches behaviour:
- **Fixtures:** R5 REST conformance scenarios as request/expectation steps.
- **CapabilityStatement:** every declared interaction on all 84 types, and every declared search parameter.

All of it runs on the SurrealDB memory engine through `oneshot`.

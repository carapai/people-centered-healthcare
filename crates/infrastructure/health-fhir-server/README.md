# health-fhir-server

Layer: **infrastructure**. FHIR R5 REST handlers, the CapabilityStatement and Bundle assembly over `health-surrealdb` (persistence.md § REST semantics implemented here, § In code).

- **`FhirServer::new(store, base_url, policy).router()`** returns the routes relative to the FHIR base, to nest under e.g. `/fhir`. They cover `metadata`, read, vread, create, update, delete, conditional create/update/delete, transaction and batch Bundles (`POST /`), instance/type/system `_history` and `search-type` over GET and POST. The write steps live in `ops` and run inside a caller's transaction, so single requests and Bundles share them.
- **Responses:** FHIR JSON bodies with weak ETags (`W/"<versionId>"`) and `Last-Modified`. Errors are OperationOutcomes. Stored bytes are embedded as they are stored (ADR 0051).
- **Validation:** request bodies pass tier 1 (structure) and tier 2 (core invariants) before anything else reads them. `with_fhirpath_engine` replaces the default engine.
- **Not mounted in `central-server` yet:** there is no authentication or authorization until #63–#65.
- **Audit (#66):** `with_audit(Auditor)` records an AuditEvent in the same transaction as every write (create, update, delete, conditional writes and each Bundle entry), and every read, vread, search, history or Bundle GET that returns a sensitive record or relies on break-glass.
  - A write that fails rolls its event back with it.
  - The agent is the `health_audit::Agent` request extension, which authentication inserts (#192). Without one the request is recorded as unauthenticated.
  - AuditEvent is read-only over REST. The CapabilityStatement declares only reads and searches for it, and writes are 405.
  - AuditEvents do not count as referrers, so audited records can still be deleted.
- **Reference integrity:** local references must resolve (422), and a referenced resource cannot be deleted (409). `with_reference_scope` lets a node allow out-of-scope targets (persistence.md § Reference integrity).

- **Access control (#192):** `with_access_control(Pdp, RecordFacility)` decides every interaction for the request's `health_access_policy::Subject` request extension, which authentication inserts ([#209](https://github.com/carapai/people-centered-healthcare/issues/209)).
  - Without a subject the answer is 401; `/metadata` stays open.
  - Searches run under a per-request `SubjectSearchPolicy`, confined to a patient for `patient=` or a `subject=` naming a Patient; 403 when the type may not be searched.
  - Reads, vreads and instance history are 404 when denied. Type and system history are decided as searches, so central's patient-confined shared reads (ADR 0056) never list another facility's records.
  - Creates, updates and deletes are 403 when denied, decided on both the new resource and the version replaced.
  - `RecordFacility` finds a facility-scope record's facility. `FacilityTag` (ADR 0057) reads the `urn:health:facility` tag on the facility-scope types; `NoFacilityScope` treats no record as facility-scope (tests).
  - **Stamping (ADR 0057):** on create, a facility-scope record gets the session's facility tag. A tag naming another facility is 403, and an update keeps the facility of the version it replaces.

- **Client registry (#197, #216):** `with_client_registry(RegistryConfig, MatchingRules)` serves `POST /Patient/$match` to anyone who may search Patients. Candidates they may not see are left out, and so is their count. It also serves the reviewer operations `POST /Patient/$merge` (preview, then the reviewer's approval), `POST /Patient/{id}/$link` and `POST /Patient/{id}/$unlink` (patient-identity.md).
  - The reviewer is the request's `Subject`: without one the answer is 401, and without `registry.review` and `Patient` update it is 403.
  - A merge is for records of the reviewer's facility only.
  - Each version written is audited in its transaction, through `AuditTrail` over the server's `Auditor`, naming the reviewer.
  - The operations are not yet declared in the CapabilityStatement.

## Tests

`tests/rest.rs` runs every fixture in `tests/fixtures/rest/` and checks that the CapabilityStatement matches behaviour:
- **Fixtures:** R5 REST conformance scenarios as request/expectation steps.
- **CapabilityStatement:** every declared interaction on all 84 types (AuditEvent: reads only, on events the earlier writes produced, and its writes refused) (conditional ones where `_id` is supported), and every declared search parameter. `concurrent_conditional_creates_make_one_resource` races 8 conditional creates. Without the condition lock it fails with 8 resources.

All of it runs on the SurrealDB memory engine through `oneshot`.

`examples/contract_server.rs` serves this API for the client's contract suite (`apps/client`, `bun run test:contract`). It runs two nodes, "central" and "edge", each on its own memory database and ephemeral local port. It prints their FHIR base URLs as one JSON line. It has no authentication and is only for development and CI.

`tests/registry.rs` covers the registry operations: `$match` ranking, label filtering, 403 and 401; a previewed then approved same-facility merge, a refused cross-facility one, link and unlink audited with survivorship review queued, and reviewers only.

`tests/facility.rs` covers the facility tag: stamped on create, refused for another facility, kept through updates, and facility-isolated searches at an edge and at central.

`tests/authorization.rs` runs the authorization matrix through HTTP: no route without a subject, labels and purpose of use, permissions on every interaction (including a transaction Bundle), facility isolation in reads, writes and history, and break-glass confined to one patient.

`tests/audit.rs` checks:
- one event per write, with the agent, the entity version, the patient and the purpose;
- no event when a write fails;
- sensitive reads audited through read, vread, search, history and a Bundle GET, naming only the sensitive record and its label;
- ordinary reads not audited;
- break-glass reads audited with `BTG`;
- unauthenticated requests recorded as such.

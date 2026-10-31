# health-identity-app

Layer: **application** (Identity & Access). The policy decision point applied where repositories run queries.

- **`SubjectSearchPolicy`** is a `health_fhir_search::SearchPolicy` for one subject's request.
  - It turns `Pdp::search_filter` into a `_security:not=…` clause, which excludes records carrying any restricted label and keeps those with no labels. So restricted records are filtered inside the SurrealDB query, never after it.
  - Without search permission it adds a clause that nothing satisfies. `check` lets the caller answer 403 first.

`health-fhir-server` builds one per request under access control (#192). Its `tests/authorization.rs` runs the authorization matrix through the HTTP API.

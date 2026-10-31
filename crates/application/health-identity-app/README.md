# health-identity-app

Layer: **application** (Identity & Access). The policy decision point applied where repositories run queries.

- **`SubjectSearchPolicy`** is a `health_fhir_search::SearchPolicy` for one subject's request.
  - It turns `Pdp::search_filter` into a `_security:not=…` clause, which excludes records carrying any restricted label and keeps those with no labels. So restricted records are filtered inside the SurrealDB query, never after it.
  - Without search permission it adds a clause that nothing satisfies. `check` lets the caller answer 403 first.

## Tests

`tests/search.rs` runs the real `FhirServer` over SurrealDB (memory engine):
- labelled records stay out of every search, including one that asks for the label;
- programs open their own labels and no others;
- a subject without search permission gets nothing;
- break-glass opens one patient's records only in a search confined to that patient.

Applying the PDP to every FHIR interaction, per request, is [#192](https://github.com/carapai/people-centered-healthcare/issues/192).

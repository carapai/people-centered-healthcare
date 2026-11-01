# health-identity-app

Layer: **application** (Identity & Access). The policy decision point applied where repositories run queries.

- **`subject_of(snapshot, roles, purpose)`** is a request's `Subject`, built from the user's verified permission snapshot at the session's facility: the permissions of their roles there (`RolePermissions`), their programs and the declared purpose (ADR 0056). `tests/subject.rs` covers it.
- **`SubjectSearchPolicy`** is a `health_fhir_search::SearchPolicy` for one subject's request.
  - It turns `Pdp::search_filter` into a `_security:not=…` clause, which excludes records carrying any restricted label and keeps those with no labels. So restricted records are filtered inside the SurrealDB query, never after it.
  - For a facility-scope type it adds `_tag=urn:health:facility|{facility}` (ADR 0057), unless the filter has no facility.
  - Without search permission it adds a clause that nothing satisfies. `check` lets the caller answer 403 first.

`health-fhir-server` builds one per request under access control (#192). Its `tests/authorization.rs` runs the authorization matrix through the HTTP API.
- **`caregivers`** (#113): caregiver delegation over any `Store` whose transaction is a `health_consent::GrantStore`.
  - `Caregivers::record` is staff recording a caregiver against a dependent at the facility, verified against the document they checked (one grant per caregiver and patient; `AlreadyGranted` while one is in force). `revoke` and `adolescent_consent` change it.
  - Each change writes the grant and its FHIR projections, the RelatedPerson and the Consent, in one transaction, each audited through `CaregiverAudit`.
  - The Consent is `active` or `inactive`. It carries the scopes on `provision.action` (`urn:health:caregiver-scope`), the expiry on `period.end`, the verification, and the authority and adolescent's consent as extensions.
  - `of_patient` and `of_caregiver` list grants.

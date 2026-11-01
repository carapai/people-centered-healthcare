# health-consent

Layer: **domain** (Identity & Access, consent). Caregiver delegation (#113; patient-identity.md § Caregiver delegation; ADR 0029; security.md §96).

- **`CaregiverGrant`** is a caregiver's access to one dependent. It brings together the RelatedPerson, a Consent with its `Scope`s (view records, book appointments, request refills, receive messages) and an expiry, and a `Verification`.
  - **Authority:** `Guardian` (of a minor) or `Delegated` (by an adult patient).
  - **Verification:** `StaffChecked` (documents), or `Delivery`. `CaregiverGrant::at_delivery` is a mother's grant over her newborn, recorded at a facility delivery: verified at once, one per baby of a multiple birth.
- **`decide(policy, request)`** is the rule the policy decision point applies with the request's labels:
  - a revoked, unverified, expired or out-of-scope grant gives nothing;
  - a guardian has no access once the dependent reaches the age of majority (`Confidentiality::age_of_majority`, default 18), nor when the age is unknown;
  - from `adolescent_age` (default 12), a guardian cannot see records with a sensitive label (`Confidentiality::sensitive`: HIV, SRH, mental health, from the configuration package) unless the adolescent consents (`adolescent_consent`, which can be withdrawn);
  - a delegate gets what the adult granted.
- **Uncertain ages are protective.** Ages come from the earliest birth date a partial `birthDate` allows (`Day::earliest`). So guardianship ends, and adolescent confidentiality starts, as early as the record allows.
- **`revoke(by, …)`:**
  - **Staff:** at any time.
  - **A guardian:** only one with a verified, live guardian grant over the same patient.
  - **The patient:** a delegation at any time. A guardian's grant only once of age by the *latest* birth date the record allows, so a minor of uncertain age cannot end it. Revocation takes effect at the next sync.
- **`GrantStore`** is the storage port: a grant is one record, found by id, patient or caregiver. `health-surrealdb` implements it (migration `0014_caregiver_grant`).
- **`Day`:** a calendar day; ages in whole years, with 29 February birthdays on 1 March.

## Tests

`tests/caregiver.rs` is an authorization matrix covering:
- guardians of children, adolescents (with and without consent, including the 12th birthday and a birth year only), adults and unknown ages;
- delegates;
- unverified, expired, out-of-scope and revoked grants;
- who may revoke;
- mother–newborn grants for twins;
- days and ages.

**Not here yet:** the staff and portal screens and e2e (#113 part 3), recording the delivery link at a facility delivery, and applying `decide` to record reads (the portal has no records view yet).

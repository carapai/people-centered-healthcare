# health-patient-engagement-app

Layer: **application** (Patient Engagement). Self-registration (#111; patient-identity.md § Self-registration; ADR 0029) over any `health_persistence::Store`.

- **`SelfRegistration::submit(account, phone, response)`:** a patient signed in with their phone (`level1`) submits the self-registration QuestionnaireResponse (`QUESTIONNAIRE`: `given`, `family`, `birthDate`, `gender`). The use case:
  - builds the Patient from the answers and the account's verified phone (`patient_of`);
  - runs `$match` through the `PatientRegistry` port and routes the request with `health_patient_engagement::registration`:
    - **No candidate:** the port creates a self-registered source Patient (provisional, patient-reported, no MRN, never auto-linked). The account's Person links to it at `level1`, and the request completes. The patient sees only what they submitted.
    - **Any candidate, or too little to match on:** the request waits for staff review. Nothing is linked, and the account sees nothing.
  - writes the QuestionnaireResponse (tagged `patient-reported`), the Person and the Registration Request's Task in one transaction, each audited through `AuditTrail`.
- **`review(account, decision, reviewer)`** (#111 part 2c): a reviewer's decision on a request waiting for review.
  - **`StaffDecision::Link { patient, document }`:** only to one of the request's candidates, and only with the ID document the reviewer checked in person. That verification raises the request to `level3`, and the Person links to the existing Patient at `level3`, so the account sees released records.
  - **`StaffDecision::Create`:** a new self-registered Patient from the submitted answers, linked at `level1`.
  - The Task, the Person and the queue item change in one transaction. A decision another reviewer made first is `AlreadyDecided`, and changes nothing.
- **`status(account)`:** the account's request, its Patient and its `PortalAccess`.
- **One request and one Person per account.** Their ids, and the new Patient's, are derived from the account (UUIDv5), so a retry never duplicates them. A second submission is `AlreadySubmitted`.
- **The FHIR forms:**
  - The Task is `intent=proposal`. The §59 state is its `businessStatus` (`urn:health:registration-status`). The assurance, route, candidates and reviewer are its `input`, and the created or linked Patient its `output`.
  - The Person has its Login Account as an identifier, and `link` entries with `assurance`.
- **Ports:**
  - `PatientRegistry`: `find`, `create_self_registered`. `health-fhir-server` implements it over the client registry, since application crates of different contexts don't depend on each other.
  - `RegistrationReviews`: `open` queues a request in the registry review queue, and `close` records the decision, both in the use case's transaction. `health-fhir-server` implements it over `ReviewQueue`, with the queue item's id being the request's.
  - `AuditTrail`.

## Tests

`tests/self_registration.rs`, over SurrealDB's memory engine with a fake registry:
- no match creates a self-registered Patient that sees only what it submitted;
- a certain match on a phone account goes to review and links nothing;
- too little to match on also goes to review;
- one registration per account;
- invalid answers are refused, and nothing is written;
- a reviewer links only a candidate, and only with an in-person check, which gives `level3` access;
- a reviewer may create a new Patient instead;
- a decision another reviewer made first changes nothing.

**Not here yet:** the portal screens, central wiring and e2e (#111 part 3).

# health-patient-engagement

Layer: **domain** (Patient Engagement). Patient access starts here as a module (crate-catalogue.md), with the self-registration Registration Request (#111; patient-identity.md § Self-registration; ADR 0029).

- **`registration::RegistrationRequest`:** a patient-reported QuestionnaireResponse and its §59 state, projected as a Task: Draft → Submitted → Verified → (Review →) Matched → Completed.
  - **Assurance** starts at `level1` (the account's phone code) and only rises: a NIN check, or a reviewer's in-person check.
  - **`route` after `$match`:**
    - no candidate creates a new self-registered source Patient, provisional until `level3`;
    - a certain match links only at `level3`;
    - anything else, including a certain match below `level3`, goes to staff review.
  - **Review:** a reviewer may link only once the holder is at `level3`, or create a new Patient.
  - **`complete`** accepts only the outcome of the decided route.
- The client registry's `Assurance` is mirrored here as this crate's own (no domain→domain edges, ADR 0006); the application layer maps between them.

## Tests

`tests/registration.rs`: the routes, a phone account with a certain match going to review rather than a link, review needing `level3` to link, and steps in order.

**Not here yet:** the use cases, persistence and HTTP (#111 part 2), and the portal screens and e2e (part 3).

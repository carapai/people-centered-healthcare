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
- **`requests`** (#114; MASTER_PROMPT §60): a patient's request is a proposal, never an order.
  - `RequestKind` names the three proposals: a refill (MedicationRequest `intent=proposal`), a test or other care (ServiceRequest `intent=proposal`), or an appointment (Appointment `proposed`).
  - `RequestState` reads the proposal's status: pending, accepted, declined or withdrawn.
  - `may_close` allows withdrawing, declining or accepting only while pending.
  - `decline_reason` requires a reason.
- The client registry's `Assurance` is mirrored here as this crate's own (no domain→domain edges, ADR 0006); the application layer maps between them.

## Tests

`tests/registration.rs`: the routes, a phone account with a certain match going to review rather than a link, review needing `level3` to link, and steps in order.

`requests` unit tests: each kind's proposal and closing statuses, and closing only from pending.

**Not here yet:** the use cases, persistence and HTTP (#111 part 2), and the portal screens and e2e (part 3).

# health-encounters-app

Layer: **application** (encounters). Encounters at the facility authority (#102). Each step is one transaction: the Encounter, its open-encounter guard, its FHIR projection and its outbox event.

- **`Encounters::check_in`:** claims the patient's guard at the service point (`AlreadyOpen` while an Encounter is open there), starts the Encounter, writes the FHIR R5 Encounter and puts `encounters.started.v1` on the outbox.
  - **The FHIR Encounter:** `status`, `class`, `subject`, `actualPeriod`, and the service point as its `location`, tagged with its facility (ADR 0057).
- **`transition`:** follows the R5 lifecycle, and an Encounter that ends releases its guard.
- **`move_to`:** releases the guard where the patient was, and claims it where they are sent.
- **Ownership:** only this module changes the Encounter's status. Patient-flow follows the events (`health-patient-flow-app`), and who may check in, end or move is the caller's permission check.

## Tests

`tests/check_in.rs`, with patient-flow over SurrealDB:

- a checked-in patient joins the triage queue once however often the event is replayed;
- a second open Encounter at the service point is refused;
- moving and ending move and close the queue entries and release the guard;
- an emergency is seen first;
- the projected Encounter passes tier 1.

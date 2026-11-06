# Patient requests

The client side of #114 (patient-identity.md § Patient-initiated workflows; MASTER_PROMPT §60). **A patient's request is a proposal: only a member of staff turns it into an order.**

- **The portal** (`MyRequests`, on the portal page once registration is complete) lists the patient's requests and where each is: waiting for staff, accepted or closed, with any reason.
  - A patient may ask for a test or other care, or for an appointment, at a facility they name by code.
  - With full access (`level3`), a patient may also ask for a refill of one of their open prescriptions.
  - A pending request may be withdrawn.
  - Each request has a client-generated UUIDv7 id (ADR 0013).
- **Staff** (`RequestActions`, on the record page an inbox item opens): for a pending proposal, staff accept or decline.
  - **Accept** writes the order: a refill is prescribed through the medication safety checks (warnings shown, then acknowledged), a care request becomes an order, and an appointment is booked for the start and end entered.
  - **Decline** needs a reason.
  - The inbox lists each kind to whoever may act on it: refills with `medications.prescribe`, care with `orders.request`, and appointments with `appointments.book`.
- **The API** (`api.ts`):
  - the portal's `/portal/requests`, `/portal/prescriptions` and `/portal/requests/{type}/{id}/withdraw`;
  - staff's `/requests/{type}/{id}/accept` and `/decline`.

## Tests

`e2e/requests.spec.ts`: a new patient asks for a test and withdraws a second request. The clinician finds the first in the inbox, still a proposal, and accepts it. Only then is it accepted for the patient, and it leaves the inbox. The withdrawn one never reaches the inbox. axe runs on each screen.

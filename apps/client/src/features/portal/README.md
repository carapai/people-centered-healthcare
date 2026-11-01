# Patient portal

The patient portal (patient-portal.md; ADR 0029; #111), at `/portal`, central only.

- **Sign-in:** a patient signs in with a one-time code to their phone, every time; there is no patient password.
  - The portal has its own client (`runtime.json` `portal`), sign-in, session and API transport (`Platform.portal`), apart from any staff session.
  - `/sign-in` asks the portal's client for the phone (posted to `/auth/otp`), then the code (posted to `/auth/authorize`), with the server's refusals (`phone`, `wait`, `refused`, `expired`) explained.
- **`PortalPage`:** before a registration, the form: given and family names, date of birth and sex, sent as the self-registration QuestionnaireResponse (`api.ts`). After it, the status:
  - waiting for review, with **Check again**;
  - registered, seeing only what the patient submitted;
  - linked to their health record (`level3`, after an in-person ID check).
- **People you care for** (#113): a caregiver's dependents, until when, and a note when a young person's sensitive records stay private to them, with a refresh button.
- **On the server:** `/api/portal/registration` (`health-fhir-server`'s `portal_router`) decides everything. The screen enforces nothing.
- **Not yet:** the portal's own UX (fog in patient-portal.md), requests, appointments and records.

## Tests

- `test/portal.test.ts`: the API, the answers as a QuestionnaireResponse, and the portal's separate session.
- `e2e/portal.spec.ts`: phone sign-in through the development code outbox, a new patient registering, and a lookalike linked by a reviewer after an in-person check, with axe on each screen.

# Caregivers

Caregiver delegation for staff (patient-identity.md § Caregiver delegation; #113), on the patient view (`/patients/{id}`).

- **`CaregiversPanel`** lists a patient's caregivers: guardian or delegated, the status (active, revoked, expired, not verified), until when, and the scopes.
  - For a guardian, whether the young person consents to them seeing sensitive records.
  - **Record caregiver:** staff record a caregiver by the phone of their portal account (they sign in to the portal once first). They choose the relationship, the authority, the scopes and the last day of access, and give the document they checked in person. The server refuses a phone with no portal account, and a second grant while one is in force.
  - **Record or withdraw the young person's consent**, and **Revoke access**.
- **`api.ts`:** `/api/caregivers` for staff (`caregiver.manage`), and `dependentsQuery`, the portal's `/api/portal/dependents`.
- **On the server:** every decision is checked and audited (`health-auth::CaregiverRoutes`). The screen enforces nothing.

## Tests

- `test/caregivers.test.ts`: the API.
- `e2e/caregivers.spec.ts`: a mother recorded as her adolescent daughter's guardian sees her in the portal with her sensitive records withheld until the daughter consents, and not at all once revoked. Axe runs on each screen.

# Patient portal

This page consolidates the decided parts of the patient workspace (MASTER_PROMPT §57–62). Identity, assurance, caregiver and request rules are decided in [patient-identity.md](patient-identity.md) (ADRs 0027–0029). Portal UX, notifications, payments in the portal, and result-release rules are **open**.

## Where it runs

- **Central only.** The portal is a central, online feature area of the one React app. Patient Engagement is global, with central as its authority (write-authority.md).
- **No offline portal.** There is no offline portal and no clinical data in browser storage (ADR 0037).
- **Facilities don't host it.** Edges never serve the portal. Patient-originated records reach a facility through the owning clinical context at that facility.

## Capabilities (§57) and their basis

| Capability | Basis | Status |
|---|---|---|
| Account creation | Login Account per verified phone number (identity) | Decided |
| Self-registration | §59 flow: Draft → … → Completed | Decided |
| Identity verification | Assurance levels: OTP `level1`; NIRA or in-person `level3` | Decided |
| Sign-in | A one-time code to the verified phone, every time; no patient password | Decided (#111) |
| Appointment booking | `Appointment.status=proposed`; staff confirm | Decided |
| Pre-visit questionnaires, patient-reported outcomes, symptom reporting | Patient-reported QuestionnaireResponses (patient-engagement); `seed`-only extraction, unconfirmed | Decided |
| Refill and care requests | `MedicationRequest` / `ServiceRequest` with `intent=proposal`, or a `Task` | Decided |
| Demographic updates | Self-reported Candidate Versions, reviewed by the registry | Decided |
| Health record access, released results | Requires `level3` and a link to a Patient; PDP and sensitivity labels apply | Access decided; **which results are "released", and when, is open** |
| Consent | Consent records; scoped, expiring caregiver grants | Decided |
| Authorized caregiver access | RelatedPerson + Consent + verification; adolescent confidentiality | Decided |
| Patient education, secure messages, notifications | Communications | **Open** (communications fog) |
| Payments | Mobile money via central provider integration (ADR 0032) | Mechanism decided; **portal payment UX is open** |
| Feedback | — | **Open** |

## In code (#111)

- **Sign-in:** a one-time code to the phone, every time (`health-auth`; codes go to a development outbox until #235).
- **Self-registration:** `health-patient-engagement-app`, at `/api/portal/registration`. A registration that resembles existing patients waits in the registry review queue. A reviewer links it only after checking the patient's ID in person.
- **Screens:** `/portal` in the web client: sign-in, the registration form and its status. These are minimal working screens; the portal's UX is still open.

## Rules

- **No automatic linking (§58).** A phone account is never linked to a Patient automatically.
- **Requests are proposals (§60).** Patient requests never become orders automatically.
- **No overwriting (§62).** Patient-reported data never overwrites clinician-verified records.

## Open (fog)

- Portal UX.
- Result-release policy (which results, after what clinician action or delay).
- Messaging and notification channels, including the SMS provider for one-time codes.
- Portal payments and feedback.

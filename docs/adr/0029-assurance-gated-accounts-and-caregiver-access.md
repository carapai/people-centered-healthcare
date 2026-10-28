# Accounts reach patient data only through a Person, at a verified assurance level, and caregivers only through RelatedPerson plus Consent

The five §58 concepts are separate records:

| Concept | Record | Owner |
|---|---|---|
| Login Account | One per verified phone number | identity |
| Registration Request | A patient-reported QuestionnaireResponse plus the §59 state, projected as a Task | patient-engagement |
| Identity verification | VerificationResult | client-registry |
| Patient | Source and golden Patients | client-registry |
| Caregiver relationship | RelatedPerson, plus Consent | client-registry, consent |

**Account to Person.** A Login Account maps to one `Person`, and `Person.link` names what the account may act as, each with an assurance level:
- `level1`: phone one-time code;
- `level3`: a NIRA NIN check, or an in-person check of an ID document.

**Linking to an existing Patient** requires `level3`. A phone account is never linked automatically. Below `level3`, a patient sees only the data they submitted themselves.

**Caregiver access** needs three things:
- a `RelatedPerson`;
- a Consent with a scope (view, book, request refills, messages) and an expiry;
- verification, either by staff checking documents or automatically for mother–newborn links recorded at a facility delivery.

**Minors and adolescents.**
- Guardians have access up to a configurable age of majority (default 18).
- From a configurable adolescent age (default 12), records with sensitive labels (HIV, SRH, mental health) are excluded from guardian access unless the adolescent consents.

**Patient input stays proposals.** Patient requests are created as proposals, never orders:
- `intent=proposal`;
- `Appointment.status=proposed`;
- `Task`.

Patient-reported data is `seed`-only, tagged `patient-reported` and unconfirmed, and never overwrites clinician-verified records (§62).

We chose this because shared phones, the lack of a NIN for many people, and adolescent confidentiality make "the phone number matches" unsafe as evidence of identity or authority.

## Considered Options

- **Link an account to a Patient whose phone number matches.** Rejected outright by §58.
- **Patient.contact for caregivers.** Rejected: a contact cannot be a reference target or an actor.
- **Unrestricted guardian access to minors' records.** Rejected: it would expose adolescents' sensitive care.

## Consequences

- A shared family phone means one account, with other family members reached through caregiver links.
- The policy decision point evaluates Person links, Consent scope, assurance level and confidentiality labels together (ADR 0011).

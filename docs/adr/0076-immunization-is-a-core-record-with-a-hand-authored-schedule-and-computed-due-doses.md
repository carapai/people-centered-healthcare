# Immunization is a core record with a hand-authored schedule and computed due doses

Decided in [How does the platform support immunization: schedule, doses, outreach, defaulters and reporting?](https://github.com/carapai/people-centered-healthcare/issues/606), after the research [What does immunization in Uganda require: schedule, reporting, existing systems and WHO SMART IMMZ?](https://github.com/carapai/people-centered-healthcare/issues/605) ([immunization.md](https://github.com/carapai/people-centered-healthcare/blob/research/immunization/docs/research/immunization.md), branch `research/immunization`). That research found that Uganda's schedule has moved past WHO SMART IMMZ:
- R21 malaria vaccine at 6, 7, 8 and 18 months in listed districts, where IMMZ models RTS,S;
- single-dose HPV.

It also found three other facts:
- IMMZ is FHIR R4, and its content licence (CC BY-NC-SA 3.0 IGO) conflicts with the CC0 its own configuration declares.
- Most children under one have no NIN.
- No national immunization registry or vaccine eLMIS exists to integrate with.

**The decision:**
- **The record is core; the content is a package.**
  - Every Patient has a lifelong immunization history in `health-immunization`, with no Program enrollment needed.
  - The **Immunization program package** (national, maintained by UNEPI) carries what changes: the schedule's Decision Support Rules, the HMIS 105 EPI Report, the defaulter Indicators and the defaulter threshold.
- **The schedule is hand-authored.**
  - It is written as Decision Support Rules (ADR 0074) from:
    - the UNEPI schedule in UCG 2023;
    - R21 by district;
    - single-dose HPV, plus 3 doses for immunocompromised girls;
    - the Td series for women aged 15–49.
  - The rules follow the structure of IMMZ's decision tables (minimum age, minimum interval, catch-up limits) but copy none of their content.
  - Importing IMMZ waits until WHO resolves its licence.
- **Due doses are computed, not stored.**
  - Rules run on the node holding the record at `patient-view` and after each dose.
  - A per-facility **Due List** Read Model is recomputed daily and after each dose. It feeds the session worklist and defaulter tracing.
  - No `ImmunizationRecommendation` is stored. One may be produced on export.
- **A dose comes from Stock.**
  - The vaccinator picks the vaccine; the batch is suggested earliest-expiry-first (FEFO, ADR 0035), and lot and expiry are filled in from it.
  - The `Immunization` records site, route, dose, performer, location and the Vaccination Session.
  - VVM stage 3 or 4 blocks a vial when it is picked and records a discard.
- **Open vials are tracked.**
  - Opening a multi-dose vial takes the whole unit out of Stock, and doses are counted against it.
  - Discard rules:
    - outreach vials at session end;
    - reconstituted BCG, YF and MR after 6 hours or at session end.
  - Wastage is doses opened minus doses given.
- **Historical Doses count for the schedule, not for reporting.** Doses from the card, the register or another facility are `Immunization` with `primarySource=false` and their source. They never enter this facility's HMIS 105.
- **Children are their own Patients from first contact.**
  - Each child is identified by the child health card number, with the NIN added when issued.
  - The mother or carer is linked as a Caregiver, whose phone receives messages.
  - A birth recorded in maternity creates the child.
- **Defaulters are traced from the Due List.**
  - A Defaulter has a due dose more than 28 days overdue (a package setting).
  - The list is grouped by village. Staff can turn entries into Tasks.
  - Caregivers who opted in get SMS reminders before each due date (ADR 0073).
  - VHTs are not app users in v1; they get a printed or exported list.
- **v1 scope.**
  - In: routine static sessions, and outreach sessions under an Authority Lease (ADR 0012).
  - In: AEFI recorded as an `AdverseEvent` linked to the dose.
  - In: the record shown to Caregivers in the patient portal.
  - In: a QR code of the child's identifier for lookup.
  - Deferred: campaigns, AEFI to eIDSR, and weekly mTrac reports.

## Considered options

- **Importing WHO SMART IMMZ:**
  - its licence is in conflict;
  - it is R4;
  - it models RTS,S, not R21;
  - its output is `MedicationRequest` proposals.
- **Stored `ImmunizationRecommendation`s:**
  - they go stale whenever a dose, a birth date or the schedule changes;
  - they would need sync and conflict handling;
  - R5 rates them maturity 1.
- **Immunization as an enrollment Program:** a child seen once at outreach would have to be enrolled before a dose could be recorded, and adults' Td and HepB doses would sit outside any history.
- **Children identified only through the mother's record:** fails for children brought by other carers, and for mothers who were never registered.

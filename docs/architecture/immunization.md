# Immunization

The lifelong vaccination record, the schedule, sessions, defaulters and EPI reporting. Decided in [How does the platform support immunization: schedule, doses, outreach, defaulters and reporting?](https://github.com/carapai/people-centered-healthcare/issues/606) ([ADR 0076](../adr/0076-immunization-is-a-core-record-with-a-hand-authored-schedule-and-computed-due-doses.md)). Facts are in the research note on branch `research/immunization`.

## Core and package

| Core (`health-immunization`) | Immunization program package (national) |
|---|---|
| `Immunization` and `AdverseEvent` records | Schedule as Decision Support Rules |
| Vaccination screen, history, Due List Read Model | HMIS 105 EPI Report and its Indicators |
| Vaccination Sessions and open-vial tracking | Defaulter threshold (default 28 days) and Indicators |
| Historical Doses | Reminder Message templates (Language Packs) |

No enrollment: every Patient's history is in core. A schedule change is a package update.

## Schedule

Hand-authored from UCG 2023 and later changes, not imported from WHO SMART IMMZ, whose content licence is in conflict and which models RTS,S:
- BCG, HepB birth dose, OPV 0–3, IPV ×2, DPT-HepB-Hib ×3, PCV ×3, rotavirus ×2, MR ×2 and YF;
- R21 malaria at 6, 7, 8 and 18 months, in the districts listed in the package;
- single-dose HPV for girls aged 9–14, and 3 doses for immunocompromised girls;
- Td 1–5 for women aged 15–49, and HepB 0, 1, 6 months for high-risk adults.

Each rule states its minimum age, minimum interval from the previous dose and maximum age for catch-up, following IMMZ's table structure. The rules ship with test patients, as ADR 0074 requires.

## Due doses

- **Advice** at `patient-view` and after each dose: due, overdue, or too early with the earliest valid date.
- **Due List:** a facility Read Model of every Patient with a dose due or overdue, recomputed daily and after each dose. It feeds:
  - the session worklist;
  - the defaulter list;
  - SMS reminders.
- Nothing is stored as `ImmunizationRecommendation`; one can be produced on export.

## Recording a dose

1. Find the child: by name, card number or a scanned QR code.
2. Pick the vaccine. The batch is suggested earliest-expiry-first (FEFO) from the session's or store's Stock, and lot and expiry are filled in.
3. Enter the site, route and dose number. Performer, location and Vaccination Session are filled in.
4. On save, Advice shows the next due date.

VVM is checked when a vial is first picked. Stage 3 or 4 blocks it and records a discard.

## Open vials and wastage

- Opening a multi-dose vial issues the whole unit from Stock; doses given are counted against the open vial.
- Discards:
  - outreach vials at session end;
  - reconstituted BCG, YF and MR after 6 hours or at session end.
- Other vials follow the multi-dose vial policy set in the package.
- Wastage per antigen = doses opened − doses given. It is reported, and it can be checked against the ledger.

## Historical Doses

A dose from the card, the register or another facility is recorded as `Immunization` with `primarySource=false`, its source, and a date that may be approximate. It counts for the schedule but never for this facility's HMIS 105.

## Children and Caregivers

- A child is a Patient from first contact, identified by the child health card number. The NIN is added when issued.
- The mother or carer is linked as a Caregiver ([patient-identity.md](patient-identity.md)), and their phone receives reminders.
- A birth recorded in maternity creates the child's Patient.
- Caregivers see the child's doses and next due dates in the patient portal ([patient-portal.md](patient-portal.md)).

## Sessions

- **Static sessions** run at the facility.
- **Outreach sessions** take an Authority Lease over a sub-store and the session's records (ADR 0012, ADR 0035). They run offline on a desktop and return at session end.
- Each dose records its session type, which HMIS 105 needs.

## Defaulters

- A **Defaulter** has a due dose more than 28 days overdue (a package setting).
- The defaulter list is grouped by village and filterable by antigen. Staff can turn entries into Tasks.
- Caregivers who opted in to the reminders category get an SMS before each due date ([communications.md](communications.md)).
- VHTs are not app users in v1; the list can be printed or exported.

## Reporting

- The HMIS 105 EPI section is a Report in the package ([analytics.md](analytics.md)). It is split by antigen, dose, age band and static or outreach.
- Its current layout and DHIS2 identifiers are not yet verified. The latest machine-readable template found dates from 2017.
- Wastage and "fully immunized by 1 year" are Indicators.

## Campaigns

Decided in [ADR 0086](../adr/0086-campaigns-are-configured-centrally-and-tallied-unless-individual-recording-is-chosen.md); facts on branch `research/immunization-campaigns`.

- **Child Health Days** (April and October) are routine static and outreach sessions tagged with the round. Doses count for the schedule; vitamin A and deworming are recorded as medication given at the session. HMIS 075/076 views cover each round.
- A **Campaign** is configuration published from central: vaccines, eligibility, dates, districts, target population per facility, recording mode, and whether its doses count for the schedule.
- **Recording mode:**
  - **tally** (default for mass campaigns): a **Campaign Tally** per post and day, with doses by vaccine and age band and vials issued, used and discarded by reason;
  - **individual** (targeted campaigns such as mpox): an `Immunization` per dose, marked with its Campaign.
- Campaign doses do not satisfy routine doses or clear the Due List unless the Campaign is set to count.
- **Micro-plans** are planned Vaccination Sessions with site, date, team, VHTs and target population.
- Campaign teams take a sub-store under an Authority Lease and return used and discarded vials by reason each day.
- A daily **Campaign Report** shows coverage against target, doses and wastage by district, facility and day. Nothing is sent to EpiVac or ODK.
- Individually recorded campaign doses print on the Child Health Card printout or a campaign card. No verifiable digital certificate yet.

## AEFI

An adverse event following immunization is an `AdverseEvent` linked to the `Immunization`. Sending it to eIDSR waits for case-based surveillance.

## Not in v1

- Weekly mTrac (HMIS 033b) reports.
- AEFI to eIDSR.
- Printing the child health card.
- VHT app access.

# Care journeys and the patient timeline

This page consolidates the decided basis for the Care Journey (MASTER_PROMPT §61) and the patient timeline (§105). Both are **read models**: projections that own nothing and are rebuilt from events (resource-ownership.md rule 8).

## What they are

| Read model | Owner module | Contents |
|---|---|---|
| Care Journey | care-journeys (Patient Engagement) | A longitudinal, patient-centred projection linking registration, appointments, encounters, programs, orders, Tasks, results, medication activities, referrals, follow-up and communication |
| Longitudinal patient summary / timeline | shared-record (Clinical Records) | An authorized activity timeline spanning Encounters, EpisodesOfCare, Observations, orders, medications, referrals and communications |

## Rules

- **Ownership is preserved.** Every entry links to the authoritative resource, owned by its Owner and Authority Node. The projection never edits or replaces it (§61, §105).
- **One person, many sources.** Entries are gathered across all of a person's Source Patients through the golden record (ADR 0027), so records from different facilities and provisional registrations appear together.
- **Authorization at read time.** The PDP filters entries by role, facility, program assignment, sensitivity labels, Consent and assurance level. Caregivers and adolescents are covered by the rules in patient-identity.md.
- **Where it runs.**
  - Central builds the full cross-facility projection.
  - An edge builds it only from its own Sync Scope; other facilities' entries come from central's shared record on demand.
  - Desktops build it only from their scope.
- **Workflow context.** Program and episode context comes from `Encounter.episodeOfCare` and the `workflow-episodeOfCare` extension. Workflow state comes from Task projections, never from clinical status (ADR 0042).
- **Rebuildable.** Projections can be rebuilt from events and resources at any time. They are not synced as authoritative data.

## Open (fog)

- Journey and timeline UX.
- Which communications appear (communications fog).
- The analytics projections used for program indicators.

# Campaigns are configured centrally and tallied unless individual recording is chosen, and campaign doses do not replace routine doses

Decided in [How does the platform support immunization campaigns: Child Health Days and mass campaigns?](https://github.com/carapai/people-centered-healthcare/issues/741), after the research [What do immunization campaigns, weekly surveillance and AEFI reporting require in Uganda?](https://github.com/carapai/people-centered-healthcare/issues/740) ([immunization-campaigns.md](https://github.com/carapai/people-centered-healthcare/blob/research/immunization-campaigns/docs/research/immunization-campaigns.md), branch `research/immunization-campaigns`). It extends [ADR 0076](0076-immunization-is-a-core-record-with-a-hand-authored-schedule-and-computed-due-doses.md), which deferred campaigns.

The research found:
- Integrated Child Health Days (ICHD) run every April and October at facilities and outreach sites, with facility and district micro-plans, the child register (HMIS 073), the tally sheet (Form 076) and the Child Health Card, reported through HMIS 075/076 into HMIS 105.
- Mass campaigns are very large and short (8.3 M children aged 9–59 months in five days in October 2026), run largely at schools and community posts, and were monitored outside routine records, by ODK feeding a DHIS2 dashboard or by EpiVac, which is not integrated with DHIS2.
- MoH and UCG: campaign doses do not replace routine doses.
- Targeted campaigns such as mpox (2025) vaccinate small adult key populations.

**The decision:**
- **Campaign.** Configuration published from central to the edges: name, vaccines, eligibility (an age range and optional criteria), start and end dates, the districts covered, the target population per facility, a "regardless of prior status" flag, a recording mode and whether its doses count for the routine schedule.
- **Child Health Days are not Campaigns.** An ICHD round is a set of routine static and outreach Vaccination Sessions tagged with the round. Doses are recorded per child and count for the schedule as usual; vitamin A and deworming are recorded as medication given at the session.
- **Recording mode.**
  - **Tally** (the default for mass campaigns): a **Campaign Tally** per post and day records doses by vaccine and age band, and vials issued, used and discarded by reason. No Patient record is needed.
  - **Individual** (for targeted campaigns): each dose is an `Immunization` on the person's record. Staff may also record a known child individually during a tally campaign.
- **Campaign doses and the schedule.** An individually recorded campaign dose is in the lifelong record, marked with its Campaign. By default it does not satisfy a routine dose or clear the Due List; a Campaign can be set to count, for a catch-up campaign.
- **Micro-plans** are planned Vaccination Sessions for a round or Campaign: site, date, team, VHTs and target population. District and national views add up facility plans. Budgets and district logistics are out.
- **Stock.** Campaign teams take a sub-store under an Authority Lease, like outreach (ADR 0012, ADR 0035): vials are issued from Stock and returned at the end of each day as used or discarded by reason (partial use, contamination, VVM change, other), from which wastage is calculated.
- **Reporting.** A daily **Campaign Report** (coverage against target, doses, wastage) on the central dashboard, by district, facility and day. HMIS 075/076 views cover ICHD rounds. No export to EpiVac or ODK in v1.
- **Proof of vaccination.** Individually recorded campaign doses print on the Child Health Card printout or a campaign card. A verifiable digital certificate waits for the national PKI decision.

## Considered options

- **Individual records for every mass-campaign dose:** five days of school and community posts with millions of children cannot wait for registration; past campaigns tallied, and coverage is what is monitored.
- **Campaign doses always counting for the schedule:** contradicts the MoH rule and would hide children who still need routine doses.
- **Treating ICHD as a Campaign:** ICHD gives the child's scheduled doses at ordinary sessions, and HMIS 105 counts them as routine.
- **Sending campaign data to EpiVac or ODK:** EpiVac's host and API are unknown, and ODK served only the 2019 campaign's monitoring.

# Surveillance

Outbreak threshold alerting and national submission. Decided in [Does the platform raise outbreak threshold alerts and submit surveillance data electronically?](https://github.com/carapai/people-centered-healthcare/issues/799) ([ADR 0091](../adr/0091-outbreak-alerts-are-computed-from-a-national-surveillance-package-and-033b-may-go-to-dhis2.md)); facts on branch `research/surveillance-alerting`. The weekly 033b, Immediately Reportable Conditions, AEFI and Case Investigation Forms are in [immunization.md](immunization.md#surveillance) (ADR 0087).

## Surveillance package

A national package, changed only by national surveillance governance (no Override Points). It holds:

- the **Immediately Reportable Condition** list;
- **threshold rules**, each with disease, place level, measure and levels, citing the IDSR section (Section 11 wins where the guidelines disagree):

| Measure | Example | Runs on |
|---|---|---|
| Single case | Cholera, VHF, AFP, neonatal tetanus: one suspected case is the alert | Facility Edge Node, at once (as an Immediately Reportable Condition) |
| Count in a period | Measles: more than 5 cases in a district in a month | Central |
| Rate per 100,000 per week | Meningitis: alert 5, epidemic 15 (10 at high risk); counts below 30,000 population | Central |
| Normal channel | Malaria: above the 75th percentile of the same week over 5 years | Facility Edge Node |

## Computing

- Thresholds run weekly when 033b is computed: facility rules on the edge, district rules at central over every facility's counts.
- **Baselines and populations** are imported at central from DHIS2 (historical weekly 033b, population figures). A normal channel needs at least 3 years; otherwise the facility shows *no baseline*.

## Outbreak Alerts

- Crossing a threshold creates an **Outbreak Alert**: disease, place, epidemiological week, level (alert or action) and the counts behind it.
- Status: open → under investigation → confirmed outbreak or discarded. Discarding needs a reason; a confirmed outbreak stays open until the district closes it.
- It raises a 24-hour Task for the district surveillance focal person and appears in the district alert log.
- **Line list:** Patient Number, age, sex, village, onset date, facility and status of each case behind it, visible only to district and national surveillance roles.
- **Messages:** central sends an SMS to the district surveillance focal person and the district health team: disease, place, week and threshold, no patient detail (an extension of ADR 0073).
- **National:** a national surveillance role sees every alert, with a Notification when an action threshold is crossed or an outbreak is confirmed.
- An Immediately Reportable Condition also appears in the district surveillance focal person's worklist at central.

## Submission

| Route | v1 |
|---|---|
| 033b to mTrac | Keyed by hand from the signed report (default) |
| 033b to DHIS2 | Central's HMIS sender (ADR 0069); off until MoH confirms the data set and permits it |
| Cases to eIDSR | Not in v1; Case Investigation Forms print |
| AEFI and ADR to VigiFlow | Not in v1; Annex 11A prints |

## Not in v1

- eIDSR writes, E2B export for VigiFlow, FHIR case-report bundles.
- Event-based (community or media) surveillance.

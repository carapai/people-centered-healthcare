# Outbreak Alerts are computed from a national Surveillance package, and 033b may go to DHIS2

Decided in [Does the platform raise outbreak threshold alerts and submit surveillance data electronically?](https://github.com/carapai/people-centered-healthcare/issues/799), after the research [What do outbreak threshold alerts and electronic mTrac, eIDSR and VigiFlow submission require in Uganda?](https://github.com/carapai/people-centered-healthcare/issues/798) ([surveillance-alerting.md](https://github.com/carapai/people-centered-healthcare/blob/research/surveillance-alerting/docs/research/surveillance-alerting.md), branch `research/surveillance-alerting`). It extends [ADR 0087](0087-surveillance-reports-are-computed-from-records-and-handed-to-existing-national-channels.md), which deferred threshold alerting and electronic submission, and [ADR 0073](0073-messages-are-sent-by-central-over-sms-through-a-local-aggregator.md), which gave staff only in-app Notifications.

The research found:

- IDSR (3rd edition, 2021) makes one suspected case the alert for cholera, VHF, anthrax, AFP, neonatal tetanus, AEFI and yellow fever. Meningitis uses weekly rates per 100,000 (or case counts below 30,000 population), measles more than 5 cases in a district in a month, and malaria a normal channel: the 75th percentile of the same week over 5 non-epidemic years. Section 3.3 and Section 11 disagree on some values.
- A facility tells the district within 24 hours; the district tells national within 24 hours, investigates within 48 hours and logs every alert.
- mTrac's open code gives the 033b SMS keywords and a DHIS2 data set, neither confirmed current. eIDSR is a DHIS2 tracker program; permission for other systems to write to it, and its identifiers, are unconfirmed. VigiFlow imports E2B files by hand, with no public API.

**The decision:**

- **Surveillance package.** A national **Surveillance package** holds the Immediately Reportable Condition list (ADR 0087) and threshold rules: disease, place level (facility or district), measure (count, rate per 100,000, or normal channel), alert and action levels, and the IDSR section cited. Where the guidelines disagree, Section 11 wins. Only national surveillance governance changes it, by publishing a new version; there are no Override Points.
- **Where rules run.** Weekly, when 033b is computed: facility thresholds on the Facility Edge Node, district thresholds at central over every facility's counts.
- **Baselines.** Central imports historical weekly 033b counts and population figures from DHIS2. A normal channel is computed only from 3 or more years of data; otherwise the facility shows *no baseline*, never an alert.
- **Outbreak Alert.** Crossing a threshold creates an **Outbreak Alert**: disease, place, epidemiological week, level crossed (alert or action) and the counts behind it. Status: open, under investigation, then confirmed outbreak or discarded (with a reason). It raises a 24-hour Task for the district surveillance focal person and appears in the district alert log. A confirmed outbreak stays open until the district closes it.
- **Line list.** The alert shows the cases behind it: Patient Number, age, sex, village, onset date, facility and status, visible only to district and national surveillance roles.
- **Single cases.** An Immediately Reportable Condition also appears in the district surveillance focal person's worklist at central.
- **Who hears.** Central sends an SMS Message to the district surveillance focal person and the district health team for each Outbreak Alert, naming disease, place, week and threshold, with no patient detail. This is the one staff Message kind beside ADR 0073's. A national surveillance role sees every alert, with a Notification when an action threshold is crossed or an outbreak is confirmed.
- **033b to DHIS2.** Central can send the signed 033b to DHIS2 through the HMIS sender (ADR 0069). It is off by default and turned on per deployment once MoH confirms the data set and permits it. Keying into mTrac stays the default; the platform sends no mTrac SMS.
- **Deferred.** Writing cases to eIDSR, E2B export for VigiFlow, and FHIR case-report bundles, until identifiers, permission and NDA's acceptance are confirmed. Case Investigation Forms and Annex 11A stay printed.

## Considered options

- **Alerts only from DHIS2 predictors:** DHIS2 can compute normal channels and notify, but only after 033b reaches it on Monday, with no line list and nothing at the facility.
- **Facility or district Override Points on thresholds:** lets a district quiet noisy alerts, but makes alerts incomparable across districts; IDSR sets national values.
- **Normal channel from fewer than 3 years:** alerts sooner, but a 1- or 2-year baseline makes any normal increase look like an epidemic.
- **Sending 033b by SMS to mTrac:** matches today's route, but the syntax is unverified for the current form and would need a shared short code; DHIS2 is where mTrac's data ends up anyway.
- **In-app Notifications only for staff (ADR 0073 as it was):** district focal persons are often away from a screen and the 24-hour clock is short.

# Research: outbreak threshold alerts and electronic surveillance submission in Uganda

Findings for the research ticket [What do outbreak threshold alerts and electronic mTrac, eIDSR and VigiFlow submission require in Uganda?](https://github.com/carapai/people-centered-healthcare/issues/798). This note reports facts and decides nothing. It covers what [ADR 0087](../adr/0087-surveillance-reports-are-computed-from-records-and-handed-to-existing-national-channels.md) deferred: outbreak threshold alerting and electronic submission to mTrac, eIDSR and VigiFlow. It builds on the note on branch `research/immunization-campaigns` (`docs/research/immunization-campaigns.md`), which already covers the HMIS 033b layout, AEFI (Annex 11A), the AFP, measles and neonatal tetanus case forms, and the eIDSR capabilities in §9.10 of the IDSR guidelines. Those facts are not repeated here.

Research date: **2026-11-19**. Every source was read on that date. Claims marked **unverified** have no primary source yet. "Secondary" means a study, news item or partner page that reports the fact, not the owning body's own document. Source numbers in square brackets refer to §8. Page numbers for [1] are the printed page numbers of the guidelines.

## 1. Summary

| Topic | Key facts | Main sources |
|---|---|---|
| Threshold types | IDSR 3rd ed. uses an **alert threshold** (prompts investigation) and an **action (epidemic) threshold** (prompts a definite response). For epidemic-prone, elimination and eradication targets, **one suspected case** is the alert. For others it is an unexplained increase over weekly or monthly data. | [1] §3.3, pp. 161–162 |
| Count thresholds | **Meningitis:** alert 5/100,000/week (30,000–100,000 people) or 2 cases in a week (<30,000); epidemic 15/100,000/week (10/100,000 at high risk) or 5 cases in a week or doubling over 3 weeks (<30,000). **Measles:** outbreak threshold "more than 5 cases in a district in a month"; confirmed when 3 of 5 tested are IgM positive. **Dysentery:** an increase in cases or deaths over time. **Cholera, VHF, anthrax, AFP, NNT, AEFI, yellow fever:** a single suspected case is the alert; a single confirmed case is the action threshold. | [1] pp. 391–553 |
| Malaria normal channel | Weekly cases for the most recent **5 non-epidemic years**, per epi week. Upper limit = **75th percentile (3rd quartile)**, lower = 25th. District and HSD: computed in a spreadsheet; above the upper line = epidemic alert, investigate in 24–48 hours. HC II and III: sort the 5 values by hand; above the 3rd quartile for **2 consecutive weeks** = report to the DHO within 24–48 hours. | [1] pp. 468–471 |
| Who acts, how fast | Facility: notify district **within 24 hours** of passing a threshold. District: notify national level within 24 hours; field investigation and concrete response **within 48 hours** of notification; record every alert in the outbreak and rumour log and the electronic event management system. Rapid Response Team risk assessment within 72 hours. | [1] pp. 173, 186, 303, 325, 330, 337, 344 |
| mTrac 033b syntax | Open-source mTrac code (RapidSMS, then RapidPro, 2011–2021): keywords `cases`, `death`, `epc`, `epd`, `tra`, `arv`, `apt`, `mat`, `qun`; values as two-letter disease code then count, separated by `.` (for example `cases ma.12.dy.0`). Pushed to DHIS2 as an XML `dataValueSet` per facility and ISO week. Whether this is the syntax in use in 2026 is **unverified**. | [3], [4] |
| 033b into DHIS2 | mTrac data are "automatically integrated into DHIS2". MoH's digital health plan lists 033B among forms digitized in eHMIS (DHIS2). An external system writing 033b to DHIS2 directly, and who authorizes it, are **unverified**. | [5], [6] |
| eIDSR | MoH-owned, on the national DHIS2, upgraded to **DHIS2 2.35** (2020–2022), in 100 of 146 districts by September 2022. Case-based through an **"eCase Notification and Investigation" tracker program**; electronic case forms for 9 of 33 priority conditions. DSFPs are the main users, but 2,409 facility health workers in 536 facilities were trained to register cases. VPD (AFP, measles-rubella, NNT, AEFI) case-based surveillance moved from EpiInfo to eIDSR from September 2023. Program and stage UIDs are **unverified**. | [1] §9.10, [7], [8], [9] |
| External push to eIDSR | No MoH guidance found allowing or forbidding systems to push case data into eIDSR. Precedent: UgandaEMR sends case-based **FHIR R4** bundles for TB (eCBSS, to a HISP Uganda FHIR server), mortality surveillance and mpox screening (to a METS central server). | [10] |
| Interoperability policy | eHealth Policy (2016): MoH with NITA-U develops an eHealth Enterprise Architecture and Interoperability Framework and enforces compliance. Digital Health Strategic Plan 2020/21–2024/25: FHIR and HL7 "not yet in use"; plans a health information exchange, registries and UgHub connection. No published interoperability standard or access procedure found. | [6], [11] |
| VigiFlow | VigiFlow imports **E2B(R2) and E2B(R3)** XML through its "Import" screen; R3 only as "Validated" import; at most 100 ICSRs per file; sub-organisations can import; import "might need to be enabled by UMC". A separate **eReporting for Industry** extension accepts E2B uploads from marketing authorisation holders. No public VigiFlow API was found. Whether NDA accepts E2B from facility systems, and how accounts are granted, is **unverified**. | [12], [13], [14] |
| Other systems | **DHIS2:** predictors (with annual and sequential samples, `percentileCont`, `stddev`) plus validation rules and validation notifications can compute a normal channel and alert user groups. **mTrac:** sends an SMS alert to every district health team member when a preset threshold is hit. **UgandaEMR:** sends HMIS aggregates to DHIS2; no 033b or threshold feature found in its documentation. **WHO EWARS:** per-disease thresholds (one case for measles, moving average for malaria), SMS and email alerts, offline mobile app. **Africa CDC:** event-based surveillance framework (2018, 2nd ed. 2023) and a DHIS2-based event management system (pilot June 2020). Uganda piloted the **7-1-7** target in 2021. | [5], [15], [16], [17], [18], [19] |

## 2. Alert and action thresholds (IDSR 3rd edition)

### 2.1 Definitions

From §3.3 [1, pp. 161–162]:

- Thresholds "indicate unusual situation and require that something should happen or change".
- **Alert threshold:** "suggests to health staff and the surveillance team that further investigation is needed". It is reached at "one suspected case (as for an epidemic-prone disease or for a disease targeted for elimination or eradication) or when there is an unexplained increase for any disease or unusual pattern seen over a period of time in weekly or monthly summary reporting".
- **Action (epidemic) threshold:** "triggers a definite response", for example communicating laboratory confirmation, an immunization activity or infection control. For rare or eradication-targeted diseases one case suggests an epidemic.
- "Trained health workers should always determine the alert and epidemic thresholds"; the disease-specific values are in Section 11 [1, p. 105]. Thresholds "are assessed and reset at national or international levels according to the observed trends" [1, p. 162].
- §3.3 also cites WHO's 2014 meningitis-belt values (alert 3 per 30,000–100,000 per week) and says "the epidemic threshold for malaria in some countries is 3rd Quartile ... for the past 5 years; Alert threshold is 2nd quartile/Median" [1, p. 162]. These differ from the Section 11 values below; the guidelines do not reconcile them.

### 2.2 Disease-specific thresholds (Section 11)

| Disease | Alert threshold | Action threshold | Page |
|---|---|---|---|
| Acute haemorrhagic fever (Ebola, Marburg, CCHF, Lassa, RVF) | A single suspected case: report case-based information immediately, isolate, collect specimen | A single confirmed case | 391–393 |
| AEFI | A single suspected case: treat, complete case investigation form | A single confirmed case: monitor for a cluster, report immediately | 397–398 |
| Anthrax (human) | A single suspected case: report immediately to public health and animal health sectors | A single confirmed case | 398–400 |
| Cholera | A single suspected case: report immediately; stool from 5 patients within 5 days of onset | A suspected case confirmed | 410–412 |
| Diarrhoea with blood (Shigella) | "the number of cases or deaths is increasing over a period of time" | A suspected outbreak confirmed | 430–431 |
| Malaria | "Unusual increase in the number of new malaria cases or deaths as compared to the same period in previous non-epidemic years" | New cases exceed "the upper limit of cases seen in a previous non-epidemic period" (normal channel, §2.3) | 468–471 |
| Measles | A suspected outbreak; outbreak threshold is "more than 5 cases in a district in a month"; blood from the first 5 cases | Outbreak confirmed ("as few as 3 out of 5 suspected measles cases are laboratory confirmed") | 480–483 |
| Meningococcal meningitis | 30,000–100,000 inhabitants: 5 cases per 100,000 per week. Under 30,000: 2 cases in 1 week, or an increase over the same time in previous non-epidemic years | 30,000–100,000: 15 per 100,000 per week (10 per 100,000 when risk is high: no epidemic in 3 years, alert in dry season). Under 30,000: 5 cases in 1 week or doubling over 3 weeks | 484–485 |
| Neonatal tetanus | A single suspected case | A case confirmed by investigation | 490–491 |
| Poliomyelitis (AFP) | A single suspected case: two stool specimens 24–48 hours apart | A confirmed case (national level decides the response) | 506–508 |
| Yellow fever | A single case or cluster suspected or probable | Confirmed case | 551–553 |

Thresholds for crisis-affected populations [1, §10.5.4, p. 384]: crude mortality under 1 per 10,000 per day, under-5 mortality under 2 per 10,000 per day, and an alert on "doubling of weekly incidence compared to the weekly average of previous 2-3 weeks".

### 2.3 How the malaria normal channel is computed

From the malaria summary [1, pp. 469–471]:

- **Baseline:** weekly malaria cases for "the most recent non epidemic 5 years", for each of the 52 weeks.
- **Method A, national, district and HSD ("computerised and statistical"):** in a spreadsheet, plot the 75th percentile of each week across the 5 years (`=PERCENTILE(...,0.75)`, the "upper normal") and the 25th percentile (the "lower normal"). The band between is the normal channel. "If the observed weekly malaria cases are above the upper plot of the Normal channel an epidemic alert exists and should be investigated within 24-48 hours."
- **Method B, HC II and III ("simple and non statistical"):** sort each week's 5 values; the middle is the median, "the fourth highest number" the 3rd quartile (upper limit), "the second highest" the 1st quartile. If current cases are "above the line of the 3rd quartile [upper limit] for 2 consecutive weeks", report "within 24-48 hours to the District Health Office".
- The worked spreadsheet formula samples `A1:A4` (four cells) although the text says five years; the guidelines do not explain the difference.
- The guidelines also mention forecasting from rainfall, temperature and humidity [1, p. 469].
- **Counted unit:** the text says "malaria cases" for method A and "observed weekly malaria cases at the health facility" for method B; it does not say whether confirmed only or confirmed plus clinical. §3.3 says "confirmed malaria cases" [1, p. 162].

### 2.4 Who acts, and how fast

| Level | Duty | Time | Source |
|---|---|---|---|
| Health facility | Notify suspected outbreaks of epidemic-prone disease to the district | Within 24 hours of passing the threshold (indicator target 80%) | [1, p. 325] |
| Health facility | Keep current trend analyses (line graph, histogram or spot map) for priority conditions | Weekly | [1, pp. 325, 330] |
| Health facility (eIDSR) | Report events requiring immediate action; submit weekly reports; "act on notifications" | — | [1, Table 14, p. 372] |
| District | Notify suspected outbreaks to the next level | Within 24 hours of passing the epidemic threshold | [1, p. 330] |
| District | Investigate when "alert or epidemic thresholds have been reached"; DHO constitutes a District Rapid Response Team and sends a preliminary report to MoH | "immediately" | [1, p. 173] |
| District | Field investigation after notification; concrete response | Within 48 hours each | [1, p. 303] |
| District | Record every alert in the outbreak and rumour log (Annex 4A) and enter it "into electronic event management systems" | — | [1, p. 173] |
| District (eIDSR) | Verify and approve onward transmission; issue alerts to other facilities and leaders; feed back to the reporting facility | — | [1, Table 14, p. 372] |
| Region | Notify national level | Within 24 hours of passing the epidemic threshold | [1, p. 337] |
| National (PHEOC) | Receive suspected outbreak notifications | Within 24 hours of passing the alert threshold | [1, p. 344] |
| Rapid Response Team | Initial risk assessment report | Within 72 hours | [1, p. 186] |
| Member State | Notify next higher level | Within 48 hours of passing the epidemic threshold (Annex 8I) | [1, p. 359] |
| Crisis settings | Alert verification by district surveillance focal points | "immediately" | [1, p. 384] |

Uganda also uses the **7-1-7** target (7 days to detect, 1 day to notify, 7 days to complete early response), piloted from October 2021. Of 42 events reviewed, 74% met the 1-day notification target and 24% met all three (secondary [19]).

## 3. mTrac and electronic 033b

### 3.1 What mTrac is

- mTrac is "part of the Ministry of Health's national eHMIS system". Facilities submit the weekly HMIS 033B form by SMS; the data are "automatically integrated into DHIS2". When a preset threshold is hit, "an SMS alert is sent to every member of the district health management team for immediate response" [5].
- MoH's *Digital Health Strategic Plan 2020/21–2024/25* lists mTrac and eIDSR as surveillance systems for HMIS 033b, and lists 033B among paper tools "currently digitized" in eHMIS (DHIS2) [6, Table 3].

### 3.2 SMS syntax (from the open-source code)

UNICEF Uganda publishes the mTrac code on GitHub under a BSD licence. Repositories were last pushed between 2017 and 2021 [3], [4].

- **Keywords:** `cases`, `death`, `tra` (tracer medicines), `arv` (HIV test kits and eMTCT drugs), `epd` and `epc` (other conditions: deaths and cases), `mat`, `apt` (OPD and eMTCT summary), `qun` [4, `mtrack/utils.py`]. RapidPro flow exports in `mtrackpro` show the triggers `death`, `epc`, `epd`, `apt`, `arv`, `tra` [3].
- **Format:** the first word is the keyword; the rest are two-letter codes each followed by a count, with `.` as the delimiter. The parser lower-cases the message and fills missing codes with 0 [3, `webhooks/mtracpro_webooks.py`]. Illustration built from the parser: `cases ma.12.dy.0.me.1`.
- **Codes in `cases`** (16): `ma` malaria, `dy` dysentery, `sa` SARI, `af` AFP, `ae` AEFI, `ab` animal bites, `mg` bacterial meningitis, `ch` cholera, `gw` guinea worm, `me` measles, `nt` neonatal tetanus, `vf` other VHF, `pl` plague, `tf` typhoid, `yf` yellow fever, `tb` presumptive MDR TB. **`death`** has 18: the same plus `md` maternal and `pd` perinatal [3].
- **Other conditions** (`epc`, `epd`) take up to three name and count pairs; the names are the IDSR two-letter codes (for example `AX` anthrax, `DP` diphtheria, `WC` pertussis, `AR` adverse drug reactions) [3], [1, p. 125].
- **Reporting week:** a report received in a week is assigned to the previous ISO week (`2016W01` format) [3].
- **Drift:** the 2021 033b has 21 named rows, including hepatitis B (HB), rifampicin-resistant TB (DR), stillbirths and early neonatal deaths, and tested and positive columns (research note on branch `research/immunization-campaigns`, §3). The 2016 code has none of these. The syntax for those rows in 2026 is **unverified**.
- **Short code:** the IDSR guidelines name 6767 for eIDSR SMS [1, p. 377]. The mTrac short code is not in the code read here (**unverified**).

### 3.3 Route into DHIS2

- The mTrac webhook builds an XML `dataValueSet` for data set `V1kJRs8CtW4` with `orgUnitIdScheme="uuid"`, a weekly period and a `completeDate`, mapping each code to a DHIS2 data element and category option combo, and queues it in a PostgreSQL table for sending to the `dhis2` server [3, `webhooks/settings.py`]. A separate `rapidsms_dhis2` repository is described as the "DHIS2-mTRACK API" [4].
- Whether `V1kJRs8CtW4` is still the eHMIS 033b data set is **unverified**; the instance metadata was not accessible.
- No source found describes another system pushing 033b straight to eHMIS. UgandaEMR's documented DHIS2 push covers "HMIS Reports and PEPFAR MER Reports" with a facility eHMIS account, not 033b specifically [15].

### 3.4 Who grants access

- No published MoH procedure for an mTrac or eHMIS integration account was found. The eHealth Policy assigns compliance with the interoperability framework to MoH [11, §7.2.4]; the IDSR guidelines give the national level "system administration (registration of health staff using server)" for eIDSR [1, p. 372].
- The roles of the Division of Health Information and the Resource Centre in approving an integration are **unverified**. UgandaEMR's DHIS2 push uses a username and password for an eHMIS account issued per facility setup [15].

## 4. eIDSR

### 4.1 What it is today

- MoH-owned system on the national DHIS2, approved by the eHealth TWG in August 2017 [1, p. 373]. Upgraded with IDI (Makerere) and HISP Uganda to **DHIS2 2.35**, "faster reading and writing tracker data, handling over 100 concurrent users and enhanced case-based surveillance features on Android and web" [7].
- **Rollout:** 100 of 146 districts and cities (68%) between October 2020 and September 2022 [7].
- **Coverage:** "electronic case investigation forms for 9 out of 33 priority diseases and conditions" [7].
- **Program:** case records sit in the **eCase Notification and Investigation** tracker program, with modules for notification and investigation, laboratory request and results, admission and case management (secondary [8]). The measles-rubella, AFP, NNT and AEFI eCase forms were revised to match the standard case forms; VPD case-based reporting moved from EpiInfo to eIDSR from September 2023, with AFP history from 2016 and measles from 2006 migrated (secondary [9]).
- **Identifiers:** program, stage and data element UIDs are not published (**unverified**).
- **Thresholds in eIDSR:** "Weekly trend curves are also pre-programed on dashboards to help in detection of epidemic alert and outbreak thresholds", using 033b data extracted from mTrac [1, p. 375]. Whether eIDSR sends automatic alerts on threshold crossing (rather than on case registration) is **unverified**.

### 4.2 Who enters cases

- The guidelines say DSFPs are "the main users" at district level: they receive messages and case forms from facilities and enter them [1, p. 375].
- Rollout trained 2,409 of 3,295 targeted **facility-level** health workers in 536 facilities, and the curriculum covered "alert notification, case notification, registration, and reporting using SMS, Android, and web platforms" [7]. Table 14 lets facilities "report events requiring immediate action" "depending on the eIDSR platform" [1, p. 372].
- So both DSFPs and trained facility staff register cases; how many facilities do so in 2026 is **unverified**.

### 4.3 External systems pushing case data

- The DHIS2 Tracker API accepts tracked entities, enrollments and events from any client with an authorized account, and DHIS2 has a FHIR adapter project [20]. Whether MoH permits a non-MoH system to write to eIDSR is **unverified**; no MoH document on this was found.
- The IDSR guidelines give the national level "ensuring linkage with other platforms, to facilitate interoperability and avoid duplication" [1, p. 372].
- **Precedent, UgandaEMR** (METS, last pushed July 2026) [10]:
  - Sync profiles send FHIR R4 `collection` bundles of `Patient`, `Person`, `EpisodeOfCare`, `Encounter` and `Observation`.
  - **eCBSS Integration** (TB case-based surveillance) posts to `https://services.fhir.hispuganda.org/fhir`.
  - **Mortality Surveillance** and **MPOX Screening** post to `https://ugisl.mets.or.ug/central-server`.
  - All three ship with `profileEnabled: false` and are switched on per site.
- The eIDSR paper notes "Uganda does not have in place interoperable and interconnected electronic reporting for animal health" [7].

## 5. VigiFlow and NDA

### 5.1 What VigiFlow accepts (UMC documentation)

- **Import:** E2B XML files are imported in VigiFlow's "Import" section. "Import functionality might need to be enabled by UMC." Each file "should contain no more than 100 ICSRs" [12].
- **Versions:** new VigiFlow imports **E2B(R2) and E2B(R3)**; R2 is converted to R3 on save. For R3 "the only option allowed is 'Validated' import" [13].
- **Validated import requires** report type, reporter identifier and qualification, patient identifier, at least one MedDRA-coded reaction and at least one suspect drug [12].
- **Who imports:** "sub-organisations are able to import E2B-xml files", not only the national centre [13].
- **Follow-ups:** an imported ICSR is a follow-up when sender organisation, sender's safety report ID and worldwide unique ID match an existing report; follow-ups are merged automatically [13].
- **Acknowledgements:** E2B acknowledgement messages can be downloaded per import [12].
- **eReporting for Industry:** an extension of VigiFlow where marketing authorisation holders enter reports or upload E2B XML ("intended for MAHs with databases capable of transmitting ICH E2B XML"), with MAH user accounts added through UMC's VigiUserAdmin [14]. Which body creates those accounts in Uganda is **unverified**.
- **Other:** national users can send copies to **AfriVigilance**, activated on request for some African organisations; **VigiMobile** is an offline-capable phone add-on for AEFI capture [12], [2].
- No public VigiFlow API or system-to-system gateway is documented on UMC's support site; every import path found is a logged-in user uploading a file (**no API found**; UMC was not asked).

### 5.2 NDA

- NDA's National Pharmacovigilance Centre manages reports in VigiFlow; channels include paper forms, an electronic form, a hotline, USSD, the Med Safety app and WhatsApp (branch `research/immunization-campaigns`, §4.3).
- NDA guidance on accepting E2B from facility systems, on using eReporting for Industry for anything other than MAHs, or on issuing VigiFlow sub-organisation accounts to facilities or districts was not found (**unverified**).
- Whether eIDSR AEFI cases reach VigiFlow is **unverified**.

## 6. Other systems

### 6.1 DHIS2 (national eHMIS and eIDSR platform)

From the DHIS2 maintenance documentation [16]:

- **Predictors** compute expected values from past data: *Sequential sample count* (immediately preceding periods), *Annual sample count* (same time of year in previous years) and *Sequential skip count* ("to skip one or more immediately preceding samples that might in fact contain values from the beginning of an outbreak").
- Generator functions include `avg`, `median`, `percentileCont(p, x)`, `stddevSamp`, `stddevPop`, `normDistCum`. The documentation's own outbreak example is `avg(...) + 2 * stddev(...)`.
- A predictor can run at several org unit levels, so a facility spike and a district-wide rise are caught separately.
- **Validation rules** compare actual values with predicted ones; **validation notifications** send the result to user groups through DHIS2 messaging, individually or as summaries ("individual messages for high-priority disease outbreaks").
- An IDSR normal channel (75th percentile of the same week over 5 years) fits `percentileCont(0.75, ...)` with an annual sample count of 5; whether Uganda's eHMIS runs such predictors is **unverified**.

### 6.2 UgandaEMR

- Sends "auto generated" HMIS and PEPFAR MER reports to eHMIS DHIS2 through an eHMIS account and the facility's DHIS2 UUID, after the user validates and submits [15].
- Case-based FHIR pushes for TB, mortality and mpox (§4.3).
- No 033b report, eIDSR link or threshold alerting appears in its user documentation index [15].

### 6.3 WHO EWARS

- WHO's Early Warning, Alert and Response System ("EWARS in a box") for emergencies: per-disease thresholds set by the response team; "with highly-infectious diseases like measles, one case triggers an alert. Other diseases like malaria have a moving average"; SMS and email alerts on trigger; Android phones and a laptop DataHub; an offline stand-alone mode was a lesson from the Rohingya response (secondary [17]).

### 6.4 Africa CDC

- Event-based surveillance framework (2018; 2nd edition 2023 adds One Health, cross-border EBS and event management systems).
- Africa CDC and HISP South Africa built a DHIS2 Tracker-based **event management system**, piloted with signal and event entry from June 2020 (secondary [18]). Whether Uganda's PHEOC uses it is **unverified**.

## 7. Trade-offs and open points

### 7.1 Trade-offs visible in the facts

- **Computing thresholds at the facility vs at central.** Facility-level malaria detection needs 5 years of weekly counts for that facility; a new edge has no such history unless back-loaded from eHMIS. District and national thresholds need the sum across facilities, which only central has. Meningitis thresholds are per population (inhabitants), which needs the yearly Population package.
- **Count basis.** The IDSR thresholds count suspected cases (measles, meningitis) or confirmed cases (malaria §3.3), weekly or monthly; a platform computing them uses the same diagnosis coding as 033b.
- **Single-case alerts** for cholera, VHF, anthrax, AFP, NNT, AEFI and yellow fever coincide with the immediately reportable list that ADR 0087 already turns into a 24-hour Task. Count thresholds (malaria, meningitis, measles, dysentery) are the new part.
- **Two alerting systems already exist** (mTrac threshold SMS to district teams; eIDSR dashboards). A third would alert on the same data unless the scope differs, for example facility staff before the weekly report is sent.
- **mTrac by SMS vs direct DHIS2.** The SMS syntax is simple, but its current form is unverified, and an SMS gateway from an edge needs a modem or aggregator; a DHIS2 `dataValueSet` matches what ADR 0069 already does for 105, 108 and 106a, but needs MoH to allow 033b to arrive outside mTrac.
- **eIDSR via Tracker API vs FHIR.** eIDSR is DHIS2 Tracker; UgandaEMR's precedent sends FHIR R4 to intermediary servers (HISP Uganda FHIR, METS central), not to eIDSR directly. The platform stores FHIR R5.
- **VigiFlow.** The documented route is a person uploading E2B XML (R3, validated) as a sub-organisation user; that would be a generated file, not an automatic submission.

### 7.2 Not verified

- Current mTrac SMS syntax for the 2021 033b rows, the mTrac short code, and whether `V1kJRs8CtW4` is the current 033b data set.
- Whether MoH allows 033b to be written to eHMIS by a system other than mTrac, and who signs off (Division of Health Information, Resource Centre).
- eIDSR program, stage and data element UIDs; whether it alerts automatically on thresholds; whether external writes are permitted.
- NDA's acceptance of E2B from facility systems, sub-organisation accounts, and any eIDSR-to-VigiFlow link.
- Whether Uganda's eHMIS runs outbreak predictors, and whether the PHEOC uses the Africa CDC event management system.

## 8. Sources

1. Uganda MoH, *National Technical Guidelines for Integrated Disease Surveillance and Response*, 3rd edition, September 2021: §3.3 (pp. 161–162), Table 2 (p. 105), Annex 2H mTrac codes (p. 125), §4.0 (p. 173), RRT risk assessment (p. 186), outbreak timeliness indicators (p. 303), Annexes 8A–8D and 8I (pp. 325, 330, 337, 344, 359), Table 14 (p. 372), §9.10 eIDSR (pp. 373–377), §10.5.4 (p. 384), Section 11 disease summaries (pp. 391–553). <https://www.afro.who.int/sites/default/files/2021-09/2_Uganda%203rd%20IDSR%20Tech%20Guideline_PrintVersion_10Sep2021.pdf>
2. Research note *immunization-campaigns.md*, branch `research/immunization-campaigns` (033b layout, AEFI, case forms, NDA channels). <https://github.com/carapai/people-centered-healthcare/blob/research/immunization-campaigns/docs/research/immunization-campaigns.md>
3. UNICEF Uganda, `mtrackpro`: mTrac RapidPro flows and webhooks (last commit January 2016). <https://github.com/unicefuganda/mtrackpro>
4. UNICEF Uganda, `rapidsms-mtrack` (keywords in `mtrack/utils.py`, last push 2019), `mtrack` (last push 2021) and `rapidsms_dhis2` ("DHIS2-mTRACK API", 2018). <https://github.com/unicefuganda/rapidsms-mtrack> ; <https://github.com/unicefuganda/mtrack> ; <https://github.com/unicefuganda/rapidsms_dhis2>
5. UNICEF Uganda, "mTrac". <https://www.unicef.org/uganda/what-we-do/mtrac>
6. Uganda MoH, *The Uganda Health Information and Digital Health Strategic Plan 2020/21–2024/25*, January 2021, §2.7 and Table 3, Strategy 3.3. <https://extranet.who.int/cpcd/sites/default/files/public_file_repository/uga_uganda_health-information-digital-health-strategic-plan_2020-2025.pdf>
7. Mugasha R et al., "Scaling Up and Enhancing the Functionality of the Electronic Integrated Diseases Surveillance and Response System in Uganda, 2020-2022", *JMIR Public Health and Surveillance* 2025, doi:10.2196/59783 (PMC12011313). <https://publichealth.jmir.org/2025/1/e59783>
8. DHIS2, "Uganda Ebola response" (eCase Notification and Investigation tracker program) (secondary). <https://dhis2.org/uganda-ebola-response/>
9. DHIS2 Community, "Enhancing disease surveillance: a shift from EpiInfo to eIDSR for VPD case-based surveillance reporting in Uganda using DHIS2 platform", 2024 DHIS2 Annual Conference abstract, 10 April 2024 (secondary). <https://community.dhis2.org/t/enhancing-disease-surveillance-a-shift-from-epiinfo-to-eidsr-for-vpd-case-based-surveillance-reporting-in-uganda-using-dhis2-platform/58093>
10. METS Programme, `openmrs-module-ugandaemr-sync`, sync profiles `profile-ecbss-integration.json`, `profile-mortality-surveillance.json`, `profile-mpox-screening.json` under `api/src/main/resources/configuration/hie/syncprofile/`. <https://github.com/METS-Programme/openmrs-module-ugandaemr-sync>
11. Uganda MoH, *Uganda National eHealth Policy*, November 2016, §7.2. <https://library.health.go.ug/file-download/download/public/1031>
12. UMC, VigiFlow support: "Import data", "Importing E2B XML files", "The differences between validated and non-validated import", "Exporting reports to AfriVigilance". <https://support.who-umc.org/vigiflow/import-data> ; <https://support.who-umc.org/vigiflow/import-data/importing-e2b-xml-files> ; <https://support.who-umc.org/vigiflow/import-data/the-differences-between-validated-and-non-validated-import> ; <https://support.who-umc.org/vigiflow/export-data/exporting-reports-to-afrivigilance>
13. UMC, *Import ICSRs: key differences in old versus new VigiFlow*, December 2019. <https://www.who-umc.org/media/165673/42-import-icsrs-old-versus-new-vigiflow.pdf>
14. UMC, VigiFlow eReporting for Industry support: "Upload E2B", "Frequently asked questions"; documentation index. <https://support.who-umc.org/vigiflow-ereporting-for-industry/upload-e2b> ; <https://support.who-umc.org/vigiflow-ereporting-for-industry/frequently-asked-questions-about-vigiflow-ereporting-for-industry> ; <https://support.who-umc.org/llms.txt>
15. METS Programme, UgandaEMR documentation: "Send Aggregate Reports" and documentation index. <https://mets-programme.gitbook.io/ugandaemr-documentation/data-exchange/sending_aggregate_reports_to_dhis2> ; <https://mets-programme.gitbook.io/ugandaemr-documentation/llms.txt>
16. DHIS2 documentation, *Configure metadata*: predictors (sample counts, skip count, functions) and validation notifications. <https://github.com/dhis2/dhis2-docs/blob/master/src/user/configure-metadata.md>
17. WHO, "A little black box for detecting and tracking outbreaks"; CDC *Emerging Infectious Diseases* 24(11), 2018, EWARS in the Rohingya response (PMC6199978) (secondary). <https://www.who.int/news-room/feature-stories/detail/a-little-black-box-for-detecting-and-tracking-outbreaks> ; <https://pmc.ncbi.nlm.nih.gov/articles/PMC6199978>
18. Africa CDC event management system on DHIS2, *PLOS Digital Health* 2024 (via summary); DHIS2 Community, "Using DHIS in an Emergency Operations Centre in Event based surveillance" (secondary). <https://community.dhis2.org/t/using-dhis-in-an-emergency-operations-centre-in-event-based-surveillance/43107> ; Resolve to Save Lives, "How event-based surveillance is saving lives across Africa". <https://resolvetosavelives.org/strategies-in-action/how-event-based-surveillance-is-saving-lives-across-africa/>
19. "Implementing the 7-1-7 target to improve epidemic preparedness and response in Uganda", *BMJ Global Health* 10(7):e018207, July 2025 (via summaries); 7-1-7 Alliance resource page (secondary). <https://717alliance.org/resources/implementing-the-7-1-7-target-to-improve-epidemic-preparedness-and-response-in-uganda/>
20. DHIS2 documentation, *Tracker* Web API; `dhis2-fhir-adapter`. <https://docs.dhis2.org/en/develop/using-the-api/dhis-core-version-master/tracker.html> ; <https://github.com/dhis2/dhis2-fhir-adapter>

# Research: immunization campaigns, weekly surveillance and AEFI in Uganda

Findings for the research ticket [What do immunization campaigns, weekly surveillance and AEFI reporting require in Uganda?](https://github.com/carapai/people-centered-healthcare/issues/740). This note reports facts and decides nothing. It extends the routine immunization note on branch `research/immunization` (`docs/research/immunization.md`) and covers what [ADR 0076](../adr/0076-immunization-is-a-core-record-with-a-hand-authored-schedule-and-computed-due-doses.md) and [immunization.md](../architecture/immunization.md) defer: campaigns, weekly mTrac (HMIS 033b) reports, and AEFI and case-based surveillance to eIDSR.

Research date: **2026-11-19**. Every source was read on that date unless a date is given. Claims marked **unverified** have no primary source yet. "Secondary" means a news item, study, partner page or third-party report that states the fact, not the Ministry of Health (MoH), UNEPI or National Drug Authority (NDA) document itself. Source numbers in square brackets refer to §7.

## 1. Summary

| Topic | Key facts | Main sources |
|---|---|---|
| ICHD | MoH *Integrated Child Health Days Implementation Guide* (June 2015): twice a year in **April and October**, static and outreach. District and facility **micro-plans** for each round. Tools: tally sheets, child registers, Child Health Cards, Mother Child Passport. Vitamin A and deworming are tallied on **Form 076** and written on the Child Health Card. Outreach data go on **HMIS 075 and 076**, then into **HMIS 105**: to the HSD by the 7th, the district by the 14th, the centre by the 28th of May or November, through DHIS2. Rounds also run birth registration (Mobile VRS) and hand out short birth certificates. | [1] |
| Mass campaigns | October 2019 MR and OPV campaign: MR for 9 months to under 15 years (18.1 M), OPV for under 5 (8.2 M); schools days 1–3, community days 4–5. Children were finger-marked and given an immunization card. Real-time monitoring used **ODK forms feeding a DHIS2 dashboard** (coverage, doses issued, used and discarded, wastage by reason, staff). nOPV2 rounds in 2022 (9.77 M doses in January). MR follow-up campaign **1–5 October 2026**, 9–59 months regardless of status: 8,285,835 vaccinated against a target of 7,616,746. Mpox (MVA-BN, 2025): targeted phases for key populations. Sudan virus disease: ring-vaccination **trial** (TOKOMEZA SVD, 2025). TCV: no campaign found. | [5], [6], [7], [8], [9], [10], [11], [12] |
| Campaign data systems | **EpiVac** is the national COVID-19 vaccination system with downloadable certificates (ID number plus last six phone digits). It "has also been adapted" to monitor yellow fever, polio and MR campaigns but "is not integrated with DHIS2". DHIS2 aggregate counts add campaign DPT doses into DPT1/DPT3. No national DHIS2 campaign data set was found (**unverified**). | [4], [13] |
| Campaign doses on the card | UCG and MoH: campaign doses **do not replace** routine doses. In 2019 children got a finger mark and an immunization card. Whether campaign doses are written on the Child Health Card or in the child register is **unverified**. | [5], [6], [14] |
| HMIS 033b | *IDSR Technical Guidelines, 3rd ed.* (September 2021) Annex 2H: weekly, every Monday, triplicate, health unit in-charge, zero reporting. Named rows include **AFP (AF), AEFI (AE), measles (ME), neonatal tetanus (NT), yellow fever (YF)**, hepatitis B, bacterial meningitis; "other conditions" codes include **diphtheria (DP)** and **pertussis (WC)**. Columns: cases, deaths, tested, positive. Sent by **mTrac SMS** (6767) and "automatically integrated into DHIS2". | [2], [15] |
| AEFI | IDSR 3rd ed.: AEFI is **immediately reportable**, case-based to the next level **within 24 hours**, using **Annex 11A** (WHO-style AEFI form: patient, reporter, vaccines with batch and expiry, event checkboxes, outcome, "AEFI worldwide unique ID"). NDA's National Pharmacovigilance Centre receives reports in **VigiFlow**; channels include paper forms, an electronic form, hotline, USSD, **Med Safety app** and WhatsApp. A National AEFI Committee classifies serious AEFI. A national investigation deadline (for example 48 hours) is **unverified**. | [2], [16], [17], [18], [19] |
| Case-based VPD | IDSR 3rd ed.: AFP (Annex 11B), measles (Annex 11F) and neonatal tetanus (Annex 11G) case investigation forms. AFP: two stools 24–48 hours apart within 14 days of onset; target 4 AFP per 100,000 under 15. Measles: serum IgM within 30 days of rash; every suspected case where elimination is the target. NNT: no lab confirmation. Specimens go to the **UVRI EPI Laboratory, Entebbe**. | [2] |
| eIDSR | MoH-owned DHIS2 system approved by the eHealth TWG in **August 2017**. District Surveillance Focal Persons enter case forms; SMS (6767), Android and web. Extracts weekly 033b aggregates from mTrac; specimen tracking, outbreak IDs, contact tracing. | [2] |

## 2. Campaigns

### 2.1 Integrated Child Health Days (ICHD)

From the MoH *Integrated Child Health Days Implementation Guide*, June 2015 [1]:

- **Schedule:** "bi-annually in the months of April and October at all health facilities and designated outreaches." Districts may add days (for example Fridays to Sundays at places of worship).
- **Package:** scheduled antigens, vitamin A, deworming, MUAC and growth monitoring, blood pressure for adults over 45, birth registration, ITN and hygiene messages, EID sample collection.
- **Responsibility:** the MoH nutrition unit (Department of Community Health) oversees ICHD. National Medical Stores and UNEPI "quantify, procure and distribute" supplies.
- **District micro-plans** for each round, with: a RED coverage analysis and catchment mapping against target populations; communication plans; site locations, staff and contacts; supplies and logistics by sub-county with a budget. They "shall be a reflection of health facility micro plans".
- **Facility micro-plans:** outreach site names, dates, responsible officers, VHTs and resources. Schedules are displayed at the facility.
- **Data tools on site:** "Tally sheets, child registers, Child Health Cards, Mother Child passport and other appropriate register".
- **Recording vitamin A and deworming:** "Tally the child's dose on the Vitamin A part of the tally sheet / Form 076" and "Record the child's dose in the Child Health Card". The guide gives no separate rule for vaccine doses given during ICHD; immunization follows the routine schedule ("the child should receive the scheduled antigens").
- **Reporting:** "All data from designated outreaches will be filled in the summary report forms (HMIS 075 and 076) and reported in the Health Facility Monthly Forms (HMIS 105)". Deadlines: health sub-district by the **7th**, district by the **14th**, centre by the **28th** of May or November. The DHO completes reporting through DHIS2.
- **EPI indicators:** number and percentage of targeted children under 1 receiving DPT3, measles and PCV, and HPV (two doses at the time).
- **Birth registration:** a sub-county delegate joins outreach teams; birth records go into Mobile VRS at the District Planning Unit; short birth certificates are printed by LC1 and handed out at the next ICHD round.

Form numbers: the Gavi Zero-Dose Learning Hub report (October 2024) calls the child register **HMIS Form 073** and the child tally sheet **HMIS Form 076** [13]. This agrees with the ICHD guide's Form 076 and settles part of the numbering question left open in the routine note.

A 2024 evaluation of ICHD as a catch-up strategy in three districts exists (MDPI *Vaccines* 12(12):1353) but could not be read (HTTP 403).

### 2.2 Mass campaigns

| Campaign | Facts | Source |
|---|---|---|
| **MR and OPV, October 2019** | Five days (16–20 October 2019). MR for 9 months to under 15 years (target 18.1 M); OPV for under 5 (8.2 M). Days 1–3 in schools, days 4–5 in communities. Reached about 19.5 M children: 18.1 M MR (100%), 7.96 M OPV (97%). 16,000 schools, 20,000 posts, 134,000 community health workers. The Minister: the campaign "does NOT replace the routine immunization schedule". | [5], [6] (UNICEF report, WHO press release) |
| Proof of vaccination, 2019 | Per the MoH, children "should have a mark on their fingers" and receive "an immunisation card indicating that they were vaccinated". | [14] (secondary) |
| **nOPV2, 2022** | After two cVDPV2 detections in November 2021: round 1 door-to-door, 14–21 January 2022, 9,768,697 doses to under 5 in 136 districts. Round 2 planned 4–10 November 2022 for 8.7 M children. Vaccination status for safety follow-up came "by finger marking or verbal recall". | [7] (study, secondary) |
| **Yellow fever preventive campaign, 2023–2024** | Phase I from June 2023 in 51 districts; 12.2 M of 14 M targeted by end of April 2024. | [20] (secondary) |
| **MR follow-up, October 2026** | 1–5 October 2026, children 9–59 months in all 146 districts and cities, including refugee settlements, regardless of prior status. Target 7,616,746 (WHO: "nearly 7.9 million"); 8,285,835 vaccinated. MR2 was added to the routine schedule in October 2022. | [8], [9] |
| **Mpox, 2025** | MVA-BN, single dose. Phase I in Kawempe and Makindye (Kampala), 1–10 February 2025; Phase II from 2 April 2025; Phase III from 26 May 2025 in Kyotera, Masaka City, Lyantonde, Kalangala and Mukono. Targets: sex workers, MSM, people in detention, truck drivers, fisher folk, health workers and contacts. 111,400 doses received. | [10] (WHO sitrep), [21] (secondary) |
| **Sudan virus disease ring vaccination, 2025** | TOKOMEZA SVD: a WHO and MoH **trial** of IAVI's rVSV candidate, launched 3 February 2025, single dose to contacts and contacts of contacts, randomised by ring. More than 130 people in 17 rings. Outbreak declared over on 26 April 2025. This was trial use, not a programme campaign. | [11], [22] (secondary) |
| **TCV** | No Uganda campaign found. UNITAG recommended a catch-up campaign for 9 months to 15 years (routine note). The February 2026 Coalition Against Typhoid fact sheet does not list Uganda as an introducing country. | [12] |
| **HPV** | Delivered through schools and community (HMIS 105 §2.10 has school and community columns). No separate HPV campaign or multi-age cohort catch-up was found (**unverified**). | routine note |

### 2.3 Campaign data channels

- **ODK plus DHIS2 (2019 MR/OPV).** WHO AFRO's GIS Centre put campaign monitoring checklists on ODK; HISP Uganda built a DHIS2 dashboard. "When monitors submitted data, the data were sent to both the ODK and DHIS2 servers." The system ran only intra-campaign. Dashboard indicators: target population; children vaccinated (MR and OPV); coverage; doses issued, used and discarded; wastage rate; discards by reason (partial use, contamination, VVM change, other); health workers deployed and workload. A National Command Center and district command centres reviewed it daily; a call centre gave feedback. Uganda had used ODK for AFP surveillance for about four years before [5].
- **Paper before 2019.** "In past campaigns, Uganda printed paper forms and a team of supervisors ... transported the data to the next level" [5].
- **EpiVac.** "The national COVID-19 online data management system" and "the official online public COVID-19 vaccination certificate and verification platform". A person gets a PDF certificate by entering an identification number and the last six digits of the phone number used at vaccination [4]. ZDLH: EpiVac "was developed by the World Health Organization (WHO)", captures the individual, the vaccine and district stock, "has also been adapted for monitoring the progress of program campaigns for yellow fever, polio and Measles-Rubella (MR), however, it is not integrated with DHIS2" [13]. In September 2022 a backlog of about 3.9 M individual COVID-19 records (24.7 M aggregate vs 20.7 M individual doses) was cleared by 2,415 data clerks [4]. Who hosts EpiVac today, its platform and API are **unverified**.
- **Smart Paper Technology** was used "country-wide during campaigns and during the COVID-19 vaccination under pilot mode" [13]. MoH suspended SPT in 2024 (routine note).
- **DHIS2 aggregate.** In DHIS2, children vaccinated with DPT1 and DPT3 "whether during facility vaccination, through outreach, or campaigns" are summed for zero-dose estimates [13]. A dedicated DHIS2 campaign data set for Uganda was **not found**. DHIS2 publishes a generic immunization-campaign package [23], but its use in Uganda is **unverified**.
- **Outbreak response vaccination.** The IDSR guidelines give the district vaccination sub-committee these tasks: macro- and micro-planning of logistics, running campaigns and "post vaccination campaign validation", monitoring and reporting AEFI, and "Data capture during vaccination into the system" (system not named) [2, §5.3, pp. 214–215].

### 2.4 Cards, registers and certificates

- UCG 2023: a child who received a dose in a national campaign "should still get the routine vaccination doses" (routine note). The 2019 and 2026 MR campaign messages repeat this [6], [9].
- 2019: finger mark plus an immunization card as proof [14]. The card format (Child Health Card entry or a separate campaign card) is **unverified**.
- Whether campaign doses are entered in the child register (HMIS 073) is **unverified**. Campaign tallies did not flow through HMIS 105 in 2019 (ODK/DHIS2 dashboard instead) [5].
- Certificates: EpiVac issues COVID-19 certificates [4]. The ICHD rounds hand out short **birth** certificates [1]. MoH announced replacement yellow fever cards with security features (Monitor, secondary [20]). No other digital vaccination certificate was found.

## 3. Weekly surveillance: HMIS 033b

From the *National Technical Guidelines for IDSR, 3rd edition*, September 2021, Annex 2H [2, pp. 125–128]:

- **Objective:** report notifiable diseases, other priority diseases, OPD attendances, eMTCT appointments, malaria and TB tested and treated, and tracer stock balances.
- **Frequency:** "Every Monday of the Week in a calendar year". The epidemiological week runs Monday 00:00 to Sunday 23:59 [2, p. 146].
- **Copies:** triplicate: one stays at the unit, one to the HSD, the original to the DHO. **Responsibility:** health unit in-charge.
- **Zero reporting:** report every week "whether there are cases or not". Weekly data are also transcribed into **HMIS 033c** (annual summary), written as "10 (2)" for 10 cases and 2 deaths.
- **Columns per disease:** cases this week, deaths this week, tested, positive.
- **Named rows (21):** malaria (MA), dysentery (DY), SARI (SA), **acute flaccid paralysis (AF)**, **AEFI (AE)**, animal bites (AB), bacterial meningitis (MG), cholera (CH), guinea worm (GW), **measles (ME)**, **neonatal tetanus (NT)**, plague (PL), typhoid (TF), **hepatitis B (HB)**, rifampicin-resistant TB (DR), **yellow fever (YF)**, other VHF (VF), maternal death (MD), macerated (MB) and fresh (FB) still births, early neonatal deaths (ND).
- **"Other conditions" codes for mTrac:** include **diphtheria (DP)**, **pertussis (WC)**, influenza-like illness (IL), adverse drug reactions (AR), and others.
- **Submission:** mTrac. "Facilities submit the mTrac HMIS 033B surveillance form" weekly, and the data are "automatically integrated into DHIS2". More than 62,000 registered health workers in 4,431 facilities. Threshold alerts by SMS go to the district team [15]. The IDSR guidelines name SMS 6767 for eIDSR community reports [2, p. 377].
- **Deadline:** the DQIP (routine note) says "every Monday before mid-day". A 2023 study measured timeliness as reports in DHIS2 "by 12:00pm Monday of the following week" (secondary [24]). Performance targets: 100% complete, 90% on time [2, §8].
- **Other weekly or monthly VPD reports** in the district IDSR logbook: "Weekly AFP polio", "Monthly Measles and yellow fever lab data", "Monthly Measles, yellow fever and NNT case-based data", "Monthly Rotavirus surveillance data" [2, Annex 2I, p. 132].

The exact mTrac SMS syntax (keywords and field order) was not found (**unverified**). Whether facilities can also key 033b directly into DHIS2 web is **unverified**.

## 4. AEFI

### 4.1 Definition and reporting rule

- IDSR 3rd ed. lists AEFI among "Diseases, conditions or events requiring immediate reporting" (Table 2) [2, p. 106]. Table 1 lists it under "Other major diseases ... of public health importance" [2, pp. 33–34].
- **Timeline:** for any immediately reportable event "the health facility must report case-based information to the next level within 24 hours of detection", first by the fastest means (phone, SMS, email), then on a written case-based form; "If a computer or other electronic device is available ... complete and submit the form electronically" [2, §2.1.1, p. 106].
- **Definition:** "Any untoward medical occurrence which follows immunization and which does not necessarily have a causal relationship with the usage of the vaccine" [2, Annex 1A, p. 56].
- **Disease summary:** for a single suspected case: treat, communicate with parents and community, respond to rumours, "Complete case investigation form". For a confirmed case: monitor for a cluster, "Send report immediately to initiate investigation of cause", take remedial action. Classify as programme-related, vaccine-induced, coincidental or unknown; beware mass psychogenic illness [2, p. 398]. (These categories predate the WHO 2018 classification used by the national committee, below.)
- AEFI is also a weekly count on HMIS 033b (row AE) [2].

### 4.2 The national form (IDSR Annex 11A)

"Reporting form for adverse events following immunization (AEFI)" [2, pp. 558–559]. Fields marked (req.) carry an asterisk on the form, meaning required:

- AEFI reporting ID number.
- **Patient:** name (req.), full address (req.), telephone, sex, date of birth (req.) or age at onset or age group (<1, 1–5, >5 years).
- **Reporter:** name (req.), institution, designation, department, address, telephone and email, date (req.).
- **Vaccination:** health facility or vaccination centre; per vaccine: name (req.), date (req.) and time (req.) of vaccination, dose number, batch/lot number (req.), expiry date.
- **Event:** checkboxes: severe local reaction (>3 days, beyond nearest joint), seizures (febrile, afebrile), abscess, sepsis, encephalopathy, toxic shock syndrome, thrombocytopenia, anaphylaxis, fever ≥38 °C, other; free-text description; date and time of onset; hospitalised (yes/no); date the patient notified the health system.
- **Outcome (req.):** recovering, recovered, recovered with sequelae, not recovered, unknown, died (date of death, autopsy done).
- Past medical history, concomitant medication.
- **First decision-making level:** "Investigation needed".
- **National level:** date received, **"AEFI worldwide unique ID"**, comments.

This layout matches the WHO AEFI reporting form. Whether UNEPI or NDA uses a newer national version is **unverified**.

### 4.3 Pathway and systems

- **NDA's role:** the National Pharmacovigilance Centre was delegated to NDA by MoH in late 2004 (secondary). Uganda's AEFI surveillance is "passive" and uses the general pharmacovigilance system for all health products [17].
- **Flow:** "AEFI is recorded in the facility vaccination register" and then transferred to NDA tools. District health workers and district surveillance officers do the work; regional referral hospitals act as regional pharmacovigilance centres. MoH and UNEPI set policy [17]. Active-surveillance studies share findings with "the National Pharmacovigilance Centre (NPC) of the NDA and the Uganda Expanded Program on Immunization (UNEPI)" (secondary).
- **Channels:** paper ADR/AEFI forms, an electronic form, toll-free hotline, USSD, **Med Safety app**, WhatsApp [17]. In November 2026 NDA named "MedSafety App, a WhatsApp reporting line, and direct reporting into VigiFlow, VigiMed for medicines, and VigiMed for vaccines" [18] (secondary).
- **VigiFlow:** NDA manages reports in VigiFlow, "a web-based WHO platform" [17]. WHO and the Uppsala Monitoring Centre trained more than 50 vaccine safety focal points in Uganda on **VigiFlow and VigiMobile** on 22–23 May 2025 [19]. VigiMobile is an offline-capable phone add-on for AEFI capture [19].
- **Med Safety app:** NDA-approved, first launched in limited settings in February 2020; a trial in HIV clinics found 73% higher ADR reporting [25]. A NEPAD document says the app includes an AEFI form (secondary). Whether Med Safety AEFI reports reach VigiFlow directly is **unverified**.
- **DHIS2:** "DHIS2 is used at public facilities to report AEFIs alongside other health data" [17]. The nOPV2 study found passive AEFI reports arriving by "telephone call, short messaging service, Open Data Kit, District Health Information System 2" [7]. UgandaEMR is not linked to NDA reporting [17].
- **eIDSR:** AEFI is immediately reportable under IDSR, so it falls within eIDSR case reporting [2]. Whether eIDSR has an AEFI-specific program stage and whether it forwards to VigiFlow is **unverified**.

### 4.4 Serious AEFI investigation and causality

- "Clinicians and healthcare workers conducted detailed investigations" of serious AEFI; cases were validated against **Brighton Collaboration** definitions; the **National AEFI Causality Committee** classified each as vaccine product-related, vaccine quality-related, immunization error-related, immunization anxiety-related, indeterminate or coincidental [7].
- The committee, set up by MoH, "periodically reviews and classifies serious AEFI" and recommends changes to safety protocols [17].
- In the 2019 MR/OPV campaign, two thirds of districts reported no AEFI, and the committee assessed 10 of 43 serious cases with clinical details (secondary).
- A national rule on how fast a serious AEFI must be investigated (WHO suggests starting within 48 hours) was **not found** in a Ugandan document (**unverified**). A national AEFI guideline from UNEPI or NDA was not found online.
- Reporting rate: about 100 pharmacovigilance reports per million population against a WHO benchmark of 200 [18] (secondary).

## 5. Case-based VPD surveillance

All from IDSR 3rd ed. [2] unless stated.

### 5.1 Common rule

- AFP (polio), measles and neonatal tetanus are immediately reportable (Table 2) and targeted for eradication or elimination (Table 1).
- Report case-based information within 24 hours; send the generic case-based form (Annex 2F) or the disease form with the specimen and the lab form (Annex 2G) [2, §2.1.1].
- The District Surveillance Focal Person (DSFP) receives facility notifications and forms, calls the regional or national PHEOC, fills the case investigation form (CIF) and sends it with the sample [2, §9.10.4].

### 5.2 Acute flaccid paralysis (Annex 11B, p. 560)

- **Suspected case:** "Any child under 15 years of age with acute flaccid paralysis or any person with paralytic illness at any age in whom the clinician suspects poliomyelitis" [2, p. 508]. Confirmed: virus isolated in stool.
- **Specimens:** two stools, the second 24–48 hours after the first, within 14 days of paralysis onset; if later than 14 days, stools from 3–5 contacts. Re-examine at 60 days if no specimen [2, pp. 507–508].
- **Performance:** 4 AFP cases per 100,000 under-15 per year; percentage reported within 48 hours; percentage with adequate specimens.
- **Reporting:** "Immediate case-based reporting of all Acute Flaccid Paralysis cases. Weekly summary reporting". The form in Annex 11B is an image in the PDF; its fields are not text-extractable (**unverified** field list).
- **Lab:** EPI Laboratory, Uganda Virus Research Institute (UVRI), Entebbe [2, p. 214].
- ODK has been used for AFP surveillance since about 2015 [5].

### 5.3 Measles (Annex 11F, pp. 571–572)

- **Suspected case:** fever and generalised maculopapular rash with cough, coryza or conjunctivitis, or clinician suspicion. Confirmed: IgM positive or epidemiological link [2, p. 480].
- **Specimens:** serum within 30 days of rash; with an elimination target, from every suspected case; during outbreaks, the first five cases then line-list [2, pp. 480–481]. Lab: UVRI EPI Laboratory.
- **Form fields:** EPID number, lab number; district, reporting unit; name, sex, age in months, date of birth; head of household, sub-county, parish, LC1; visit date; inpatient/outpatient; fever and rash with onset dates, temperature, cough, red eyes, running nose, complications; outcome; date the unit notified the district; vitamin A given and doses; **immunization history: card seen/not seen, number of measles doses, date of last dose**; blood and urine specimen dates and condition; investigator; IgM result, virus isolation; **final classification** (confirmed, epi-linked, compatible, discarded, suspected); date results sent to district.

### 5.4 Neonatal tetanus (Annex 11G, pp. 573–575)

- **Suspected case:** normal suck and cry for the first 2 days, then between day 3 and 28 cannot suck and becomes stiff or convulses. No lab confirmation [2, p. 491].
- **Response:** investigate; if confirmed, immunize the mother and pregnant women in the locality with at least 2 doses of TT/Td, and run a supplemental campaign for women of childbearing age.
- **Form fields:** child name, date of birth, age in days, sex, mother's and father's names, residence; admission and onset dates, symptoms; outcome and date; place of delivery and facility; birth attendant; delivery surface; cord-cutting instrument; cord-stump application; mother's age, education, occupation; birth order; ANC visits and facility type; **mother's TT1–TT5 dates and clinics, "by card" or "by history"**.

### 5.5 eIDSR

- Built on DHIS2 by MoH with CDC and HISP Uganda; approved by the National eHealth TWG in **August 2017** as "a Ministry of Health-owned system for registering and reporting of new cases of notifiable diseases" [2, §9.10.1, p. 373].
- **Capabilities:** SMS and email notifications on a new suspect case, lab request, specimen movement, result release or death; specimen tracking through the hub system; outbreak IDs; outbreak and contact-tracing modules; **extraction of weekly 033b aggregates from mTrac** for validation against case data; outbreak confirmation and closure; merging outbreaks into clusters [2, §9.10.3].
- **Access:** online on the MoH DHIS2; zero-rated on MTN under an MoU. Main district users are DSFPs. Channels: SMS to 6767, Android app, web [2, §9.10.4–9.10.6].
- Whether health facilities (not only DSFPs) enter AFP, measles and NNT CIFs directly, and the eIDSR program and data-element identifiers, are **unverified**.

## 6. Open questions

- A current UNEPI or NDA AEFI guideline, its form version and investigation timelines.
- Whether campaign doses are written on the Child Health Card and in the child register, and the format of the campaign card.
- A DHIS2 campaign data set or tracker for Uganda; EpiVac's host, platform, API and current use.
- mTrac 033b SMS syntax; direct DHIS2 entry of 033b.
- eIDSR program identifiers for AFP, measles, NNT and AEFI; whether eIDSR passes AEFI to VigiFlow.
- Field list of the AFP case investigation form (Annex 11B is an image).
- HPV and TCV campaign plans.

## 7. Sources

1. Uganda MoH, *Integrated Child Health Days Implementation Guide*, June 2015, §§2–4 and Annex A. <https://library.health.go.ug/file-download/download/public/1726>
2. Uganda MoH, *National Technical Guidelines for Integrated Disease Surveillance and Response*, 3rd edition, September 2021: Table 1 (pp. 33–34), Annex 1A (p. 56), §2.1.1 and Table 2 (pp. 105–106), Annex 2H HMIS 033b (pp. 125–128), Annex 2I (p. 132), §2.3 epi week (p. 146), lab directory (p. 214), §5.3 sub-committees (pp. 214–215), §9.10 eIDSR (pp. 373–377), §11 summaries (measles pp. 480–481, NNT p. 491, AFP pp. 507–508, AEFI p. 398), Annexes 11A, 11B, 11F, 11G (pp. 558–575). <https://www.afro.who.int/sites/default/files/2021-09/2_Uganda%203rd%20IDSR%20Tech%20Guideline_PrintVersion_10Sep2021.pdf>
3. Routine immunization research note, branch `research/immunization`, `docs/research/immunization.md` (2026-11-18): UCG 2023 campaign rule, DQIP mTrac deadline, HMIS 105 layout, SPT suspension.
4. FHI 360 EpiC, *Mainstreaming COVID-19 Vaccination Data in Uganda: Challenges and Solutions*, August 2023 (secondary). <https://www.fhi360.org/wp-content/uploads/2024/02/resource-epic-covid-19-uganda-mainstreaming-vaccination-data-challenges-solutions.pdf>
5. UNICEF, *The Use of Digital Technologies and Approaches for Real-Time Monitoring of Supplementary Immunization Activities*, January 2021, Case Study 3 Uganda (pp. 42–48). <https://www.gavi.org/news/document-library/using-digital-technologies-real-time-supplementary-monitoring-immunisation> ; ODK Forum showcase, 8 April 2021. <https://forum.getodk.org/t/odk-and-dhis2-used-in-uganda-to-help-vaccinate-19-5-million-children/33121>
6. WHO AFRO, "More than 18 million children in Uganda to be immunized against Measles, Rubella and Polio in Mass Campaign", 15 October 2019. <https://www.afro.who.int/fr/node/11867>
7. Emerging Infectious Diseases 30(4), 2024, "Safety of nOPV2 ... Uganda" (passive, active, AFP and cohort surveillance; National AEFI Causality Committee) (secondary). <https://wwwnc.cdc.gov/eid/article/30/4/23-1361_article>
8. Monitor, "8.2m children vaccinated as Uganda battles recurring measles outbreaks", 7 October 2026 (secondary). <https://www.monitor.co.ug/uganda/news/national/8-2m-children-vaccinated-as-uganda-battles-recurring-measles-outbreaks-5623390> ; Kampala Post, 30 April 2026 (secondary). <https://kampalapost.com/articles/health-ministry-announces-vaccination-campaign-as-uganda-battles-measles-outbreak>
9. WHO AFRO, "Reaching every child: Uganda's nationwide push against measles and rubella", 2 October 2026. <https://www.afro.who.int/countries/uganda/news/reaching-every-child-ugandas-nationwide-push-against-measles-and-rubella>
10. WHO AFRO / MoH, *Uganda Mpox Situation Report*, 19 May 2025. <https://www.afro.who.int/sites/default/files/2025-05/Ug_Mpox_Sitrep_19May25.pdf>
11. WHO, "Groundbreaking Ebola vaccination trial launches today in Uganda", 3 February 2025. <https://www.who.int/news/item/03-02-2025-groundbreaking-ebola-vaccination-trial-launches-today-in-uganda>
12. Coalition Against Typhoid, *Uganda TCV Fact Sheet*, February 2026. <https://www.coalitionagainsttyphoid.org/wp-content/uploads/2026/03/Uganda-TCV-Fact-Sheet-Feb2026.pdf> (via the routine note)
13. Gavi Zero-Dose Learning Hub Uganda, *Utility of data capture platforms for identifying zero dose children in Uganda*, October 2024 (EpiVac pp. 9 and 25; HMIS 073/076 p. 13; SPT p. 16; DHIS2 zero-dose p. 27). <https://zdlh.gavi.org/sites/default/files/2024-12/UGANDA_LH_Data_EcoSystems_OCT2024.pdf>
14. Watchdog Uganda, 25 October 2019 (finger mark and card, attributed to MoH) (secondary). <https://watchdoguganda.com/?p=81334> ; Nile Post, 22 October 2019 (secondary). <https://nilepost.co.ug/health/57946/ugandas-mandatory-measles-immunisation-stirs-debate-on-social-media>
15. UNICEF Uganda, "mTrac". <https://www.unicef.org/uganda/what-we-do/mtrac>
16. IDSR 3rd ed. Annex 11A, see [2].
17. Nambasa V et al. (NDA authors), "Pharmacovigilance for Vaccines Used in Pregnancy: A Gap Analysis From Uganda", *Pediatr Infect Dis J* 2025;44(2 Suppl):S123–S129. <https://pmc.ncbi.nlm.nih.gov/articles/PMC7617404>
18. Monitor, "NDA moves to digitise reporting of medicine side effects in HIV guidelines", 13 November 2026 (secondary). <https://www.monitor.co.ug/uganda/news/national/nda-moves-to-digitise-reporting-of-medicine-side-effects-in-hiv-guidelines-5669758>
19. WHO Regulation and Prequalification, Pharmacovigilance team page (VigiFlow and VigiMobile training, Entebbe, 22–23 May 2025). <https://www.who.int/teams/regulation-prequalification/pharmacovigilance> ; UMC, VigiFlow for AEFI. <https://who-umc.org/pv-products/vigiflow-for-aefi> (via search summaries)
20. AP via News4Jax, "Uganda tackles yellow fever with new travel requirement, vaccination campaign", 28 May 2024 (secondary). <https://www.news4jax.com/news/world/2024/05/28/uganda-tackles-yellow-fever-with-new-travel-requirement-vaccination-campaign-for-millions/> ; Monitor, "Govt to replace yellow fever cards" (secondary). <https://www.monitor.co.ug/uganda/news/national/govt-to-replace-yellow-fever-cards-4584210> (via search summaries)
21. Gavi VaccinesWork, "Mpox vaccines are beginning to roll out in Uganda"; European Commission HERA, 21 January 2025 (secondary). <https://www.gavi.org/vaccineswork/mpox-vaccines-are-beginning-roll-out-uganda-and-sex-workers-are-front-queue> ; <https://health.ec.europa.eu/latest-updates/hera-delivers-mpox-vaccines-support-ugandas-outbreak-response-2025-01-21_en> (via search summaries)
22. IAVI, "About IAVI's Sudan virus vaccine program" (secondary). <https://www.iavi.org/fact-sheet/about-iavis-sudan-virus-vaccine-program/> (via search summary)
23. DHIS2 documentation, Immunization campaigns package. <https://docs.dhis2.org/en/implement/health/immunization/immunization-campaigns/use.html> (via search summary)
24. *Timeliness and completeness of weekly surveillance data reporting on epidemic prone diseases in Uganda, 2020–2021*, BMC Public Health 2023 (secondary). <https://bmcpublichealth.biomedcentral.com/articles/10.1186/s12889-023-15534-w> (via search summary)
25. Kiguba R et al., Med Safety cluster-randomised trial protocol (PMC9252195) and results release, Makerere University, 2025 (secondary). <https://www.ncbi.nlm.nih.gov/pmc/articles/PMC9252195/> ; <https://chs.mak.ac.ug/news/new-research-findings-smartphone-app-boosts-drug-safety-reporting-uganda-landmark-trial-finds> (via search summaries)

# Research: immunization in Uganda

Findings for the research ticket [What does immunization in Uganda require: schedule, reporting, existing systems and WHO SMART IMMZ?](https://github.com/carapai/people-centered-healthcare/issues/605). This note reports facts and decides nothing. The decision ticket is #606. Related design: [program-designer.md](../architecture/program-designer.md), ADR 0074 (decision support as FHIRPath decision tables translated from WHO content), ADR 0068 and 0069 (indicators, HMIS reporting to DHIS2), [inventory-and-procurement.md](../architecture/inventory-and-procurement.md), [patient-identity.md](../architecture/patient-identity.md).

Research date: **2026-11-18**. Every source was read on that date unless a date is given. Claims marked **unverified** have no primary source yet. "Secondary" means a news item, partner page or study that reports the fact, not the Ministry of Health (MoH) document itself.

## 1. Summary

| Topic | Key facts | Main sources |
|---|---|---|
| Schedule | The latest MoH schedule text found is the *Uganda Clinical Guidelines 2023* (UCG), "adapted from the UNEPI/MOH Immunization Schedule, 2022". Birth: BCG, HepB birth dose, OPV0. 6/10/14 weeks: OPV, DPT-HepB-Hib, PCV; IPV at 6 and 14 weeks; rotavirus at 6 and 10 weeks. 9 months: MR1, yellow fever. 18 months: MR2. HPV for girls. Td for women 15–45/49 in five doses. Added since: malaria vaccine R21/Matrix-M (April 2025, 6/7/8/18 months, 105 districts). HPV moved to one dose (UNITAG recommendation). No hexavalent and no typhoid conjugate vaccine in routine use yet. | §2 |
| Catch-up rules | UCG gives only general rules: give each dose at the first contact after the due age; minimum 4 weeks between DPT-HepB-Hib doses; campaign doses do not replace routine doses; sick children are vaccinated at discharge. No age limits per antigen were found. | §2.3 |
| Reporting | Paper chain: child health card → child register and tally sheet → HMIS 105 (monthly, to the district by the 7th, into DHIS2 by the 15th). Weekly vaccine-preventable disease counts go by mTrac (HMIS 033b). HMIS 105 EPI cells are aggregate counts by antigen and dose, split by age band and static/outreach. DHIS2 holds no client-level routine immunization data. | §3 |
| Existing systems | DHIS2 (national, aggregate). eCHIS (community, 19 districts in 2024, not yet in DHIS2). Smart Paper Technology (scanned paper, 11 districts, suspended by MoH in 2024). EpiVac (COVID-19 and campaign records, certificates by NIN or phone). UgaVax, Smart Health App, house-to-house registers: pilots or prototypes. UgandaEMR has no current EPI report; mTrac and EMRs "do not currently capture childhood immunisation". There is no national electronic immunization registry. | §4 |
| MoH requirements | The UNEPI M&E Plan 2024–2028 lists features required of any digital immunization system: life-course records, stock with batch and expiry, scheduling and reminders, AEFI, offline mode with sync, HL7/FHIR exchange, a parent portal, QR/barcode capture, audit, automatic reporting. | §4.2 |
| Field operations | Static and outreach sessions. Defaulter tracing from the child register, often with VHTs. Multi-dose vial policy in UCG: discard opened vials taken on outreach; discard reconstituted BCG, YF and MR after 6 hours or at session end. VVM stage 3–4 means discard. Stock is tracked in the VIMCB on paper; MoH says "eLMIS… not in existence". | §5 |
| WHO SMART IMMZ | Published IG 1.0.0 (2026-10-02), FHIR **4.0.1**. 9 business processes (A–I). Data dictionary: 17 client-registration and 194 administration data elements. Decision tables and schedules for 23 vaccines, 279 CQL libraries, 138 PlanDefinitions, 41 indicator Measures. Content licence **CC BY-NC-SA 3.0 IGO**; software BSD-3. The IG's `sushi-config.yaml` says `CC0-1.0`, which conflicts. The IG creates `MedicationRequest` proposals, not `ImmunizationRecommendation`. Its malaria table models RTS,S, not R21. | §6 |
| FHIR R5 | `Immunization` (maturity 5), `ImmunizationRecommendation` and `ImmunizationEvaluation` (maturity 1). In R5 `doseNumber` and `seriesDoses` are strings, and `Immunization.basedOn` can point to an `ImmunizationRecommendation`. | §7 |
| Child identity | A NIN is issued at birth registration, but under-one birth registration was about 30–36% in 2022–23. Health facilities notify births to NIRA (Mobile VRS). The child health card and the child register carry name, date of birth, parents and phone. SPT gave each child a unique ID written on the card. | §8 |

## 2. Schedule

### 2.1 Routine schedule (UCG 2023)

Source: *Uganda Clinical Guidelines 2023*, §18.1.1 "National Immunization Schedule", "Adapted from the UNEPI/MOH Immunization Schedule, 2022" (pp. 872–880).

| Antigen | Age | Dose and route | Site |
|---|---|---|---|
| BCG | At birth or first contact | 0.05 mL to 11 months, 0.1 mL above 11 months; intradermal | Right upper arm |
| Hepatitis B birth dose | At birth (first contact within the first 7 days) | 0.5 mL IM | Left thigh |
| OPV | Birth (OPV0, ideally at birth, otherwise in the first 2 weeks), 6, 10, 14 weeks | 2 drops oral | Mouth |
| IPV | 6 and 14 weeks | 0.5 mL IM | Right thigh, 2.5 cm from PCV |
| DPT-HepB-Hib | 6, 10, 14 weeks | 0.5 mL IM | Left thigh |
| PCV | 6, 10, 14 weeks | 0.5 mL IM | Right thigh |
| Rotavirus | 6 and 10 weeks | 2 drops oral | Inner cheek |
| Measles-rubella | 9 and 18 months | 0.5 mL subcutaneous | Left upper arm |
| Yellow fever | 9 months | 0.5 mL subcutaneous | Right upper arm |
| HPV | Girls in primary 4, or girls aged 10 outside school | "2 doses IM 6 months apart" (since changed, see §2.2) | Left upper arm |

Other UCG rules:

- **Td for women of childbearing age** ("15–45 years" in UCG §18.1.4; other MoH material says 15–49). Td1 at first contact or as early as possible in pregnancy. Td2 at least 4 weeks after Td1. Td3 at least 6 months after Td2. Td4 at least 1 year after Td3. Td5 at least 1 year after Td4. High-risk adults get the same series and boosters every 10 years.
- **Hepatitis B for adolescents and adults** in high-risk groups (health workers and others): 0, 1 and 6 months, preferably after testing.
- **Measles exposure:** an unimmunised child of 6–9 months exposed to measles can get MR early and must repeat it at 9 months.

UCG still shows the "2022" schedule. No later full MoH schedule document was found online (**unverified** whether a 2025 or 2026 schedule card exists).

### 2.2 Changes since 2022

| Change | Status | Source |
|---|---|---|
| **Malaria vaccine R21/Matrix-M** | Launched 2 April 2025 in Apac. Four doses at 6, 7, 8 and 18 months. 105 high and moderate transmission districts (one government page says 107). About 1.1 M children under two. 2.278 M of 3.5 M doses flagged off on 7 March 2025. UNITAG recommended R21. | WHO AFRO; UNICEF Uganda press release 12 March 2025; PATH 2 April 2025; UNITAG combined reports |
| **HPV single dose** | UNITAG recommended one dose of Cervarix or Gardasil for immunocompetent girls aged 9–14. Immunocompromised girls, including those with HIV, keep 3 doses (at least 2), with doses 2 and 3 at care clinics. Uganda now runs a single-dose schedule. The date it took effect was not found (**unverified**). | UNITAG combined reports 2023–2025 (§6 of the HPV report); New Vision, May 2026 (secondary) |
| **HepB birth dose** | UNITAG recommended it in April 2022. UCG 2023 lists it. Gavi-linked reporting says it was introduced in 2025 with 75% coverage. The national start date is **unverified**. | UNITAG HepB BD recommendation (April 2022); UCG 2023; Nile Post (secondary) |
| **IPV second dose** | UCG 2023 lists IPV at 6 and 14 weeks. The 2014-era HMIS 105 form has a single "IPV" row. | UCG 2023; UgandaEMR HMIS 105 template (§3.2) |
| **Typhoid conjugate vaccine (TCV)** | UNITAG (November 2023) recommended a single dose at 9 months with a catch-up campaign for 9 months to 15 years. It is not yet introduced: not in the schedule as of January 2025 (One Health Trust), and Uganda is not on the Coalition Against Typhoid list of TCV countries (February 2026). | UNITAG combined reports; Coalition Against Typhoid fact sheet Feb 2026 |
| **Hexavalent (DTwP-HepB-IPV-Hib)** | Not introduced. A July 2025 Nile Post piece says Uganda had not rolled it out. No MoH or Gavi plan was found. | Nile Post (secondary) |
| **Mpox and Ebola vaccines** | Campaign use only. MVA-BN for ages 12–50 in hotspots; UNITAG addenda on children under 12 and fractional doses. ERVEBO recommended for preventive Ebola campaigns among high-risk groups. | UNITAG combined reports |

MoH says the routine programme now covers 14 vaccine-preventable diseases (Gavi-linked reporting, secondary).

### 2.3 Catch-up rules

UCG 2023 gives these general rules only:

- "Give each vaccine at the recommended age or if this is not possible, at any first contact with the child after this age."
- Minimum interval between DPT-HepB-Hib doses is 4 weeks.
- MR is given at 9 and 18 months "or first contact after this age".
- A child who received a dose in a national campaign "should still get the routine vaccination doses".
- Admit and treat a severely ill child and vaccinate at discharge. Minor illness is not a contraindication.
- Do not give BCG to a child with signs of immunosuppression such as AIDS.

No Ugandan document was found that sets upper age limits per antigen (for example rotavirus, BCG, OPV0) or a catch-up table for older children (**unverified**). WHO's position papers (encoded in SMART IMMZ, §6) do set such limits.

## 3. Reporting

### 3.1 The paper chain

The UNEPI *EPI Data Quality Improvement Plan 2024–2028* (DQIP) describes the flow:

1. A **child health card** is issued to every vaccinated child (parent-held).
2. Each dose is written in the **child register** (the "Integrated Child Health Register") and tallied on the **child tally sheet** during the session.
3. The facility compiles the **HMIS 105 monthly report**. It goes to the district by the 7th and into DHIS2 by the 15th. Data cleaning runs for about 5 days after entry.
4. **Weekly** vaccine-preventable disease reports (HMIS 033b) go by **mTrac** SMS "every Monday before mid-day".
5. Case-based surveillance (AFP, AEFI, neonatal tetanus, measles, yellow fever) goes through **eIDSR**, built on DHIS2.

Other EPI tools in the DQIP tool-availability survey (May 2024): the coverage **monitoring chart** and the **VIMCB** (Vaccine and Injection Materials Control Book, the stock ledger). The child register is also used "to identify immunization defaulters and replace Child health cards when they are lost" (MyChild external evaluation, 2018, secondary). UCG says: "Record every vaccination in the child register and on a tally sheet… Use the child register and child health card for tracking drop outs."

Form numbers: the DQIP calls the tally sheet "HMIS 006". The MyChild evaluation calls it HMIS 073a and the register HMIS 073. The current numbering is **unverified**.

Lower-level facilities (HC II and some HC III) do not enter data into DHIS2. Their forms are entered at the HC IV under each facility's own organisation unit (Gavi ZDLH report, October 2024).

### 3.2 What HMIS 105 asks for

The only machine-readable layout found is UgandaEMR's template `HMIS_105-2.8-2.12.xls` (last changed 2017-12-18). It follows the 2014 revision of the form, and its DHIS2 data set is labelled "HMIS 105:2.8-2.12":

- **2.8 Child health:** vitamin A and deworming doses, by 6–11 months, 12–59 months, 1–4 years and 5–14 years.
- **2.9 Tetanus (TT):** T1–T5, for pregnant and non-pregnant women, in school, static and outreach.
- **2.10 HPV:** doses 1–3, school and community, by single year of age 9–14 and 15+.
- **2.11 Child immunisation:** BCG, protection at birth for tetanus (PAB), polio 0–3, IPV, DPT-HepB+Hib 1–3, PCV 1–3, rotavirus 1–3, measles, "fully immunized by 1 year", DPT-HepB+Hib doses wasted. Columns are under 1 and 1–4 years (static and outreach disaggregation **unverified** for this section).
- **2.12 Hepatitis B for health workers:** doses 1–3, by sex.

This layout predates MR2, yellow fever, IPV2, HepB birth dose, malaria vaccine and single-dose HPV. The DQIP names the DHIS2 data set "HMIS 105:02-03 - OPD Monthly Report (MCH, FP, EID, EPI & HEPB)" and notes "use of outdated EPI data tools with no new vaccines". The DQIP also compares wastage from the VIMCB with wastage reported in HMIS 105 for BCG, yellow fever, HPV and MR. The current (2020 or later) EPI section layout, its data elements and its category combos were **not found** (**unverified**). UgandaEMR has 2019 and 2024 templates for other HMIS 105 sections, but none for EPI.

### 3.3 Data quality facts (DQIP, May 2024 national DQA)

- Child register vs HMIS 105 differed by 14% for BCG; HMIS 105 vs DHIS2 by 0%. DHIS2 and HMIS 105 often showed more doses than the register and tally sheet.
- 52% of sampled facilities had complete HMIS 105 EPI cells.
- Tool stock-outs by district: tally sheets 31%, HMIS 105 24%, monitoring chart 24%, immunization register 14%.
- 40% of districts reported over 100% DPT1 coverage in 2023 (denominator problems).
- HMIS 105 on-time reporting was below 90% from 2020 to 2023.

### 3.4 Outreach sessions

Static sessions run at the facility. Outreach sessions run in the community, about 5 km from the facility and monthly in one Hoima study (secondary). HMIS 105 separates static and outreach for at least Td (§3.2). The MoH Integrated Child Health Days guide says ICHD results are "summarised at the end of each month in HMIS 105" for both static and outreach sessions. The national immunization programme review (2021) found that all rural facilities ran outreach, but 16% of urban facilities did not, and 44% of facilities lacked staff for regular outreach.

## 4. Existing systems

### 4.1 Inventory

From the Gavi Zero-Dose Learning Hub report *Utility of data capture platforms for identifying zero dose children in Uganda* (October 2024, fieldwork March–December 2023), unless stated:

| System | What it is | Status | Integration |
|---|---|---|---|
| **DHIS2 (eHMIS)** | National aggregate HMIS at `hmis.health.go.ug` | National. MoH funded. | Hub for everything else. "No client level data for routine immunization" (DQIP). |
| **DHIS2 e-Tracker** | DHIS2 Tracker for client-level capture | DQIP recommends it for client-level routine immunization; AEFI tracker exists | Inside DHIS2. Scale for routine immunization **unverified**. |
| **eCHIS** | VHT mobile app (ICCM, RMNCH, FP, ANC, PNC, immunization, stock, supervision) | 19 districts in 2024; national rollout planned. Built on Living Goods' Smart Health App. | "Not yet synchronized into DHIS2" because of technical problems. |
| **Smart Paper Technology (SPT)** | Scannable paper forms; each child gets a unique ID written on the child health card | Piloted 2013–2020; 10 districts from February 2022; 11 districts in 2023. **Suspended by MoH in 2024** over data loss. | Uploads to its own server. A Gavi article (2 Nov 2023) says it was cloud hosted and Uganda's data policy requires in-country servers. |
| **EpiVac** | Online platform for COVID-19 vaccination records; also used to monitor yellow fever, polio and MR campaigns | In use for campaigns | People download COVID-19 certificates by NIN (or passport) and the last six digits of the phone number registered at vaccination (Monitor, secondary). Platform **unverified**. |
| **UgaVax** | SMS reminders, digital register and digital card; planned HPV registry for 12–35 | Not rolled out at the time of the study | — |
| **House-to-house registration** | Paper registers kept by VHTs | UNICEF pilot in 4 districts | Excel |
| **mTrac** | SMS reporting (short code 6767) | National | Feeds HMIS 033b into DHIS2. Does not capture childhood immunization. |
| **EMRs (UgandaEMR, eAFYA, ClinicMaster)** | Facility EMRs | Ongoing rollout | The ZDLH report says EMRs "do not currently capture information on childhood immunisation". UgandaEMR's reports module has no current EPI template (§3.2). |
| **eLMIS for vaccines** | — | "Not in existence" (DQIP). The DQIP plans to introduce one. | — |
| **Mobile VRS** | NIRA birth and death registration from facilities (web) and communities (USSD) | In use | Linked to NIRA; DQIP plans to use birth registrations as a denominator source. |

The ZDLH report recommends a nationwide tracking system "where every child registered at birth can be tracked… irrespective of where they seek health care services", linked to civil registration. No national electronic immunization registry exists today.

### 4.2 MoH requirements for digital immunization systems

The *UNEPI M&E Plan 2024–2028*, Table 4.1 "Features of the digital Immunization system", lists what "will be required for all digital systems":

- **Functional:** authentication and role-based access; life-course patient immunization records; vaccine inventory with batch numbers, expiry dates and low-stock alerts; scheduling, reminders and rescheduling; coverage, wastage and AEFI reports; integration with EHRs; AEFI reporting and tracking.
- **Non-functional:** security and privacy; scale; availability with backup; usability; **interoperability "using standardized formats (e.g., HL7, FHIR)"**; **an offline mode** that syncs when connectivity returns; a **parent or guardian portal**; multilingual support; vaccine information for patients; data migration from paper; audit logging; **barcode or QR code capture**; a mobile app; validation checks; data sharing with schools and agencies; and **automatic reporting to health authorities**.

The MoH *Guidelines for the Introduction of Digital Health Solutions* (January 2023) require digital solutions to transmit the HMIS data set to eHMIS/DHIS2 (see the DHIS2 research note on branch `research/dhis2-hmis`).

## 5. Field operations

- **Sessions:** static (daily at facilities) and outreach (community posts). Uganda uses the RED/REC (Reaching Every District / Every Child) approach with microplans. Integrated Child Health Days twice a year add outreach (national review 2021; ICHD guide).
- **Defaulter tracing:** done from the child register. A district explained: the focal person checks the register for children past their return date, lists them by village and gives names and parents' phone numbers to the VHT. Another district uses an appointment book "the way of ART clinic". 24% of facilities had no or poor defaulter tracking (national review 2021). The DQIP lists "absence of defaulter tracking" and "complexity in filling the registers especially subsequent doses" as root causes of poor data.
- **Child health card:** parent-held. Issued at the first dose. Used with the register to track drop-outs. When lost it is rebuilt from the register. Its exact fields were not checked (**unverified**). A new vaccine needs the card and M&E tools updated (UNITAG TCV report).
- **Multi-dose vial policy (UCG 2023):**
  - OPV and IPV; and DPT-HepB-Hib, HepB, TT and PCV: discard if contaminated or unlabelled, VVM at or past the discard point, opened for 4 weeks, **opened during outreach**, or not kept in the cold chain. The second group is also discarded if frozen. A separate UCG rule says never to use frozen DPT-HepB-Hib, HepB, PCV, rotavirus, IPV, HPV or TT/Td.
  - BCG, yellow fever, MR: discard opened vials 6 hours after reconstitution or at the end of the session, whichever comes first.
  - The outreach rule is stricter than WHO's 2014 policy, which lets some vials return from outreach if conditions are met (**unverified** against the WHO text in this pass).
- **VVM:** never use a vaccine whose VVM is at stage 3 or 4.
- **Cold chain:** +2 to +8 °C at facilities. OPV and MR may be stored at −20 °C in district and central stores. Fridge temperature recorded twice a day including weekends. 30-day temperature monitoring devices since 2015, with low reporting rates (national review 2021).
- **Stock reporting:** the VIMCB records doses issued and wastage. HMIS 105 carries "doses wasted" counts. District stores (DVS) supply facilities, and transport was insufficient or partial in 69% of districts. MoH states there is no eLMIS for vaccines. The tool districts use for stock (WHO's Stock Management Tool or similar) was not confirmed (**unverified**).

## 6. WHO SMART Guidelines for immunization (IMMZ DAK)

### 6.1 Release and licence

- **Published IG:** `smart.who.int.immunizations` 1.0.0, release, FHIR **4.0.1**, dated **2026-10-02** (`package-list.json`). The repository's latest commit is `bd7fa72` (2026-10-01). The index page also carries a disclaimer: "a demo working specification and may not be used for any implementation purposes".
- **Content licence:** `LICENSE.md` and the IG's license page say all WHO-authored SMART Guidelines are under **Creative Commons Attribution-NonCommercial-ShareAlike 3.0 IGO**. Adaptations must use "the same or equivalent Creative Commons licence". There must be no suggestion that WHO endorses a product, and no WHO logo. Translations need a stated disclaimer.
- **Software licence:** the same file puts software under **BSD 3-clause**. It does not say which files count as software. Whether CQL, FSH or FHIR JSON are "software" is not stated (**unverified**).
- **Conflict:** `sushi-config.yaml` and `dak.json` both declare `license: CC0-1.0`, so the generated ImplementationGuide resource says CC0. The repository's GitHub licence is "Other". Which statement governs is **unverified**.
- **Dependencies:** `smart.who.int.base` 0.2.0, `hl7.fhir.uv.extensions.r4` 5.1.0, `hl7.fhir.uv.cql` 1.0.0; ActivityDefinitions use the CPG IG 2.0.0 profiles.

### 6.2 Contents

- **Business processes (L2):** A vaccination location registration, B plan service delivery, C client registration, D administer vaccine, E client reminder, **F defaulter tracing**, G resolve duplicate client records, H resolve duplicate vaccination events, I report generation. The actors include "Electronic immunization registry (EIR) staff".
- **Data dictionary** (`input/L2/IMMZ DAK_core data dictionary.xlsx`): sheets for IMMZ.C client registration (17 data elements), IMMZ.D administer vaccine (194), IMMZ.I report generation, and IMMZ.Z vaccine library. Client registration has a unique identifier ("national identification [ID], health ID, immunization information system ID, medical record ID"), a caregiver's names ("next of kin (e.g. partner, husband, mother…)"), and an identifier "to associate the immunization event represented in a paper vaccination card to its digital representation(s)".
- **Decision-support logic** (`input/decision-logic/IMMZ DAK_decision-support logic.xlsx`): one sheet per vaccine: BCG, hepatitis B, polio, DTP, Hib, pneumococcal, rotavirus, measles, rubella, HPV, typhoid, JE, yellow fever, TBE, cholera, meningococcal, hepatitis A, rabies, dengue, malaria, mumps, seasonal influenza, varicella.
- **Computable artefacts:** 279 CQL libraries, 138 PlanDefinitions in three families:
  - **IMMZ.D2.DT** (59): "determine required vaccination(s)", one per schedule variant (for example DTP on-time start, DTP delayed or interrupted series, DTP in pregnancy starting with 3 or 4 doses, HPV single dose, HPV 2 doses, HepB birth dose, polio bOPV+IPV, malaria). Each is a decision table: rows of conditions ("client's age is less than 5 months", "latest dose given less than 4 weeks ago") leading to "due", "not due" or "complete", with guidance text.
  - **IMMZ.D5.DT** (22): contraindications per vaccine. Some are still stubs (`//TODO: Add needed actions` in the malaria one).
  - **IMMZ.D18.S** (57): schedules. Each dose has a trigger event (birth, or "count of primary doses = n"), a trigger date and a create condition such as "minimum interval of 4 weeks".
- **Indicators:** 41 Measures (`IMMZIND01`–`IMMZIND45`) with CQL logic and YAML test cases. The CQL spike found that `octofhir-cql` translates 0 of the 41 (branch `research/cql-spike`).
- **Questionnaires** for the main L2 activities (C4, D1, D2, D5, D7, D13, D17, D18, D20, D21) and logical models for the same.
- **Profiles (R4):** `IMMZ.Patient` (identifier 1..*, name text, gender, birthDate, address and a phone all required), `IMMZ.Caregiver` (RelatedPerson), `IMMZ.Immunization` (statusReason, vaccineCode from the IMMZ vaccine library, `protocolApplied` with series and string `doseNumber` required, and a due-date-of-next-dose extension), `IMMZ.Observation`, `IMMZ.AdverseEvent` (from IMMZ.D17 report AEFI).

### 6.3 How it maps to FHIR R5

- **Outputs.** A D2 decision table's "due" outcome creates a **`MedicationRequest`** with intent `proposal` through the `IMMZD2DTMR` ActivityDefinition (CPG `cpg-immunizationactivity`), plus a `CommunicationRequest` for guidance. Contraindications use `IMMZD5DTMR` ("Don't administer"). No file in the repository mentions `ImmunizationRecommendation` or `ImmunizationEvaluation`. The CQL pseudocode names the outcome "Immunization recommendation status" (Due, Not due, Complete), which would map to `ImmunizationRecommendation.recommendation.forecastStatus` **(analysis, not stated by WHO)**.
- **Inputs.** The logic reads `Immunization` history, Observations and Patient age.
- **R4 to R5 differences** that touch these profiles (from the R5 pages): `protocolApplied.doseNumber` is a string in R5 (WHO already constrains it to string); `reaction.detail` became `reaction.manifestation`; `reasonCode`/`reasonReference` became `reason`; `programEligibility` became a backbone element; `Immunization.basedOn` is new and can reference `ImmunizationRecommendation` or `MedicationRequest`. `AdverseEvent` changed substantially in R5. The cql-spike note lists the same Immunization shifts.
- **Malaria product gap.** WHO's malaria table and schedule text describe RTS,S/AS01 ("first dose… from 5 months", three doses 4 weeks apart, fourth 12–18 months after the third). Uganda uses R21/Matrix-M at 6, 7, 8 and 18 months. A Uganda table would need different ages.
- **HPV and HepB.** WHO has `HPVSingleDose`, `HPV2Doses` and `HepatitisBBirthDose` variants that match Uganda's current choices.
- **Adapting for countries.** The IG has an "Adapting Guidelines for Country use" page (`adapting.md`), not read in this pass (**unverified** what it requires).

## 7. FHIR R5 resources

From the HL7 R5 specification pages:

| Resource | Maturity / status | Key points |
|---|---|---|
| `Immunization` | 5, trial use | `status` completed / entered-in-error / not-done; `vaccineCode` 1..1; `administeredProduct`; `lotNumber`, `expirationDate`; `primarySource` and `informationSource` for reported history; `location`; `performer.actor` may be Patient or RelatedPerson; `isSubpotent`; `programEligibility`; `fundingSource`; `reaction.manifestation`; `protocolApplied` with `series`, `authority`, `targetDisease`, `doseNumber` (string, 1..1), `seriesDoses` (string); `basedOn` (CarePlan, MedicationRequest, ServiceRequest, ImmunizationRecommendation). |
| `ImmunizationRecommendation` | 1, trial use | Point-in-time forecast for a patient. `recommendation` 1..*: `vaccineCode` and/or `targetDisease` (one required), `contraindicatedVaccineCode`, `forecastStatus` 1..1 (modifier), `forecastReason`, `dateCriterion` (code + dateTime: earliest, due, overdue…), `series`, `doseNumber` and `seriesDoses` (strings), `supportingImmunization` (Immunization or ImmunizationEvaluation), `supportingPatientInformation`. |
| `ImmunizationEvaluation` | 1, trial use | Validity of one past dose against a protocol: `targetDisease` 1..1, `immunizationEvent` 1..1, `doseStatus` 1..1 (valid / not valid), `doseStatusReason`, `series`, `doseNumber`, `seriesDoses`. Overall status (due, complete) belongs in ImmunizationRecommendation. |

## 8. Identifying children

- **NIN at birth.** Birth registration is free and mandatory and gives the child a NIN (NIRA event reported by The Independent, 2026, secondary; Plan International handbook). NIRA plans "instant NIN generation" at birth registration (UNECA CRVS deck, secondary).
- **Coverage is low.** Under-one birth registration was about 30% (NIRA, ID4Africa 2023) and 36% for 2022/23 (ICAO 2023 deck). See the patient-identity research note on branch `research/patient-identity`.
- **Facility notification.** Health facilities issue a birth notification record, and Mobile VRS registers births from hospitals (web) and communities (USSD). A HEPS article says health workers have "largely shelved" the NIRA notification forms (secondary). The DQIP plans to use NIRA birth registrations as a denominator source.
- **What identifies a child today.** The child register holds name, date of birth, vaccines, return date, village, parents' names and phone (national review 2021 interview; MyChild evaluation). The child health card is parent-held. SPT gave a unique ID written on the card. EpiVac uses NIN or phone. eCHIS registers household members, but status without a card was "complex to ascertain".
- **Mother link.** Td doses are recorded for the mother and "protection at birth" (PAB) for the child (HMIS 105 §2.11). WHO's data dictionary links the child to a caregiver (RelatedPerson), not to the mother's own record. No Uganda rule was found on linking a child's record to the mother's ANC record or NIN (**unverified**).

## 9. Not verified

- The current (2020 or later) HMIS 105 EPI section: data elements, age bands, static/outreach split, the category combos and DHIS2 UIDs on `hmis.health.go.ug`.
- Current numbers of the child register and tally sheet forms (HMIS 073/073a vs HMIS 006).
- A 2025 or 2026 MoH schedule document or card. The date single-dose HPV took effect. The national start date of the HepB birth dose.
- Upper age limits and catch-up tables per antigen in Uganda.
- Whether HMIS 105 or a separate data set carries malaria vaccine doses 1–4.
- EpiVac's platform, owner and API. Whether UgaVax or the Smart Health App went live. Whether SPT was resumed.
- Scale of DHIS2 Tracker use for routine immunization in Uganda.
- Which stock tool districts use for vaccines (SMT, wVSSM or other).
- The child health card's fields and current print version.
- WHO's MDVP text, to compare with UCG's outreach rule.
- Which SMART IMMZ licence statement governs (CC BY-NC-SA 3.0 IGO vs CC0-1.0) and which files count as software.
- The IMMZ "Adapting Guidelines for Country use" page.
- Any rule linking a child's record to the mother's record or NIN.

## 10. Sources (read 2026-11-18 unless dated)

1. Uganda MoH, *Uganda Clinical Guidelines 2023*, chapter 18 Immunization, pp. 872–881. https://www.differentiatedservicedelivery.org/wp-content/uploads/UCG-2023-Publication-Final-PDF-Version-1.pdf
2. UNITAG, *Combined UNITAG Reports 2023–2025* (HPV single dose, TCV November 2023, R21 malaria, mpox addenda, Ebola). https://www.nitag-resource.org/sites/default/files/2026-03/COMBINED%20UNITAG%20REPORTS%202023-2025.pdf
3. UNITAG, *Recommendation on the introduction of HepB BD in UNEPI*, April 2022. https://www.nitag-resource.org/sites/default/files/2022-07/UNITAG%20Recommendation%20on%20the%20introduction%20of%20HepB%20BD%20in%20UNEPI%2C%20April%202022.pdf (title only, not read)
4. UNICEF Uganda, "Minister of Health receives and flags off malaria vaccines to 105 districts", 12 March 2025. https://www.unicef.org/uganda/press-releases/minister-health-receives-and-flags-malaria-vaccines-105-districts
5. PATH, "Uganda launches Africa's largest malaria vaccine rollout to date", 2 April 2025. https://www.path.org/our-impact/articles/uganda-launches-africas-largest-malaria-vaccine-rollout-to-date/
6. WHO AFRO, "Uganda launches largest malaria vaccine introduction to date" (R21/Matrix-M, Apac, 2 April 2025). https://afro.who.int/fr/node/20950 (via search summary)
7. New Vision, "Health experts push for wider HPV vaccination", May 2026 (secondary). https://www.newvision.co.ug/category/health/health-experts-push-for-wider-hpv-vaccination-NV_233596_052026
8. Nile Post, "Six diseases, one shot", July 2025 (secondary). https://nilepost.co.ug/health/273453/six-diseases-one-shot
9. Coalition Against Typhoid, *Uganda TCV Fact Sheet*, February 2026. https://www.coalitionagainsttyphoid.org/wp-content/uploads/2026/03/Uganda-TCV-Fact-Sheet-Feb2026.pdf ; One Health Trust, *GARP Uganda Policy Brief*, 2025. https://onehealthtrust.org/wp-content/uploads/2025/05/GARP-Uganda-Policy-Brief.pdf (via search summaries)
10. Uganda MoH / UNEPI, *EPI Data Quality Improvement Plan (DQIP) 2024–2028*. https://library.health.go.ug/sites/default/files/resources/EPI%20Data%20Quality%20Improvement%20Plan%20(DQIP)%202024-2028.pdf
11. Uganda MoH / UNEPI, *UNEPI M&E Plan 2024–2028*, Table 4.1. https://library.health.go.ug/sites/default/files/resources/UNEPI%20M&E%20PLAN%2024-28%20final%20Version.pdf
12. Uganda MoH, *National Expanded Program on Immunization Review Report 2021*. https://library.health.go.ug/sites/default/files/resources/NIPR%20Comprehensive%20Report%20_Final.pdf
13. Gavi Zero-Dose Learning Hub Uganda, *Utility of data capture platforms for identifying zero dose children in Uganda*, October 2024. https://zdlh.gavi.org/sites/default/files/2024-12/UGANDA_LH_Data_EcoSystems_OCT2024.pdf
14. Gavi VaccinesWork, "Paper jam: Ugandan vaccinators ditch old school ledgers and trial digital alternatives", 2 November 2023. https://www.gavi.org/vaccineswork/paper-jam-ugandan-vaccinators-ditch-old-school-ledgers-and-trial-digital
15. MyChild Solutions, *External evaluation Uganda 2018* (HMIS 073/073a naming; secondary). https://uploads-ssl.webflow.com/61b71cb9b3bcb8b89636d4c1/61d30d99297bb30eb416d28c_MyChildExternalEvaluationUganda2018.pdf (via search summary)
16. Uganda MoH, *Integrated Child Health Days* guide. https://library.health.go.ug/file-download/download/public/1726 (via search summary)
17. Static vs outreach in Hoima: https://www.ncbi.nlm.nih.gov/pmc/articles/PMC5327539/ (via search summary)
18. Monitor, "Covid-19: Vaccinated Ugandans struggling to access online certificates" (secondary). https://www.monitor.co.ug/uganda/news/national/covid-19-vaccinated-ugandans-struggling-to-access-online-certificates-3602764
19. `METS-Programme/openmrs-module-ugandaemr-reports`: `HMIS_105-2.8-2.12.xls` (last commit 2017-12-18), `SetupMoH105_28_To_212Report.java`, file tree. https://github.com/METS-Programme/openmrs-module-ugandaemr-reports
20. WHO, SMART Immunizations IG 1.0.0: https://smart.who.int/immunizations/v1.0.0/index.html ; `package-list.json` https://smart.who.int/immunizations/package-list.json
21. `WorldHealthOrganization/smart-immunizations`, commit `bd7fa72` (2026-10-01): `LICENSE.md`, `sushi-config.yaml`, `dak.json`, `input/pagecontent/business-processes.md`, `input/L2/*.xlsx`, `input/decision-logic/*.xlsx`, `input/fsh/plandefinitions/IMMZD2DTMalaria.fsh`, `IMMZD18SMalaria.fsh`, `IMMZD5DTMalariaContraindications.fsh`, `input/fsh/activitydefinitions/*`, `input/fsh/profiles/*`, `input/fsh/rulesets/rulesets-plandefinition.fsh`, `input/cql/IMMZD2DTMalariaLogic.cql`. https://github.com/WorldHealthOrganization/smart-immunizations
22. HL7 FHIR R5: https://hl7.org/fhir/R5/immunization.html , https://hl7.org/fhir/R5/immunizationrecommendation.html , https://hl7.org/fhir/R5/immunizationevaluation.html
23. NIRA birth registration and NIN: The Independent, "Why you should disrupt NIRA offices for your child's immediate birth and NIN registration", 2026 (secondary) https://www.independent.co.ug/why-you-should-disrupt-nira-offices-for-your-childs-immediate-birth-and-nin-registration-2 ; Plan International, *Handbook for birth registration in Uganda* https://plan-international.org/uploads/sites/22/2022/02/handbook_for_birth_registration_in_uganda_pages_0.pdf ; UNICEF Uganda, Mobile VRS https://www.unicef.org/uganda/what-we-do/mobilevrs ; ICAO TRIP 2023 deck https://www.icao.int/Meetings/TRIP-Symposium-2023/Documents/Christopher%20KANTINTI.pdf ; UNECA CRVS interoperability deck https://apai-crvs.uneca.org/sites/default/files/resourceuploads/day-2-interoperability-uganda.pdf ; HEPS Uganda https://www.heps.or.ug/?p=4226 (all via search summaries)
24. Prior research notes in this repository: `research/dhis2-hmis` (HMIS forms, DHIS2 API), `research/patient-identity` (NIN coverage, NIRA interface), `research/cql-spike` (IMMZ CQL translation), `research/indicators` (IMMZ Measures).

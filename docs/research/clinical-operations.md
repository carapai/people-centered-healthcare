# Research: referrals, inpatient care, emergency care and imaging in Uganda

Findings for the research ticket [What do referrals, inpatient care, emergency care and imaging in Uganda require?](https://github.com/carapai/people-centered-healthcare/issues/623). This note reports facts and decides nothing. The decision tickets are #624, #625 and #626. Related design: [clinical-workspaces.md](../architecture/clinical-workspaces.md), [resource-ownership.md](../architecture/resource-ownership.md), [billing-and-payments.md](../architecture/billing-and-payments.md) (bed-days). Related research notes: `research/dhis2-hmis` (HMIS forms and the DHIS2 API), `research/immunization` (style and HMIS 105).

Research date: **2026-11-18**. Every source was read on that date unless a date is given. Claims marked **unverified** have no primary source yet. "Secondary" means a news item, study, partner page or third-party code that reports the fact, not the Ministry of Health (MoH) document itself. Source numbers in square brackets refer to §8.

## 1. Summary

| Topic | Key facts | Main sources |
|---|---|---|
| Tiers | HC I (VHT, village), HC II (parish, 5,000 people, 2 beds for emergency delivery), HC III (sub-county, 20,000, 14 beds), HC IV (county, 100,000, 24 beds, theatre, obstetric ultrasound, blood), general hospital (district, 500,000, 100–250 beds), regional referral hospital (RRH, region, 2 M, 500 beds), national referral hospital (NRH, 10 M, 600+ beds). The HC IV is "the first point of entry for referrals" from lower units. | MoH Service Standards 2016 [2] |
| Referral rules | No standalone current MoH referral guideline was found. The 2016 standards require a standardized referral form, referral to a named provider, communication to the receiving unit, recording in registers, and a feedback mechanism. The EMS strategic plan cites "National guidelines for case referral and inter-facility transfers", not found online. | [2], [5] |
| Referral form | **HMIS 032 Referral Note** (2010): top half filled by the sender (history, investigations, diagnosis, treatment, reason), bottom half by the receiver (arrival and discharge dates, findings, diagnosis, treatment, treatment to continue, remarks). The original goes with the patient and should come back. The sender keeps a copy or a list and monitors the return rate. | HMIS manual 2010 [1] |
| Referral counting | Counts only. OPD: "referrals to unit" and "referrals from unit" in HMIS 105 (today 105:01 rows OR01 and OR02, by age band and sex). Inpatient: HMIS 108 item 2, inpatients referred from and to the unit, counted from the inpatient register. ANC and maternity have their own referral rows. | [1], [10] (secondary) |
| E-referral | No national e-referral standard was found. MoH says an "E-referral system" reached all RRHs after a Kiruddu pilot, with mixed use (news, July 2025). Kampala has a call and dispatch centre and the KDETS app linking ambulances and 72 facilities (USAID MaNe project). | [12], [13], [14] (secondary) |
| Ambulance (912) | The EMS Policy dates from 2021. The 912 short code was allocated by UCC but was not live in November 2025 ("will introduce… by the end of this year"). Call and dispatch centres run at Naguru NRH and Masaka RRH; four more were under construction. 911/999 are police lines. Whether 912 is live now is **unverified**. | MoH SP II [4]; EMS strategic plan [5]; news [15], [16] |
| Inpatient forms | HMIS 051 inpatient treatment sheet, HMIS 034 inpatient treatment follow-up form (vitals and a dose-by-dose tick grid), HMIS 052 inpatient discharge form, HMIS 054 inpatient register, HMIS 091b inpatient tally sheet, daily census (Table 6a). | [1] |
| HMIS 108 | Monthly, due on the 7th. Item 1 census by ward: beds, admissions, deaths, patient days, ALOS, average occupancy, bed occupancy. Item 2 referrals. Items 3–5 surgery and blood. Item 6 admissions and deaths by diagnosis, by under 5 / 5+ and sex. The live DHIS2 data set is "HMIS 108 - IPD Monthly Report" with census by 20 ward types. | [1]; live form via [10] (secondary) |
| Bed measures | ALOS = patient days / admissions. Average occupancy = patient days / days in month. Bed occupancy = average occupancy × 100 / beds. Patient-day rules: count the admission day, not the discharge day, and count a same-day stay as one day. | [1] |
| Triage | Three tools appear. **UTAT** (Uganda Triage and Treatment Algorithm, MoH and CDC): red within 10 minutes, yellow within 1 hour, green after. **IITT** (WHO, ICRC, MSF): red, yellow, green; one study says MoH adopted it in 2021 (**unverified**). **ETAT** (WHO, children): emergency, priority, queue. No MoH triage document was found online. | CDC [7]; WHO [8]; MoH EMS assessment 2024 [6] |
| Emergency data | Paper emergency register, summarized monthly into HMIS 105. Forms HMIS EMS 002 (ambulance) and HMIS EMS 003 (emergency unit) exist but 74–81% of facilities lacked them. The live 105:01 has an Emergency Medical Services section (ES01–ES09: cases by type, haemorrhage, road traffic injuries by mode, deaths). | MoH EMS assessment 2024 [6]; [10] (secondary) |
| Imaging | Standards: HC III portable obstetric ultrasound; HC IV obstetric ultrasound; general hospital plain X-ray and barium swallow; RRH radiology and imaging; NRH CT and MRI. MoH reports 14 RRHs with CT, 58% of hospitals with working X-ray (GH 39%), and 34% of public HC IVs with working ultrasound. The paper X-ray register is HMIS 056, also used for ultrasound and CT. | [2], [4], [1] |
| Teleradiology and PACS | MoH set up a national teleradiology centre at Mulago in 2023 for CT images from RRHs. Teleradiology is mostly private; one private workflow uses HMIS → RIS → PACS/"Radweb" → report back to HMIS. About 75 radiologists in 2023, mostly in Kampala. No public PACS inventory was found. | [17], [18] (secondary) |
| Open imaging stack | Orthanc 1.13.1 (core GPLv3+, some plugins AGPLv3+). Its DICOMweb plugin implements WADO-URI, WADO-RS, QIDO-RS and STOW-RS as server and client. | [21] |
| FHIR R5 | Encounter maturity 4 (`admission` backbone, `location[]` with period and `form`, `partOf`); EncounterHistory 0; EpisodeOfCare 2; ServiceRequest 4; Task 3; ImagingStudy 4; ImagingSelection 1. All trial use. IHE 360X is HL7 v2 + C-CDA over Direct, not FHIR. IHE mCSD 4.0.0 is FHIR R4. | [19], [20], [22] |

## 2. Referrals

### 2.1 Tiers and roles

The *Health Sector Service Standards and Service Delivery Standards* (MoH, 2016), Table 1 [2]:

| Level | Admin level | Population | Beds | Services named |
|---|---|---|---|---|
| HC I (VHT) | Village | 1,000 | 0 | Community preventive and promotive |
| HC II | Parish | 5,000 | 2 (emergency delivery) | Outpatient, curative, emergency delivery |
| HC III | Sub-county | 20,000 | 14 (4 maternity, 4 children, 4 female, 2 male) | Adds maternity, inpatient, laboratory |
| HC IV | County | 100,000 | 24 (8 maternity, 6 children, 6 female, 4 male) | Adds obstetric ultrasound, emergency and simple surgery including caesarean section, blood transfusion, mortuary |
| General hospital | District | 500,000 | 100–250 | Adds general medicine and surgery, specialists in medicine, surgery, paediatrics, O&G |
| RRH | Region | 2,000,000 | 500 | Adds psychiatry, ENT, ophthalmology, dentistry, intensive care, radiology, pathology |
| NRH | National | 10,000,000 | 600 minimum | Super-specialist care, CT and MRI |

Other statements in [2]:

- The HC III is the immediate referral point for the HC IIs in its sub-county.
- The HC IV "is mainly a Primary Health Care referral facility". Patients are "assessed, diagnosed, stabilised and either treated, referred back to a lower level or referred to a higher level". It is "the first point of entry for referrals from the LLHUs and for self-referrals in case of an emergency".
- The general hospital is "primarily a referral hospital" for the HC IVs in its catchment.

### 2.2 Referral rules

No standalone, current MoH referral guideline was found online (**unverified** whether one exists). What was found:

- **Service standards [2], standard 7.2 "functional referral system":** refer when needs exceed staff competence; written protocols including pre-referral care; medicines for stabilisation; "a standardized referral form" with demographics, clinical findings, diagnosis, pre-referral treatment and reason; referral "to a named health care provider or department"; "the facility communicates the referral to the referral unit"; referrals "recorded appropriately in the registers"; "a functional mechanism for follow up / feedback"; a documented referral network; refresher training every 2 years.
- **EMS strategic plan [5]:** the EMS will act only on emergency transfers requested by a health facility. Emergency unit staff start inter-facility transfers "by the following standard protocols and National guidelines for case referral and inter-facility transfers". That guideline was not found (**unverified**). Non-emergency transport is left to the private sector (Type A ambulances).
- **MoH Strategic Plan II 2025/26–2029/30 [4]** lists "Develop standards and guidelines for EMS & referral" as a planned output. It reports "the use of social media to coordinate emergency referrals" as an innovation.
- **2010 HMIS manual [1]:** in facilities that charge user fees, referred patients "should be treated at reduced rates".

### 2.3 The referral note (HMIS 032)

From the *HMIS Health Unit Procedure Manual*, August 2010, Technical Module 2 Part 4 [1]:

- **Use:** outpatient, inpatient, family planning, ANC and maternity referrals "to a higher-level health Centre".
- **Copies:** two or three. The original goes with the patient and "should be returned… to the health unit after treatment at the referred unit". The receiving hospital may keep a second copy. The sending unit may keep a third.
- **Sender section:** date of referral; to; from (health unit); referral number; patient name and number; age; sex; date of first visit; history and symptoms; investigations done; diagnosis; treatment given; reason for referral; clinician name, phone and signature.
- **Receiver section:** date of arrival; date of discharge; further investigations; diagnosis; treatment given; treatment or surveillance to be continued; remarks (for example whether the diagnosis was accurate and the referral justified); clinician name, phone and signature.
- **Tracking:** returned notes are kept in a special file and discussed at staff meetings. The sender keeps copies or a list (referral number, date, patient number, reason, referred to, date returned, remarks) and "should monitor the rate of return of referred patients". If patients do not return the notes, the district team can collect them during supervision.
- **Registers:** the OPD register (HMIS 031) has "REF IN NUM" and "REF OUT NUM" columns that carry the referral note number. The inpatient register (HMIS 054) has "REF IN?" and "REFERRED FROM" columns and a final status "R" for referred out.

Whether HMIS 032 is still the current number and layout is **unverified**. The 2024 MoH emergency care assessment [6] found that 80.7% of facilities did not have the emergency unit form "or a referral register". A 2023 study of Kampala says referral mechanisms "lack referral forms and functional feedback loops" [13] (secondary).

### 2.4 How referrals are counted in HMIS

- **HMIS 105 (OPD):** section 1.2 "Outpatient referrals": "Referrals to unit" and "Referrals from unit" [1]. In the live 105:01 these are data elements **OR01** and **OR02**, by the OPD age bands (0–28 days, 29 days–4 years, 5–9, 10–19, 20+) and sex [10] (secondary: third-party code that reads the live form). ANC and maternity sections carry their own "referrals to unit" and "referrals from unit" rows [1].
- **HMIS 108 (inpatient):** item 2 "Referrals": "Number of Inpatients referred from this health unit" and "Number of Inpatients referred to the health unit", tallied from the inpatient register [1].
- **No case-level data.** HMIS carries counts only. There is no field for the receiving facility, reason or outcome. The feedback loop lives on paper (the returned note).
- **Data quality.** At Jinja RRH, eHMIS reported 180–206 OPD referrals in a month in January–March 2026, while the EMR held 88 referral visits for the quarter [10] (secondary).

### 2.5 E-referral systems

| System | What it is | Status | Source |
|---|---|---|---|
| MoH "E-referral system" | Electronic referral linked to the hospital EMRs ("Electronic Patient Management System") | MoH says it was piloted at Kiruddu NRH and rolled out to all RRHs. Hospitals report a "rough and bumpy start", paper in parallel, and that Arua's national referral component "has not functioned". No hospital could share results with another. Builder and standard not named. | Plus News, 28 July 2025 [12] (secondary) |
| KDETS (Kampala Digital Emergency Transport System) | Mobile app for inter-facility maternal and newborn referral; shows ambulances in real time and notifies the receiving facility | Built by the USAID-funded MaNe project (PSI and KCCA). The Kampala call and dispatch centre took 10,183 calls from April 2020 to June 2021, 25% maternal and newborn. PSI (undated) reports 25 ambulances and 72 facilities linked. | GHSP June 2023 [13]; PSI page (secondary) |
| Obulamu Mama | SMS referral notices from community providers to facilities, with acknowledgement | Framework paper, field status **unverified** | PMC article (secondary) |
| Pre-referral phone calls | Preloaded phones for HCs to call Mbarara RRH before sending a woman in labour | Randomised pilot | Study (secondary) |
| ClinicMaster ExternalReferrals | Referral-out form inside a hospital EMR | At Jinja RRH it held at most four forms a month in 2026 | [10] (secondary) |

No national e-referral data standard, message format or FHIR profile was found (**unverified**).

### 2.6 Ambulance and dispatch

- **Policy.** The National Emergency Medical Services Policy dates from 2021 [6]. The *National EMS Strategic Plan 2018/19–2024/25* (later edition 2020/21–2024/25) operationalises it [5].
- **Design in the plan [5]:** one call and dispatch centre per region (14) using a "cloud-based Computer Aided Dispatch (CAD) platform" with vehicle locators, GPS and GIS; a national coordination centre; an ambulance-to-hospital radio link for pre-arrival notice; a tiered response that sends first responders from the nearest facility first. Pre-hospital triage decides the destination facility.
- **Number.** The plan says UCC allocated an emergency number but it was "redundant… due to lack of an established medical call and dispatch system". The plan's two editions name it as 911 and the 2024 MoH assessment as 912 [5], [6]. In November 2024 MoH said it had "secured" 912 through UCC [16]. On 25 November 2025 the Director General said MoH "will introduce a short code – 912 – by the end of this year" [15]. Police run 999 and 911/112 lines [15], [16]. Whether 912 is live today is **unverified**.
- **Centres.** "2 call and dispatch centers and ambulance hubs have been established in Naguru NRH and Masaka RRH." Four more (Arua, Lira, Mbale, Mbarara) were under construction from FY 2022/23 [4]. KCCA runs a Kampala centre [13].
- **Fleet.** 276 public Type B ambulances and 11 Type C in November 2024 [16] (secondary). 68 ambulances procured in the last plan period [4]. SP II plans 15 ambulances a year and five regional call and dispatch centres [4].
- **Planned EMS indicators [5]:** activation time (call to mobilisation), travel time (dispatch to scene), response time (call to arrival), case holding time (scene time); median time from arrival to first vitals and to first provider care; deaths in the emergency unit within 2, 8 and 24 hours. The plan says EMS facility indicators "have already been integrated in the national HMIS tools" and that a core EMS data set will be reported through DHIS2.

## 3. Inpatient care

### 3.1 Paper forms (HMIS manual 2010, Technical Module 5)

| Form | Purpose | Key fields |
|---|---|---|
| **HMIS 051 Inpatient treatment sheet** | Monitor treatment and condition during the stay. Kept by the ward in-charge. | Ward, bed number, name, inpatient number, address, age, sex, next of kin, admission date and time, referred from, discharge date, status of discharge, final diagnoses, surgical procedures and special services, follow-up date and place, treatment instructions after discharge. Lower half: provisional diagnosis, clinical notes, laboratory and X-ray findings. |
| **HMIS 034 Inpatient treatment follow-up form** | "Surveillance notes" for nurses and clinicians | Patient name, IP number, ward, bed; a temperature chart; a grid where the clinician writes each instruction (e.g. "BP 2 x 1 day x 4d", "Inj. Quinine 600mg tds") and the nurse writes "√" or "X" under each date as it is given, and "//" when finished. This is the national form closest to a medication administration record and observation chart. |
| **HMIS 052 Inpatient discharge form** | Kept by the patient, "Reference information for patient to retain about his/her hospital stay" | Ward, IP number, name, age, sex, address, admission and discharge dates, status on discharge, provisional and final diagnosis, clinical presentation, summary of interventions (investigations, treatment, surgery), recommendation on discharge, discharging officer, follow-up date and place. The patient brings it to later visits. |
| **HMIS 054 Inpatient register** | Admission and discharge record. Registration is central so each patient gets a unique IP number. | IPD number (restarts at 1 each July), name, village, parish, age, sex, next of kin, ref in?, referred from, date in, date out, provisional diagnosis, diagnosis at discharge, final status (D discharged, DD died, T transferred to another ward, R referred out, S self-discharge or runaway), remarks (cause of injury). |
| **HMIS 091b Inpatient tally sheet** | Tally diagnoses from the register daily | — |
| **Table 6a Daily inpatient census** | Per ward per day, from ward report books | Patients the previous day, admissions, discharges, deaths, runaways, beds occupied today = (1)+(2)−(3)−(4)−(5). A day begins at midnight. |
| **HMIS 056 X-ray register**, **HMIS 057 Operating theatre register**, blood transfusion record | Department registers feeding HMIS 108 items 3–5 | — |

Other rules [1]:

- At night or in an emergency, a treatment sheet can be issued without an IP number. Registration follows the next morning.
- On readmission the discharge form is used to find the old file. The old file is attached to the new one under the new IP number.
- Diagnoses are tallied from the register daily, notifiable ones reported weekly in HMIS 033b.

No MoH standard for fluid balance charts, early warning scores, nursing care plans or a separate MAR was found (**unverified**). Studies describe nursing documentation as paper-based with "few standards" (Mulago case study) and fluid balance charts as often missing or wrongly filled (Bushenyi study) (secondary).

### 3.2 HMIS 108 inpatient monthly report

From the 2010 manual [1]:

- **Who and when:** every unit with inpatients; due on the 7th of the following month; copies to the HSD and DHO; hospitals also send one to the MoH Resource Centre.
- **Item 1, census by ward:** (A) ward by function, not number (paediatric, maternity, male/female surgical, …); (B) number of beds; (C) admissions; (D) deaths; (E) patient days; (F) average length of stay = E / C; (G) average occupancy = E / days in month; (H) bed occupancy = G × 100 / B. Hospital totals must be recomputed from the column totals, not summed.
- **Item 2, referrals:** inpatients referred from and to the unit.
- **Items 3 and 4:** major procedures (caesarean section, laparotomy, tracheostomy, evacuation, internal fixation, burr hole, thoracotomy, ENT, other) and minor procedures (oral surgery, herniorrhaphy, debridement, incision and drainage, plastic, ocular, minor ENT, safe male circumcision, other).
- **Item 5:** blood transfusions (units).
- **Item 6:** admissions ("cases") and deaths by diagnosis, split under 5 / 5 and over and male / female, for 114 listed diagnoses (notifiable diseases, other infections, maternal and perinatal, non-communicable diseases, NTDs, medical emergencies such as cardiac arrest, GI bleeding, respiratory distress, acute renal failure, acute sepsis).
- **Item 7:** comments.

The live DHIS2 version (from third-party code that reads `hmis.health.go.ug`, secondary [10]):

- Data set "HMIS 108 - IPD Monthly Report", UID `EBqVAQRmiPm`, monthly.
- Census data elements CI01 beds, CI02 admissions, CI03 deaths, CI04 patient days, CI05 average length of stay, disaggregated by a "Ward Type" category with 20 options: male medical, female medical, male surgical, female surgical, paediatrics, maternity/obstetric, gynaecology, emergency ward, ICU, neonatal unit, TB, psychiatric, eye, ENT, orthopaedic, nutrition, palliative, rehabilitation ward, acute care unit, other wards.
- Diagnoses use "Age(0-4, 5+Yrs) & Sex"; some maternal items use 10–14, 15–19, 20–24, 25–49 and 50+; neonatal items use 0–7 and 8–28 days.
- The compiler computes patient days per discharge as (discharge − admission) with a minimum of one day. This matches the manual's rule only for discharged patients; the manual counts patients still on the ward from the daily census.

The full current 108 layout, its revision year and its diagnosis list (ICD-11 or the 2010 list) are **unverified** from a primary source.

### 3.3 Bed and stay measures

From the 2010 manual [1]:

- **Patient days:** "The day the patient is admitted is a patient day." A day counts if the patient is there at the counting time on two consecutive days. "The day a patient is discharged is not counted." A transfer day is not counted for the old ward. A same-day admission and discharge counts as one day. The monthly patient days equal the sum of the daily census counts.
- **ALOS** = patient days / admissions. **Average daily occupancy** = patient days / days in month. **Bed occupancy (%)** = average daily occupancy × 100 / beds.
- Designated bed numbers per level come from the 2016 standards (§2.1).

### 3.4 Discharge summary contents

The national discharge summary is HMIS 052 (§3.1). Its fields: ward, IP number, identity, admission and discharge dates, status on discharge, provisional and final diagnosis, clinical presentation, interventions (investigations, treatment, surgery), recommendation, discharging officer, follow-up date and place. The referral note's bottom half (HMIS 032) is the discharge feedback to a referring unit (§2.3). No newer MoH discharge summary template was found (**unverified**).

### 3.5 Ward rounds

No MoH document on ward round documentation was found. The 2010 manual puts clinical notes on HMIS 051 and orders and their administration on HMIS 034. Hospital EMRs record ward rounds as notes; at Jinja RRH, ClinicMaster notes include "the admission note, the first ward round" [10] (secondary). Ward round practice is **unverified**.

## 4. Emergency care

### 4.1 Policy and plans

- **National EMS Policy, 2021.** Launched by the Minister of Health; costed at UGX 608 billion over five years (news, secondary). The policy text was not found online (**unverified** in full).
- **National EMS Strategic Plan** (2018/19–2024/25; a 2020/21–2024/25 edition is overlaid in the same PDF) [5]: emergency units in 193 HC IVs with "acuity-based triage"; emergency departments in general hospitals and RRHs; "All health facilities will have a triage area… based on national guidelines for Emergency medical departments/units"; patients needing life- or limb-saving treatment "shall be seen immediately upon arrival".
- **Service standard 7.3 [2]:** an emergency room or unit; trained staff; displayed guidelines; equipment; a secure emergency drug cupboard (adrenaline, prednisone, aminophylline, diazepam, 50% dextrose); 24/7 cover.
- **MoH SP II [4]:** the EMS policy and strategic plan were disseminated; call and dispatch centres as in §2.6.

### 4.2 Triage systems

| Tool | Owner | Categories and timings | Status in Uganda |
|---|---|---|---|
| **UTAT** (Uganda Triage and Treatment Algorithm) | MoH with CDC and partners; adult and child versions; printed as posters | Red: treat within 10 minutes. Yellow: within 1 hour. Green: after critical patients. | CDC-supported first training; used at Mulago. MoH's 2024 assessment calls its introduction "a milestone" [6], [7]. The MoH UTAT document itself was not found (**unverified**: exact criteria). |
| **IITT** (Interagency Integrated Triage Tool) | WHO, ICRC, MSF (2020); routine ≥12 years, <12 years, mass-casualty versions | Red: "high acuity; need to be seen immediately". Yellow: "moderate acuity; need to be seen soon". Green: "low acuity; can wait". WHO states no minute targets. | A 2025 study says MoH adopted it in 2021, citing only the WHO page; one rural hospital used an IITT checklist with coloured wristbands [8], [9]. Adoption is **unverified**. |
| **ETAT / ETAT+** (WHO) | WHO, children | Emergency signs (ABCD: airway, breathing, circulation, coma, convulsion, dehydration), priority signs ("3TPR MOB"), queue (non-urgent) | Training is widespread; the 2024 MoH assessment reports ETAT training rates by level [6]. |
| **SATS** (South African Triage Scale) | Emergency Medicine Society of South Africa | Not checked | Use at some Ugandan hospitals is reported in studies (**unverified**). IITT's discriminators "closely resemble" SATS [9]. |

How UTAT and IITT relate is **unverified**. Both use the red/yellow/green traffic-light scheme from ETAT.

### 4.3 Emergency register and reporting

From the MoH *Emergency Care Systems in Uganda* assessment, December 2024 [6]:

- "More than half of the emergency departments primarily used the manual emergency register within the Health Management Information System (HMIS) to capture daily data on emergency cases. This data was aggregated and submitted monthly in HMIS 105 reports."
- Emergency registers were present at 87.5% of RRHs, 43.5% of general hospitals, 30.4% of HC IVs and 0% of NRHs ("transitioning to electronic systems").
- 73.7% of facilities lacked the ambulance form **HMIS EMS 002** and 80.7% lacked the emergency unit form **HMIS EMS 003** or a referral register.
- Facilities called the "EMIS" (Emergency Management Information System) forms outdated and said their eHMIS 105 lacked an emergency section. 45.6% had a data system and internet.
- The assessment counted as an emergency "Any case documented in the emergency register".

The live HMIS 105:01 has section 1.3.25 "Emergency Medical Services" [10] (secondary, read from the live form on 2 November 2026):

- **ES01** emergency cases: medical, obstetrics/gynaecology, surgical, road traffic injuries;
- **ES07** external haemorrhages: total, controlled;
- **ES08** road traffic injuries: motor vehicle, motor cycle, bicycle, pedestrian, other;
- **ES09** deaths at the emergency unit: medical, obstetrics/gynaecology, surgical, road traffic injuries;
- all by OPD age bands and sex.

The emergency register's columns and the content of HMIS EMS 002 and 003 were not found (**unverified**). HMIS 108 item 6 has a "Medical emergencies" diagnosis group, and the live 108 census has an "Emergency Ward" ward type (§3.2).

## 5. Imaging

### 5.1 Modalities by level

From the 2016 service standards [2]:

| Level | Imaging named |
|---|---|
| HC III | "Basic imaging services (Portable ultrasound for obstetric investigations)" |
| HC IV | "Basic imaging services (ultrasound for obstetric investigations)" |
| General hospital | "Radiological investigations: Plain radiographs, Barium swallow"; imaging for obstetric and other abdominal investigations |
| RRH | "Radiological services"; "Imaging services"; a radiology building |
| NRH | "Basic and advanced imaging services"; "MRI and CT Scan"; nuclear medicine |

What exists, from MoH Strategic Plan II [4] (performance in the previous plan period):

- "All targeted 14 RRHs (100%), have been equipped with CT scanners." Kayunga and Yumbe RRHs were to get theirs in FY 2023/24. In May 2023 most new RRH CT scanners were idle for lack of radiologists or Atomic Energy Council validation (news, secondary [18]).
- "An average of 58 % of Hospitals… have functional X-Rays": NRH 5/5, national specialised hospitals 5/5, RRH 15/16, general hospitals 20/51 (39%). Philips X-rays had gone unmaintained for 3 years, and some non-digital Philips units were declared obsolete. MoH planned 20 new X-ray machines.
- "Only 34% (62 out of the 181 Public HC IVs)… have functional ultrasound machines", with a shortage of sonographers.
- SP II plans: X-ray and ultrasound procurement, "2CT at RRH, 10CT at GH) 3MRI at RRH", mobile X-rays per region, portable X-rays with CAD4TB for TB case finding, and "Functionalize teleradiology services at all regional referral hospital and GH (internet and renumeration)".

### 5.2 Requests, registers and reporting

- **Standards 7.42–7.49 [2]:** services licensed by the Atomic Energy Council; emergency imaging after hours; a named referral centre when capacity is missing; a patient register; X-rays only on "a signed request form [from] a qualified medical practitioner"; request forms carry name, examination, relevant previous examinations and clinical information; results "reported on by a qualified person within a time frame that meets clinical needs"; reports have "a clear conclusion"; a copy of the report is filed in the patient record; films available at each visit; film storage per guidelines.
- **HMIS 056 X-ray register [1]:** serial number (restarts monthly), name, patient number (IP or OPD), age, sex, from (OPD or ward), investigation (region), counts of film used by size. "The same type of register can be used for Ultra Sound and CT scan investigation." Monthly report: number of patients X-rayed.
- **HMIS counting:** the 2010 Table 7 counts "X-Rays taken". Whether the live 105 or 108 carries imaging counts is **unverified**.
- **Workforce:** about 75 registered radiologists in 2023, mostly in Kampala [17]. RRHs meant to have 27 radiographers average five (news, 2026, secondary).

### 5.3 Teleradiology, PACS and DICOM

- **National teleradiology centre.** "In 2023, Uganda's Ministry of Health established a national teleradiology center at Mulago National Referral Hospital to facilitate image sharing from CT scanners installed at regional referral hospitals" [17]. MoH's spokesman: "We have created a center where they will be sending images to radiologists at Mulago Hospital for interpretation" (May 2023) [18]. The software, PACS and network used are **unverified**.
- **Practice (qualitative study, 17 providers, mostly private) [17]:** teleradiology is "largely restricted to isolated private initiatives and pilot projects"; mostly CT and MRI, with X-ray, ultrasound, fluoroscopy and mammography sent out on holidays. One described workflow: registration and request in the hospital information system, payment confirmed, demographics and request "crossed into RIS", images uploaded "to platforms such as PACS or Radweb", remote reporting, report back into the HMIS. Some private facilities send images to radiologists abroad "without clear accountability mechanisms". Radiologists phone provisional reports for urgent cases. WhatsApp image sharing is reported.
- **Barriers:** connectivity, servers, staff remuneration [17], [4].
- **PACS inventory.** No list of facilities running PACS or DICOM modalities was found (**unverified**). The non-digital X-rays and film-size registers [1], [4] show that much plain radiography is still film.

### 5.4 Integratable open-source options

- **Orthanc** (University Hospital of Liège, now UCLouvain): latest core on the source server is 1.13.1. Core licence GPLv3+. Plugins for scalability, cloud and viewers (PostgreSQL, web viewers, Stone) are AGPLv3+ [21]. The DICOMweb plugin "simultaneously turns Orthanc into a DICOMweb server and into a DICOMweb client" and provides "a basic, reference implementation of WADO-URI, WADO-RS, QIDO-RS and STOW-RS"; "The full standard is not implemented yet" [21]. The plugin list includes an OHIF plugin and a Worklists plugin. The DICOMweb plugin's own licence was not stated on the page (**unverified**).
- **DICOMweb and FHIR.** FHIR `ImagingStudy` holds study, series and instance UIDs and `endpoint` references; the images stay on a DICOM or WADO-RS server [19].
- Other PACS (dcm4chee, commercial Fuji, Radweb) were not evaluated.

## 6. FHIR R5 fit

From the HL7 FHIR R5 (5.0.0) pages and profiles [19]. All are trial use.

| Resource | Maturity | Facts that bear on this ticket |
|---|---|---|
| `Encounter` | 4 | `class` 0..* (ambulatory, emergency, inpatient, virtual…); `admission` backbone (renamed from R4 `hospitalization`): `preAdmissionIdentifier`, `origin` (Location or Organization), `admitSource`, `reAdmission`, `destination`, `dischargeDisposition`; `location[]` with `location` 1..1, `status`, `form` (renamed from `physicalType`) and `period`, which gives a ward and bed history; `partOf` for sub-encounters; `episodeOfCare`; `basedOn` (CarePlan, DeviceRequest, MedicationRequest, ServiceRequest); `diagnosis[]` with `condition` and `use`; `actualPeriod`, `length`. HL7: "admission/discharge related information is kept in a separate admission component", and the level of aggregation (one Encounter per admission or per visit) varies by jurisdiction. |
| `EncounterHistory` | 0 | New in R5. Replaces R4 `Encounter.statusHistory` and `classHistory`. |
| `Location` | — | `form` (bed, room, ward…), `operationalStatus` bound to v2-0116 bed status ("typically only for a bed/room"), `partOf`. |
| `EpisodeOfCare` | 2 | `status` with `statusHistory`; `type`; `reason`; `diagnosis`; `patient`; `managingOrganization`; `period`; `referralRequest` (ServiceRequest); `careManager`; `careTeam`; `account`. HL7: "When an incoming referral is received a new EpisodeOfCare may be created for this organization." |
| `ServiceRequest` | 4 | HL7 names referral and transfer of care as a use. `status`, `intent`, `category`, `priority`, `code`, `subject`, `encounter`, `requester`, `performer` (incl. Organization, HealthcareService), `location`, `reason`, `supportingInfo`, `basedOn`, `replaces`, `requisition` (shared identifier). |
| `Task` | 3 | "A Task could request fulfillment of a ServiceRequest". `status`, `businessStatus`, `intent`, `focus`, `for`, `owner`, `requester`, `requestedPerformer`, `restriction`, `input`, `output`, `executionPeriod`. HL7 lists "admitting a patient" as a Task example. |
| `ImagingStudy` | 4 | "SHALL reference one DICOM Study". `modality`, `series[]` (`uid`, `modality`, `bodySite`, `laterality`, `instance[]`), `basedOn` (ServiceRequest, Task…), `referrer`, `endpoint`, `encounter`, `numberOfSeries`, `numberOfInstances`. "The DICOM instances are not stored in the ImagingStudy resource; use of a DICOM WADO-RS server or other storage mechanism is needed." |
| `ImagingSelection` | 1 | New in R5. A selection of instances or frames within one study and series (key images, regions); `code` 1..1, `studyUid`, `seriesUid`, `instance[]` with `imageRegion2D/3D`, `derivedFrom` ImagingStudy. |

IHE profiles:

- **IHE PCC 360X Closed-Loop Referral** [20]: Rev. 1.2, trial implementation, 14 April 2021. Actors: Referral Initiator and Referral Recipient. Transactions PCC-55 to PCC-60 (request, decline, outcome, cancellation, interim consultation note, appointment notification; accept is also defined). Content is HL7 v2 (`OMG^O19` for the referral order, `OSU^O51` for status) plus C-CDA documents, carried in XDM packages over Direct messaging. The supplement does not use FHIR. A FHIR edition was not found (**unverified**).
- **IHE ITI mCSD (Mobile Care Services Discovery)** [22]: 4.0.0, "Active as of 2025-05-21", based on FHIR 4.0.1. Transactions: Find Matching Care Services [ITI-90], Request Care Services Updates [ITI-91], Care Services Feed [ITI-130]. The page header says trial implementation while the body says publication. Whether mCSD models beds and wards was not checked (**unverified**).

## 7. Not verified

- Whether MoH has a current standalone referral guideline, and the "National guidelines for case referral and inter-facility transfers" cited by the EMS plan.
- Whether HMIS 032 is still the referral note number and layout. The current layouts of HMIS 051, 034, 052 and 054.
- The builder, data standard and current use of the MoH "E-referral system" in RRHs.
- Whether 912 is live in November 2026, and how many dispatch centres now run.
- The full text of the National EMS Policy 2021.
- The MoH UTAT document and its criteria; whether MoH adopted IITT in 2021; how UTAT and IITT relate; SATS use in Uganda.
- The columns of the emergency register and of forms HMIS EMS 002 and 003.
- The current HMIS 108 layout from a primary source: revision year, diagnosis list, whether diagnoses are now ICD-11.
- Whether live HMIS 105 or 108 counts imaging examinations.
- National nursing chart standards (fluid balance, early warning, MAR).
- Which software and network the Mulago teleradiology centre uses; which facilities run PACS or DICOM modalities.
- The licence of Orthanc's DICOMweb plugin.
- A FHIR edition of IHE 360X; whether mCSD models beds.
- The live DHIS2 facts in this note come from third-party code [10], not from MoH.

## 8. Sources (read 2026-11-18 unless dated)

1. Uganda MoH, *The Health Management Information System, Volume 1: Health Unit Procedure Manual*, August 2010: Technical Module 2 Part 4 (HMIS 032), Module 5 (HMIS 051, 034, 052, 054, 091b, Table 6/6a/6b, HMIS 056, 057, Table 7), Module 7 (HMIS 105 §1.2, HMIS 108). https://www.gou.go.ug/sites/default/files/media-files/THE%20HEALTH%20MANAGEMENT%20INFORMATION%20SYSTEM_0.pdf
2. Uganda MoH, *Health Sector Service Standards and Service Delivery Standards*, 2016: Table 1, §§38, 55, 74, 88, 103, standards 1.1.5, 7.2, 7.3, 7.42–7.49. https://library.health.go.ug/sites/default/files/resources/Health%20Sector%20Service%20Standards%20&%20Service%20Delivery%20Standards_2016.pdf
3. Uganda MoH, *Primary Health Care Grants guidelines* (HC IV as referral entry point; via search summary). https://library.health.go.ug/file-download/download/public/647
4. Uganda MoH, *Strategic Plan II 2025/26–2029/30*: EMS progress, CT/X-ray/ultrasound functionality, results framework (teleradiology, imaging, EMS outputs). https://guluhospital.net/wp-content/uploads/2025/12/30.pdf
5. Uganda MoH, *National Emergency Medical Services Strategic Plan 2018/19–2024/25* (February 2019; 2020/21–2024/25 edition, September 2020, overlaid in the same file): §2.4, strategies 2.3, 2.4, 3.1, §6 indicators and data flow, implementation matrix. https://guluhospital.net/wp-content/uploads/2022/12/NATIONAL-EMERGENCY-MEDICAL-SERVICES-STRATEGIC-PLAN.pdf
6. Uganda MoH, *Emergency Care Systems in Uganda: A Comprehensive Facility-Based Assessment for Emergency, Critical and Operative Care*, December 2024: §3.5 data systems, UTAT, 912. https://seedglobalhealth.org/wp-content/uploads/2025/03/Final-Report-ASSESSMENT-OF-EMS.pdf
7. CDC, "Uganda Improves Emergency Medical Services" (UTAT), 4 January 2023 (archived). https://archive.cdc.gov/www_cdc_gov/global-health/impact/Uganda-improves-emergency-medical-services.html
8. WHO, *Interagency Integrated Triage Tool*. https://www.who.int/tools/triage ; Rovati L. et al., "Effectiveness of a structured triage system…", *Int J Emerg Med* 2025;18:190 (secondary for the 2021 adoption claim). https://orbilu.uni.lu/bitstream/10993/66513/1/s12245-025-01005-z.pdf
9. Mitchell R. et al., "Triage systems in low-resource emergency care settings", *Bull WHO* 2025;103(3):204–212. https://pmc.ncbi.nlm.nih.gov/articles/PMC11865852/ ; WHO ETAT participant manual and facilitator guide (via search summary). https://www.afro.who.int/sites/default/files/2017-06/participant_manual.pdf
10. `marksemakula/hmis-report-compiler` (third-party compiler for Jinja RRH, last push 2026-11-03): `README.md`, `api/_lib/metadata.py` (data set UIDs, Ward Type and age category combos), `compiler.py`, `referrals.py` (OR01/OR02), `emergency.py` (ES01–ES09, read from the live form 2026-11-02). Secondary. https://github.com/marksemakula/hmis-report-compiler
11. Prior research note `research/dhis2-hmis` in this repository (HMIS reporting deadlines, DHIS2 API).
12. Plus News, Uganda E-referral system in regional referral hospitals, 28 July 2025 (secondary). https://plusnews.ug/?p=32362
13. "Strengthening Kampala's Urban Referral System for Maternal and Newborn Care Through Establishment of an Emergency Call and Dispatch Center", *Global Health: Science and Practice* 11(3), June 2023 (via search summary). https://pmc.ncbi.nlm.nih.gov/articles/PMC10285736/
14. Obulamu Mama framework (PMC13715344) and Mbarara pre-referral phone pilot (via search summaries, secondary). https://pmc.ncbi.nlm.nih.gov/articles/PMC13715344
15. Nile Post, "Health Ministry says over 64% of Uganda now served by ambulances", 25 November 2025 (secondary). https://nilepost.co.ug/news/305425/health-ministry-says-over-64-of-uganda-now-served-by-ambulances
16. The Independent, "Report: critical medical emergency contact still unknown to most Ugandans", 22 November 2024 (secondary). https://independent.co.ug/report-critical-medical-emergency-contact-still-unknown-to-most-ugandans
17. Eyotaru F. et al., "Current practices and implementation considerations for teleradiology services in Uganda", Research Square preprint, 29 June 2025. https://www.researchsquare.com/article/rs-6976547/v1
18. The Kampala Report, "Uganda Gov't sets up teleradiology centre at Mulago to coordinate CT scans from regional hospitals", 5 May 2023 (secondary). https://www.thekampalareport.com/latest/2023050526405/uganda-govt-sets-up-teleradiology-centre-at-mulago-to-coordinate-ct-scans-from-regional-hospitals.html
19. HL7 FHIR R5 (5.0.0): https://hl7.org/fhir/R5/encounter.html , `encounter.profile.json`, https://hl7.org/fhir/R5/encounter-version-maps.html , https://hl7.org/fhir/R5/encounterhistory.html , https://hl7.org/fhir/R5/episodeofcare.html , https://hl7.org/fhir/R5/servicerequest.html , https://hl7.org/fhir/R5/task.html , https://hl7.org/fhir/R5/imagingstudy.html , https://hl7.org/fhir/R5/imagingselection.html , `location.profile.json`
20. IHE PCC, *360 Exchange Closed Loop Referral (360X)* supplement, Rev. 1.2 Trial Implementation, 14 April 2021. https://www.ihe.net/uploadedFiles/Documents/PCC/IHE_PCC_Suppl_360X.pdf
21. Orthanc Book: licensing FAQ https://orthanc.uclouvain.be/book/faq/licensing.html ; DICOMweb plugin https://orthanc.uclouvain.be/book/plugins/dicomweb.html ; plugin list https://orthanc.uclouvain.be/book/plugins.html ; source downloads https://orthanc.uclouvain.be/downloads/sources/orthanc/index.html
22. IHE ITI, *Mobile Care Services Discovery (mCSD)* 4.0.0. https://profiles.ihe.net/ITI/mCSD/index.html
23. Nursing documentation studies (secondary, via search summaries): Mulago case study https://nru.uncst.go.ug/handle/123456789/3628 ; Bushenyi fluid balance chart study https://ir.kiu.ac.ug/items/e370e0e8-f74f-41a5-86e9-a042b8aaf749

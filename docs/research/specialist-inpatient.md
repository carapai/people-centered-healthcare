# Research: theatre, labour and delivery, blood transfusion, ICU and the mortuary in Uganda

Findings for the research ticket [What do theatre, labour and delivery, blood transfusion, ICU and the mortuary require in Uganda?](https://github.com/carapai/people-centered-healthcare/issues/684). This note reports facts and decides nothing. The decision tickets are #685, #686 and #687. It extends the research note on branch `research/clinical-operations` (`docs/research/clinical-operations.md`), which covered wards, emergency care, referrals and imaging [9]. Related decisions: [ADR 0078](../adr/0078-inpatient-stays-are-received-into-wards-and-counted-from-location-history.md) and [inpatient-and-emergency.md](../architecture/inpatient-and-emergency.md), which defer all five units covered here.

Research date: **2026-11-18**. Every source was read on that date unless a date is given. Claims marked **unverified** have no primary source yet. "Secondary" means a news item, study, partner page or third-party code that reports the fact, not the Ministry of Health (MoH), NIRA or UBTS document itself. Source numbers in square brackets refer to §10.

## 1. Summary

| Unit | Key facts | Main sources |
|---|---|---|
| Theatre register | **HMIS 057 Operating Theatre Register**, one per theatre, kept by the theatre in-charge; separate registers for major and minor theatres. Columns: date, serial number (restarts at 1 each **January**), name, patient number (IP or OPD), age, sex, surgeon, anaesthetist, anaesthesia (name and amount), diagnosis, operation, remarks. | HMIS manual 2010 [1] |
| Theatre standards | MoH standards require a documented pre-anaesthetic assessment, an anaesthetic record, an intra-operative report, post-operative diagnosis, names of the team, informed consent, recovery-room arrival and discharge times, and handover signatures. No MoH theatre booking or list standard was found. | Service standards 2016 [2] |
| Surgical Safety Checklist | One study says MoH adopted the WHO checklist in 2011 (**unverified** from MoH). Compliance in five referral hospitals was 41.7% (11.9–89.8%). Mbarara used a locally adapted version. No national MoH version of the form was found. | [10], [11] (secondary) |
| HMIS 108 items 3–4 | The 2010 form has 9 major and 9 minor procedure rows. The live DHIS2 "HMIS 108 - IPD Monthly Report" instead counts procedures by specialty (obstetric SP01–SP05, gynaecological GN01–GN06, neurosurgery NS, ENT TS, plastic PR, general OT, and others), plus imaging procedures. | [1]; live form via [7] (secondary) |
| Maternity register | In the 2010 manual the maternity register is **HMIS 072 Integrated Maternity Register**; HMIS 071 is the antenatal register. The 2017 MPDSR guidelines still cite HMIS 072. Columns cover admission, delivery, PMTCT, the baby (sex, Apgar, weight), discharge condition (mother D/DD/R; baby SB/NND/AL), who delivered. Whether the 2019 HMIS revision renumbered it is **unverified**. | [1], [3] |
| Partograph or LCG | The 2016 standards require partographs in the delivery room and monitoring of labour with the partograph. The 2010 register says a partogram is completed for every delivery, carrying the IP number. MoH is "in the process of adopting" the WHO Labour Care Guide (study, November 2025); a 2023 protocol says the new Essential Maternal and Newborn Clinical Care guidelines recommend it. The MoH guideline itself was not found. | [1], [2]; [12], [13] (secondary) |
| MPDSR | 2017 MoH guidelines. Maternal death notification **HMIS 120a**, perinatal notification **HMIS 121a**: to the facility in-charge within 24 hours, to the district and MoH within 48 hours (maternal: "within 24 hours" in §3.3.1). Facility review within **7 days** with **HMIS 120** (maternal) or **HMIS 121** (perinatal). Then cause-of-death certificate **HMIS 100** in triplicate (facility, relatives, NIRA). Weekly zero reporting through mTRAC/eIDSR; review data entered as events in DHIS2. | MPDSR guidelines 2017 [3]; MPDSR report 2022/23 [4] |
| Birth notification | Registration of Persons (Births and Deaths) Regulations 2015: the facility **administrator records every birth** in the facility, obtains the Form 3 particulars and parents' signatures, and files **weekly returns** with the registration officer. Hospitals notify through NIRA's web-based **Mobile Vital Records System (MVRS)**. | Regulations [5]; MVRS [14] (secondary) |
| Blood transfusion record | **HMIS 091 (091B) Blood Transfusion Record**, kept by the laboratory in-charge: unit number, unit group, component, received-by, patient, patient group, IP number, Hb before transfusion, ward, requesting doctor, compatibility (immediate spin, IAGT), expiry, time out, taken by, issued by, return, reason (anaemia, surgery, haemolysis). | [1] |
| HMIS 108 item 5 | 2010: blood transfusions (units) and other transfusions. Live: **UT01** units requested, **UT02** received, **UT03** transfused, **UT04** adverse blood reactions. | [1]; [7] (secondary) |
| UBTS and haemovigilance | UBTS runs 7 regional blood banks (Nakasero HQ, Arua, Fort Portal, Gulu, Kitovu, Mbale, Mbarara) and collection centres. No national haemovigilance form or reporting route was found; studies say there is "no active haemovigilance system" and suggest hospital transfusion committees revive it. | [15], [16] (secondary) |
| ICU and HDU | Standards: ICU operational policy, 1:1 nursing, individual patient records, weekly case review. HMIS 108 counts ICU and Neonatal Unit as Ward Types (beds, admissions, deaths, patient days). No separate ICU HMIS data set or ICU register form was found; the MPDSR guidelines name an "ICU register" as a data source. 11 ICUs assessed in 2024, 7 with HDUs. | [2], [3], [6]; [7] (secondary) |
| Early-warning scores | **No MoH mandate found** for NEWS, NEWS2, MEWS, PEWS or MEOWS. Ugandan studies (Mulago) validate MEWS. | [17] (secondary) |
| Mortuary | Standards 7.50: policies for preparation, storage, release and transport of bodies, and "a control system, which monitors the movement and release of bodies, including death notifications and related documents". Services by level: HC IV external examination; GH adds embalming; RRH and NRH add refrigerated storage. No national mortuary register form was found. National Mortality Surveillance Guidelines (2025) standardize "mortality registers and gate passes". | [2]; [18], [19] (secondary) |
| Death notification | Regulations 2015: the facility **administrator records every death** in the facility and files **weekly returns**; the **certificate of cause of death (Form 13)** goes from the administrator to the registration officer **within 7 days**; registration within 3 months. Form 12 notification (14 days) applies to deaths outside facilities. | [5] |

## 2. Operating theatre

### 2.1 The theatre register (HMIS 057)

From the *HMIS Health Unit Procedure Manual*, August 2010, Technical Module 5 Part 11 [1]:

- **Objective:** "Record the interventions (operations) done in the operating theatres."
- **Copies:** one for each theatre. **Responsibility:** the in-charge of the theatre.
- **Separate registers for major and minor theatres.** Whether each theatre room has its own register "is determined by the hospital".
- **Columns:**

  | # | Column | Rule |
  |---|---|---|
  | 1 | Date | Day and month |
  | 2 | Serial No. | "Each operation is given a unique number starting with number '1' on the first day of January each year" |
  | 3 | Patient name | |
  | 4 | Patient No. | The number from the inpatient or outpatient register |
  | 5 | Age | Years; months ("MTH") under 1; days under 1 month |
  | 6 | Sex | |
  | 7 | Surgeon | Name |
  | 8 | Anaesthetist | Name |
  | 9 | Anaesthesia | "Name and amount of anaesthesia used" |
  | 10 | Diagnosis | Why the operation is done |
  | 11 | Operation | Brief description |
  | 12 | Remarks | "Outcome, Nurse, etc." |

- **Reported monthly to the in-charge:** minor and major operations; sterilizations and caesarean sections; implant insertions and removals.
- **Other uses:** the HMIS 105 family planning section counts female and male sterilization and implants "From the Operating Theatre Register" [1]. The MPDSR guidelines list the theatre register as a source for finding maternal deaths [3].
- **Table 7** (the facility's annual database table) tallies theatre, X-ray and transfusion registers monthly [1].

The current layout and number of the theatre register after the 2019 HMIS revision are **unverified**.

### 2.2 Theatre documentation standards

The *Health Sector Service Standards and Service Delivery Standards* (MoH, 2016) [2]:

- **7.12 Surgical care planned and documented:** history, examination and investigations documented before surgery; test results recorded before surgery; anaesthetic assessment for drug sensitivities; "an intra-operative report and a post-operative diagnosis are documented"; "the names of the surgeon and other personnel are documented"; post-operative monitoring; evidence of informed consent in the record.
- **7.14 Staff:** a theatre duty roster for all shifts; anaesthesia only by qualified personnel; recovery nurses until full recovery; "Anaesthetist supervises the recovery period and authorizes the patient's discharge from the recovery room."
- **7.17 Guidelines:** surgical and anaesthesia guidelines and SOPs available.
- **7.18 Anaesthesia:** a pre-anaesthetic assessment before anaesthesia, documented before the start; the patient's status "continuously monitored and recorded during anaesthesia and surgery"; "Anaesthesia used is documented in the patient's anaesthetic record."
- **7.19:** "The surgical procedures are documented in the patient's file."
- **7.20 Recovery:** post-anaesthetic status monitored and documented; "Recovery area arrival and discharge times are recorded"; signatures of those handing over and receiving.
- **7.21 Sterilizing unit:** autoclave sterility tested daily and results recorded.

No national anaesthesia record form, theatre booking form or theatre list standard was found (**unverified** whether one exists). Staffing norms in the 2010 manual list anaesthetic officers, theatre assistants, anaesthetic assistants and theatre attendants by level [1].

### 2.3 WHO Surgical Safety Checklist in Uganda

- A 2023 Makerere dissertation (audit at Kawempe NRH) states that MoH adopted the WHO checklist in 2011 and found low adherence in emergency obstetric surgery [11] (secondary; the MoH adoption document was not found).
- A multicentre study in five regional referral hospitals found overall compliance of 41.7%, from 11.9% to 89.8% by hospital, and no association with outcomes [10] (secondary).
- A quality-improvement project at Mbarara RRH wrote a locally adapted checklist with a free-text section for barriers; compliance rose from a median 29.5% to 85.0% [10] (secondary).
- The 2016 standards do not name the checklist; they require surgical SOPs (7.17) [2].

No Ugandan national version of the checklist form, and no HMIS indicator counting its use, was found (**unverified**). Whether Uganda has an active National Surgical, Obstetric and Anaesthesia Plan (NSOAP) is unsettled in the sources (secondary) [10].

### 2.4 HMIS 108 items 3 and 4

**2010 form [1]:**

- Item 3, major operations: caesarean sections, laparotomy, tracheostomy, evacuations, internal fixation, burr hole, thoracotomy, ENT surgical procedures, other major.
- Item 4, minor operations: dental extractions, dental fillings, herniorrhaphy, debridement and wound care and skin grafting, incision and drainage, plastic/reconstructive, ocular, safe male circumcision, other minor.

**Live DHIS2 form** (data element names read from `hmis.health.go.ug` by third-party code [7], secondary). Procedures are grouped by specialty, each a count:

| Group | Examples |
|---|---|
| SP obstetric | SP01 caesarean sections, SP02 obstetric fistula repair, SP03 evacuations (incomplete abortion), SP04 other obstetric surgery, SP05 hysterectomy |
| GN gynaecological | Ovarian laparotomy, abdominal and vaginal hysterectomy, myomectomy, laparotomy for ectopic, other |
| NS neurosurgery | Brain surgery, burr hole, craniotomy for trauma, VP shunt, spina bifida, EVD, skull fracture elevation |
| TS ENT | Tracheostomy, adenotonsillectomy, nasal, laryngeal, otological, endoscopic, foreign bodies |
| CS cardiothoracic | Thoracotomy, CABG, valve, septal defect, chest tube, gastrostomy |
| OT general | Debridement, incision and drainage, circumcision, other laparotomy, other |
| Others | PR plastic, OM oral and maxillofacial, HS hepatobiliary, CR colorectal, UG upper GI, GI, ES endocrine, BS breast, VS vascular, PS paediatric (neonatal laparotomy, colostomy), OR orthopaedic (**unverified** contents) |

The live form also has rows for imaging procedures (CT, MRI, fluoroscopy, mammography, interventional), and many elements carry a `_2019` suffix, which suggests the 2019 HMIS revision (**unverified**). The disaggregation of these procedure counts (none, age, sex) is **unverified**.

## 3. Labour and delivery

### 3.1 The maternity register

In the 2010 manual, **HMIS 071 is the Integrated Antenatal Register** and the maternity register is **HMIS 072 Integrated Maternity Register** [1]. The 2017 MPDSR guidelines list "HMIS form 072: Integrated Maternity Register" and 072A maternity tally sheet, 073 child register, 078 integrated postnatal register [3]. Whether the 2019 HMIS revision renumbered or relaid the maternity register is **unverified**.

HMIS 072 [1]:

- **Objective:** record the admissions and deliveries. One copy stays on the maternity ward. Kept by the midwife or maternity nurse on duty.
- **Every admission is written, whether or not the woman delivers.**
- **"A PARTOGRAM CHART is completed during delivery. The IPD Number on the MATERNITY REGISTER should be entered on the PARTOGRAM CHART."**
- "All maternal and newborn deaths should be discussed at senior staff meetings."
- **Columns:**
  - Admission: date of admission; IP number ("a unique serial number given to each admission to maternity"); ANC number and referral (ANC client number and unit, "REF" if referred, "NB" if not booked); name; village and parish; phone; age; gravida/parity; diagnosis (obstetric, e.g. "transverse position", "BBA"; WHO stage or CD4 for HIV-positive mothers).
  - Delivery: mode of delivery; date; time; management of third stage (1 ergometrine, 2 oxytocin, 3 misoprostol).
  - Mother: PMTCT code for woman and partner (C, TR, TRR); vitamin A; ARVs to mother; breastfed within 1 hour.
  - Child: sex; Apgar at 1 and 5 minutes; weight (kg); ARVs to baby; BCG and polio; infant feeding option (1 exclusive breastfeeding, 2 replacement, 3 mixed).
  - Discharge: condition of mother (D alive, DD died, R ran away; transfers and referrals to be harmonized); condition of baby (SB stillbirth, NND neonatal death 0–7 days, AL alive); delivered by (name and signature); date for postnatal care; discharging officer.
- **Reported daily to the matron:** admissions, discharges, deliveries, deaths, mothers on the ward.
- **Reported monthly (HMIS 105 maternity section):** admissions; referrals to and from unit; deliveries; deliveries HIV-positive and on ARVs; live births; live births to HIV-positive mothers; birth asphyxia; low birth weight (<2.5 kg); babies given ARVs; HIV-positive mothers initiating breastfeeding within 1 hour; mothers tested; new positives; vitamin A; fresh stillbirths; macerated stillbirths; newborn deaths (0–7 days); maternal deaths; deliveries with TBA.

The live HMIS 105 maternity rows were not read for this note (**unverified**). The live HMIS 108 carries maternal conditions as cases and deaths (MC01–MC12: abortions by cause, malaria in pregnancy, hypertension, obstructed labour, haemorrhage, sepsis, fistula) and neonatal conditions in total and under 2.5 kg (ND01–ND07: neonatal admissions, birth asphyxia, jaundice, meningitis, sepsis, pneumonia, other) [7] (secondary).

### 3.2 Partograph and the WHO Labour Care Guide

- **2016 standards [2]:** "Partographs are available in the delivery room" (7.26.5); "A delivery register is available in the delivery room" (7.26.6); health workers provide "technically correct services in monitoring of labor using the partograph" (7.32.1) and in active management of the third stage (7.32.2). Up-to-date HMIS forms "(Medical forms, partographs, …)" are a supervision item (4.1.4).
- **2010 manual [1]:** a partogram is completed for every delivery and carries the maternity IP number.
- **Labour Care Guide (LCG):** a 2023 study protocol says MoH "launched the new Essential Maternal and Newborn Clinical Care guidelines" in which reproductive health experts recommended replacing the partograph with the WHO LCG [12] (secondary). A qualitative study published 26 November 2025 says MoH "is in the process of adopting the new recommended LCG to replace the long-used partograph across all health facilities" [13] (secondary). Trials of a modified LCG ran in Mbarara.

The EMNC guideline itself, its year, and whether a national LCG form or rollout order exists are **unverified**.

### 3.3 Delivery and newborn records

- The delivery and newborn data on paper are the maternity register (§3.1), the partograph, the child register (HMIS 073) and the postnatal register (HMIS 078) [3].
- Newborn identification: "Policies and procedures guide the identification of newborn babies" and "established security systems for protecting newborn babies" (standard 7.34) [2].
- Neonatal admissions to a neonatal unit are counted by HMIS 108 Ward Type "Neonatal Unit" and items ND01–ND07 [7] (secondary).
- No national newborn examination or newborn discharge form was found (**unverified**).

### 3.4 Maternal and perinatal death surveillance and response (MPDSR)

From the *Maternal and Perinatal Death Surveillance and Response Guidelines*, MoH, August 2017 [3]:

- **Forms:**

  | Form | Use |
  |---|---|
  | HMIS 120a | Maternal death notification form; also the IDSR event form for maternal deaths |
  | HMIS 120 | Maternal death review (audit) form |
  | HMIS 121a | Perinatal death notification form |
  | HMIS 121 | Newborn/perinatal death review form |
  | HMIS 100 | Medical certificate of cause of death |

- **Who and when (facility deaths):**
  - Any death of a woman of reproductive age is screened as a possible maternal death by the nurse-midwife or designated worker.
  - The nurse-midwife on duty completes HMIS 120a in **quadruplicate within 24 hours** and hands it to the ward in-charge, who notifies the facility head and the surveillance focal person.
  - Notification to HSD, DHO and MoH: "within 24 hours" (§3.3.1) and "within 48 hours" (§3.1, §3.3.2, figure 3.1). NRHs and RRHs notify MoH directly, with a copy to the host district. Scanned forms are emailed to the Division of Health Information and the Reproductive Health Division.
  - Perinatal deaths (stillbirths and deaths in the first 7 days) are notified to the facility in-charge within 24 hours and to the DHO and MoH within 48 hours.
  - The facility MPDSR committee reviews each death **within 7 days**, using HMIS 120 or 121.
  - After review, the cause of death is coded and **HMIS 100 is completed in triplicate**: one copy stays at the facility for DHIS2 entry, the original goes to the relatives, the third goes to NIRA.
- **Sources for finding deaths:** maternity, gynaecology, operating theatre, OPD, admission, discharge, **ICU**, postnatal, birth and death and **mortuary** registers.
- **Reporting:** weekly IDSR reporting of maternal and perinatal deaths, including zero deaths, through mTRAC into DHIS2; monthly HMIS counts of maternal deaths identified and reviewed.
- **Current practice:** the FY 2022/23 national MPDSR report says MoH "continues to support implementation of the 2017 National MPDSR Guidelines"; reviewed deaths are entered as events in the e-HMIS/DHIS2; a digital MPDSR action tracker for review recommendations was rolled out in some regions [4]. The live HMIS 108 has "CI03a. Total Deaths notified" [7] (secondary).

### 3.5 Birth notification to NIRA

*The Registration of Persons (Births and Deaths) Regulations, 2015* (SI 2015 No. 68, under the Registration of Persons Act 2015) [5]:

- **Reg. 6, facility births:** "Every birth occurring in a medical facility shall be recorded by the administrator of the facility." The administrator obtains the reg. 8(2) particulars and the parents' signatures and files returns "every week with the registration officer of the relevant registration area", under the registration officer's guidance. "Medical facility" includes public and private hospitals, clinics, health units, nursing homes and maternity centres.
- **Reg. 7, births elsewhere:** parents notify; failing them the occupier, guardian, birth attendant or midwife; within **30 days**.
- **Reg. 8(2) particulars (Form 3):**
  - child: name, sex, date, any disability, birth weight, place and district of birth;
  - mother: name, age, marital status, residence, nationality, NIN or alien number, education, occupation, previous births;
  - father: the same except previous births;
  - informant: name, nationality, NIN, signature, date.
- **Reg. 9–10:** on registration NIRA allocates a NIN to a citizen child and issues a birth certificate (Form 4).
- **Electronic channel:** hospitals notify through a web application of NIRA's **Mobile Vital Records System (MVRS)**, communities through USSD; an event counts only once recorded in MVRS, verified and approved by NIRA. UNICEF reports MVRS in 135 hospitals (undated) [14] (secondary). Paper notification forms distributed to facilities are often unused [14] (secondary).

Whether MVRS exposes an API for EMRs, and how stillbirths are notified to NIRA, are **unverified**.

## 4. Blood transfusion

### 4.1 The blood transfusion record

From the 2010 manual, Technical Module 5 [1]. The heading reads "HMIS Form 091B" and the form itself "HMIS Form 091"; the same manual also uses 091b for the inpatient tally sheet, so the number is ambiguous.

- **Objective:** "To maintain records of blood transfusion carried out in health facilities." Filled whenever transfusion is required, by the laboratory in-charge or a designated person.
- **Columns:**

  | # | Column |
  |---|---|
  | 1–4 | Item; unit number; unit blood group (ABO and Rh); component (whole blood, packed cells, platelets, plasma, cryoprecipitate) |
  | 5 | On arrival: initials of who received and inspected the blood |
  | 6–10 | Patient name, sex, age, patient's blood group, hospital (IP) number |
  | 11–13 | Hb before transfusion (g/dL); ward; requesting doctor |
  | 14 | Compatibility testing: immediate spin (IS) and indirect antiglobulin test (IAGT), "+" incompatible, "−" compatible |
  | 16–20 | Expiry date; date/time out; taken by (who carries it to the bedside); issued by; date/time returned and by whom |
  | 21 | Reason: anaemia, surgery, haemolysis (from the request form) |
  | 22 | Notes; date issued by the blood bank; date received by the facility |

- The record covers request (via the request form, which is not reproduced), cross-match, issue and return. It has no columns for **administration** (start, end, observations) or **reactions**. No national request form, bedside transfusion observation chart or transfusion reaction form was found (**unverified**).
- The inpatient treatment sheet (HMIS 051) lists transfusions under "special services" [1].

### 4.2 HMIS 108 item 5

- **2010:** "Blood Transfusions (Units)" and "Other Transfusions (units)", tallied from the transfusion record via Table 7 [1].
- **Live DHIS2** [7] (secondary): **UT01** No. of units requested; **UT02** No. of units received; **UT03** No. of units transfused; **UT04** No. of adverse blood reactions in the facility. Disaggregation is **unverified**.

### 4.3 UBTS supply

- The Uganda Blood Transfusion Service (UBTS) is a centrally coordinated national service with **7 regional blood banks** (Nakasero headquarters and reference centre, Arua, Fort Portal, Gulu, Kitovu, Mbale, Mbarara) and collection centres [15] (secondary). Some RRHs without a regional bank (Mubende, Hoima) draw from Nakasero (2017) [15] (secondary).
- Hospitals hold blood in their own storage. The 2024 MoH emergency care assessment found 80% of facilities with functional blood storage equipment but many without storage units, and urgent theatre transfusion turnaround of 10–19 minutes in over half of theatres [6].
- The 2016 standards list blood transfusion services from HC IV upward and blood grouping and cross-matching as essential maternity laboratory tests (7.30.1) [2].
- How a hospital orders blood from UBTS (form, system, frequency) and whether UBTS runs an electronic hospital ordering or traceability system were not found (**unverified**). The UBTS website did not resolve on the research date.

### 4.4 Haemovigilance

- No national haemovigilance guideline, reaction reporting form or reporting route was found (**unverified**).
- A Mulago-based study states "there is no active haemovigilance system in Uganda"; a Mbarara study found UBTS SOPs not followed and proposed reviving haemovigilance through the hospital transfusion committee [16] (secondary).
- The only routine national count is HMIS 108 UT04, adverse blood reactions [7] (secondary).

## 5. ICU and HDU

### 5.1 Standards

Standard 7.22 "The facility has a functional intensive care unit" [2]: a clear ICU operational policy; nurse-to-patient ratio of 1:1; a consultant director; 24-hour on-site medical cover; organ support equipment; agreed and documented procedures (suction, vascular catheters, circuits); **"Individual patient records"**; regular, preferably weekly, multidisciplinary case reviews. ICUs are an RRH-level service (Table 1) [2].

The 2024 MoH emergency care assessment defines ICU and HDU (an HDU is intermediate between ward and ICU) and assessed 11 facilities with ICUs (Kiruddu, Kawempe and Mulago NRHs; Mbale, Jinja and Mbarara RRHs; IHK, Mengo, Nakasero, Lacor and Nsambya), of which 7 had HDUs; 34% of ICU staff had specialist critical care training [6]. A 2018 survey found 12 ICUs and 55 beds (1.3 per million) and called for standardization of ICU care (secondary).

### 5.2 What is recorded and counted

- **HMIS 108 census:** "Intensive Care Unit (ICU)" and "Neonatal Unit" are Ward Types, so ICU beds, admissions, deaths and patient days are counted like any ward [7] (secondary). There is no HDU Ward Type; "Acute care unit" exists.
- **No ICU-specific HMIS data set** (ventilator days, severity scores, ICU readmissions) was found (**unverified**).
- **ICU register:** the MPDSR guidelines name an "ICU register" as a source for finding maternal deaths [3], but no national ICU register form was found (**unverified**).
- Diagnoses such as "Acute sepsis – given oxygen" (ME06b) appear in HMIS 108 item 6 [7] (secondary).

### 5.3 Early-warning scores

- **No MoH document mandating NEWS, NEWS2, MEWS, PEWS or MEOWS was found.** The 2016 standards and the 2024 assessment do not mention one [2], [6].
- A prospective study at Mulago found MEWS ≥ 5 independently associated with mortality among ward patients and recommended MEWS as a triage tool; a nurse education study at Mulago improved MEWS knowledge; a comparison of MEWS, NEWS, NEWS2 and the South African triage score was run in a Ugandan hospital [17] (secondary).
- ADR 0078 already decided "no early-warning score in v1"; this research found no national requirement that contradicts it.

## 6. Mortuary and deaths

### 6.1 Mortuary standards and services

- **Standard 7.50** "The health facility has functional mortuary services" [2]:
  - "policies and procedures guiding all aspects of preparation, storage, release and transportation of bodies according to the relevant national legislation e.g. National Human Tissue Act";
  - suitable infrastructure and equipment; trained staff; PPE;
  - **"A control system, which monitors the movement and release of bodies, including death notifications and related documents."**
- **Services by level** [2]: HC IV mortuary with autopsy limited to external examination; general hospital adds post-mortem imaging, needle sampling and embalming; RRH and NRH add refrigerated storage. Mortuary attendants appear in the 2010 staffing norms [1].
- **Mortuary register:** the MPDSR guidelines list the mortuary register as a source [3], but no national mortuary register form or HMIS number was found (**unverified**).
- **Body release:** no MoH body release procedure or form was found (**unverified**). The 2025 National Mortality Surveillance Guidelines "standardized all-cause death review tools, mortality registers, and gate passes" and set quarterly mortality data reviews; drafting included NIRA and the Uganda Police Force [18], [19] (secondary; only the abstract and workshop report were read). News reports describe bodies being photographed and identifying features recorded on arrival, and fee disputes over release (secondary).
- **Police cases and post-mortems:** the police post-mortem form and the procedure for medico-legal deaths were not found (**unverified**).

### 6.2 Death recording and notification to NIRA

*Registration of Persons (Births and Deaths) Regulations, 2015* [5]:

- **Reg. 17, facility deaths:** "Every death occurring in a medical facility shall be recorded by the administrator of the facility." The administrator obtains the reg. 20(2) particulars and files returns "every week with the registration officer of the relevant area".
- **Reg. 19, deaths elsewhere:** next of kin, relative, occupier or whoever takes charge of the body notifies the registration officer within **14 days**, on **Form 12**.
- **Reg. 20(2) particulars:** name, sex, date of death, place of death, date of birth; NIN or alien number; informant's name, capacity, nationality, NIN, signature and date.
- **Reg. 21, certificate of cause of death (Form 13):** "shall be forwarded by the administrator of the medical facility to the relevant registration officer **within seven days** of the occurrence of the death."
- **Reg. 16:** registration is free and compulsory, immediately and in any case within 3 months.
- **Reg. 22:** NIRA issues the death certificate.

How these fit together with health forms:

- The MPDSR guidelines send one copy of **HMIS 100** (medical certificate of cause of death) to NIRA [3]. Whether HMIS 100 and NIRA Form 13 are the same document is **unverified**.
- The FY 2022/23 MPDSR report records training in ICD-11 and MCCOD [4].
- MVRS also handles deaths [14] (secondary). In 2019 only about a third of deaths were notified to NIRA (secondary).

## 7. Relation to existing decisions

- **ADR 0078 deferred** the operating theatre, blood transfusion, labour ward, ICU and the mortuary, early-warning scores and death notification to NIRA. The facts above are inputs for #685–#687.
- **Census.** ICU and Neonatal Unit are already HMIS 108 Ward Types, so ADR 0078's location-history census covers their beds, admissions, deaths and patient days. HDU has no Ward Type.
- **Numbering.** ADR 0078's Inpatient Number restarts each July (HMIS 054). The theatre serial number restarts each **January** (HMIS 057). The maternity register has its own "unique serial number given to each admission to maternity", entered on the partograph (HMIS 072). The 2010 manual does not say whether the maternity IP number is the facility IP number.
- **Discharge statuses.** HMIS 072 adds baby outcomes (SB, NND, AL) and fresh versus macerated stillbirth, which HMIS 054's statuses do not cover.
- **Deaths.** ADR 0078 records a cause-of-death certificate in the WHO MCCD layout (ICD-11). The law adds a 7-day duty to forward the certificate (Form 13) and weekly facility returns of births and deaths to the registration officer; MPDSR adds 24/48-hour notification (HMIS 120a/121a) and 7-day review (HMIS 120/121).
- **Charting.** ADR 0078's administration chart and vital-sign Observations have no transfusion administration or reaction fields; the national transfusion record (§4.1) stops at issue and return.
- **Early-warning scores.** No mandate found, consistent with "no early-warning score in v1".

## 8. FHIR R5 fit

Maturity levels from the R5 resource list [8]. All are trial use except Observation (normative).

| Need | R5 resource | Maturity |
|---|---|---|
| Theatre register entry, operation note | Procedure | 4 |
| Theatre booking | Appointment, Schedule, Slot | 3 |
| Request to theatre, blood request | ServiceRequest | 4 |
| Blood unit | BiologicallyDerivedProduct | 2 |
| Blood issue to a patient | BiologicallyDerivedProductDispense (new in R5) | 0 |
| Transfusion reaction | AdverseEvent | 2 |
| Cross-match sample | Specimen | 2 |
| Partograph or LCG readings, Apgar, birth weight, early-warning score | Observation | N |
| Early-warning risk result | RiskAssessment | 2 |
| Anaesthetic record, MPDSR review, notification forms | Composition | 4 |
| Notification and review tasks | Task | 3 |
| Encounter per stay | Encounter | 4 |

How the R5 resources would represent a body in the mortuary, and whether any FHIR implementation guide covers MPDSR or civil registration notification, were not researched (**unverified**).

## 9. Not verified

- Current (2019 revision or later) numbers and layouts of the theatre register, maternity register, blood transfusion record and HMIS 100; whether the maternity register is still HMIS 072.
- The live HMIS 105 maternity rows; the disaggregation of HMIS 108 procedure and transfusion counts.
- A national anaesthesia record, theatre booking standard, or MoH-issued Surgical Safety Checklist; the MoH 2011 adoption of the checklist; NSOAP status.
- The EMNC guidelines document, its year, and the national LCG rollout status and form.
- A national blood request form, transfusion observation chart, reaction form, haemovigilance route; how hospitals order from UBTS.
- An ICU register form and any ICU-specific data set.
- A national mortuary register, body release procedure, gate pass form and police post-mortem form; the full National Mortality Surveillance Guidelines.
- Whether HMIS 100 is NIRA Form 13; MVRS interfaces for EMRs; stillbirth notification to NIRA.
- Live DHIS2 facts come from third-party code [7], not from MoH.

## 10. Sources (read 2026-11-18 unless dated)

1. Uganda MoH, *The Health Management Information System, Volume 1: Health Unit Procedure Manual*, August 2010: Technical Module 3 (HMIS 071, HMIS 072 maternity register, family planning theatre rows), Module 5 Part 11 (HMIS 057 theatre register), Part 12 (Table 7), blood transfusion record (HMIS 091/091B), Module 7 (HMIS 108), staffing norms, Annexes VII–IX. <https://www.gou.go.ug/sites/default/files/media-files/THE%20HEALTH%20MANAGEMENT%20INFORMATION%20SYSTEM_0.pdf>
2. Uganda MoH, *Health Sector Service Standards and Service Delivery Standards*, 2016: Table 1; §§56, 75, 89, 104 (mortuary by level); standards 4.1.4, 7.12–7.22, 7.26, 7.30–7.34, 7.50. <https://library.health.go.ug/sites/default/files/resources/Health%20Sector%20Service%20Standards%20&%20Service%20Delivery%20Standards_2016.pdf>
3. Uganda MoH, *Maternal and Perinatal Death Surveillance and Response Guidelines*, August 2017: §1.11, chapter 3, §5.1.2 (HMIS forms), annexes (HMIS 120, 120a, 121). <https://guluhospital.net/wp-content/uploads/2023/02/MATERNAL-AND-PERINATAL-DEATH-SURVEILLANCE-AND-RESPONSE-GUIDELINES-AUGUST-2017-2.pdf>
4. Uganda MoH, *National MPDSR Report FY 2022/2023*, 7th edition. <https://guluhospital.net/wp-content/uploads/2023/10/National-MPDSR-Report-for-FY-2022-2023-7th-Edition.pdf>
5. *The Registration of Persons (Births and Deaths) Regulations, 2015*, Statutory Instrument 2015 No. 68, Uganda Gazette supplement 11 December 2015: regulations 2–10, 14–22. <https://guluhospital.net/wp-content/uploads/2024/07/Births-and-Deaths-Reg-20158.pdf>
6. Uganda MoH, *Emergency Care Systems in Uganda: A Comprehensive Facility-Based Assessment for Emergency, Critical and Operative Care*, December 2024: definitions, facilities assessed, §4.2.1. <https://seedglobalhealth.org/wp-content/uploads/2025/03/Final-Report-ASSESSMENT-OF-EMS.pdf>
7. `marksemakula/hmis-report-compiler` (third-party compiler for Jinja RRH): `api/_lib/dhis2_mapping.json` (data set and data element names for HMIS 105:01 and HMIS 108 read from `hmis.health.go.ug`), `api/_lib/metadata.py` (Ward Types). Secondary. <https://github.com/marksemakula/hmis-report-compiler>
8. HL7 FHIR R5 (5.0.0) resource list with maturity levels. <https://hl7.org/fhir/R5/resourcelist.html>
9. Prior research note `research/clinical-operations` in this repository (`docs/research/clinical-operations.md`): wards, HMIS 108 census, Ward Types.
10. WHO Surgical Safety Checklist studies in Uganda (secondary, via search summaries): multicentre compliance in referral hospitals, *Anesthesia & Analgesia* 127(6):1427–1433 (2018) <https://resources.wfsahq.org/anesthesia-and-analgesia/world-health-organization-surgical-safety-checklist-compliance-and-associated-surgical-outcomes-in-ugandas-referral-hospitals/> ; Mbarara QI project <https://nru.uncst.go.ug/handle/123456789/3599> ; NSOAP status (medRxiv 2022) <https://medrxiv.org/content/10.1101/2022.09.22.22280231v2.full.pdf>
11. Makerere University dissertation, WHO SSC adherence in emergency obstetric surgery at Kawempe, 2023 (secondary, via search summary; source of "MoH adopted the checklist in 2011"). <https://dspace.mak.ac.ug/items/211b374f-2269-4091-b1f7-3f9d786496b2>
12. Study protocol on the WHO Labour Care Guide in Mbarara, medRxiv, June 2023 (secondary, via search summary; EMNC guideline statement). <https://medrxiv.org/content/10.1101/2023.06.06.23291028v1.full-text>
13. "Exploring potential opportunities and strategies of using the Labour Care Guide … in Uganda: a qualitative study", *BMC Pregnancy and Childbirth* 25:1272, 26 November 2025 (secondary). <https://pmc.ncbi.nlm.nih.gov/articles/PMC12659402>
14. UNICEF Uganda, "Mobile Vital Records System (MobileVRS)" (undated) <https://www.unicef.org/uganda/what-we-do/mobilevrs> ; IUSSP CRVS side-meeting presentation <https://iussp.org/sites/default/files/IUSSP%20side-meeting%20CRVS%20Ssekisaka%20Farouk.pdf> ; HEPS Uganda on CRVS interoperability <https://www.heps.or.ug/the-interoperability-of-crvs-systems-is-critical-for-the-digitalization-of-hospital-records/> (all secondary).
15. CDC, UBTS headquarters success story <https://www.cdc.gov/globalhivtb/who-we-are/success-stories/success-story-pages/uganda-blood-transfusion.html> ; Ministry of Finance BMAU briefing paper 24/17, "Challenges facing Uganda Blood Transfusion Services" (2017) <https://archive.finance.go.ug/sites/default/files/Publications/BMAU%20Briefing%20Paper%2024-17%20-%20Challenges%20facing%20Uganda%20Blood%20Transfusion%20Services%20%28UBTS%29.pdf> (secondary).
16. Haemovigilance studies (secondary, via search summaries): Mbarara stored-blood quality, *J Blood Med* 10 (2019) <https://www.dovepress.com/assessment-of-the-quality-of-stored-blood-for-transfusion-at-mbarara-r-peer-reviewed-fulltext-article-JBM> ; Uyoga et al., *Vox Sanguinis* 2019 <https://discovery-pp.ucl.ac.uk/10066845/7/George_Uyoga_et_al-2019-Vox_Sanguinis.pdf> ; Mulago study <https://pmc.ncbi.nlm.nih.gov/articles/PMC9499229>
17. Early-warning score studies (secondary): MEWS at Mulago <https://www.ncbi.nlm.nih.gov/pmc/articles/PMC4795640/> ; MEWS nurse education <https://nru.uncst.go.ug/handle/123456789/3591> ; score comparison <https://pmc.ncbi.nlm.nih.gov/articles/PMC6964175> ; ICU capacity survey <https://nru.uncst.go.ug/items/3fd4d846-069e-477a-b1ee-9f8f2536795d/full>
18. Kyozira C. et al., "The journey of strengthening mortality surveillance in Uganda through multisectoral guideline development", *Discover Health Systems*, 29 May 2026 (abstract only; secondary). <https://nru.uncst.go.ug/items/d200f89a-c956-4f70-b5c8-73c4d7743b92/full>
19. AFENET, "Development of the National Mortality Surveillance Guidelines", workshop 12–16 May 2025 (secondary). <https://afenet.net/?p=34151>

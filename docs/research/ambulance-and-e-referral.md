# Research: ambulance dispatch and outside e-referral systems in Uganda

Findings for the research ticket [What do ambulance dispatch and outside e-referral systems require in Uganda?](https://github.com/carapai/people-centered-healthcare/issues/761). This note reports facts and decides nothing. It covers what [ADR 0077](../adr/0077-referrals-carry-a-summary-into-the-receivers-sync-scope-and-close-with-feedback.md) and [referrals.md](../architecture/referrals.md) leave out of v1: ambulance dispatch (912, call and dispatch centres, KDETS), referrals sent from VHT apps, and exchange with outside e-referral systems. It builds on the note on branch `research/clinical-operations` (`docs/research/clinical-operations.md`, §2.3 HMIS 032, §2.5 e-referral systems, §2.6 ambulance and dispatch) and does not repeat it.

Research date: **2026-11-19**. Every source was read on that date unless a date is given. Claims marked **unverified** have no primary source yet. "Secondary" means a news item, study, partner page or third-party report that states the fact, not the Ministry of Health (MoH) document itself. Source numbers in square brackets refer to §7.

## 1. Summary

| Topic | Key facts | Main sources |
|---|---|---|
| Law and policy | National EMS Policy approved 2021 (text not found online). The EMS Strategic Plan designs 14 regional call and dispatch centres on a cloud Computer Aided Dispatch (CAD) platform. The **Traffic and Road Safety (Post Crash Emergency Care) Regulations, 2023** (SI 2023 No. 31) require a toll-free universal access number of three or four digits, linked to a call and dispatch centre, and bar facilities from refusing a crash patient for lack of money. | [1], [5] |
| 912 | UCC allocated 912 to MoH for EMS. MoH's own Health Alert page lists "SMS 6767 or 912" for medical emergencies. In November 2025 MoH said it would introduce 912 by the end of 2025. Whether 912 now reaches a dispatcher anywhere is **unverified**. | [2], [6], [13], [14] |
| Centres | Naguru NRH and Masaka RRH centres established (SP II). Lira RRH runs a Regional Ambulance Dispatch Centre coordinating 12 ambulances for about 100 facilities (March 2026). Arua, Mbale and Mbarara were under construction. KCCA runs a Kampala centre. No national coordination centre was found. | [3], [15], [16] |
| Call-to-dispatch flow | Plan: call to the universal number, dispatcher categorises by priority **AS1 Emergency (red), AS2 Urgent (yellow), AS3 Routine (green)**, CAD picks the closest suitable vehicle, crew can request more resources, ambulance-to-hospital radio gives pre-arrival notice for handover. Pre-hospital triage picks the destination. Kampala staff were trained on an "existing MOH national call and dispatch manual", not found online. | [1], [11] |
| What is recorded | Plan indicators: activation time (call to mobilisation), travel time (dispatch to scene), response time (call to arrival), case holding time (scene time), arrival to first vitals and to first provider care, deaths at 2, 8 and 24 hours. Forms **HMIS EMS 002 (ambulance)** and **HMIS EMS 003 (emergency unit)** exist, but 73.7% and 80.7% of facilities lacked them in 2024. Their fields were not found. | [1], [2] |
| Prehospital patient care record | No national prehospital patient care report (PCR) or handover form was found beyond the name HMIS EMS 002 (**unverified** whether it is a per-patient record). | [2] |
| Ambulance types | MoH defines **Type A** patient transport, **Type B** basic life support, **Type C** advanced life support / ICU ambulance, plus isolation, air, medical rescue and medical response vehicles. **No Type D** appears in MoH sources. Targets: 1 Type B per 100,000 people, 1 Type C per 2 M. Public fleet: 296 (June 2026), 54% of need; 50 Type B and 2 Type C ordered for 2026/27. | [1], [17] |
| Facility role | The EMS acts only on **emergency transfers requested by a health facility**. Emergency unit staff start the transfer under "National guidelines for case referral and inter-facility transfers" (not found). Non-emergency transfers go to private Type A vehicles. Receiving facilities get pre-arrival notice. | [1] |
| Dispatch software | MoH advertised an EOI for "Design and Development of the National EMS Call & Dispatch System" (May–June 2023): decentralised Medical Emergency Coordination Centres, GPS/GIS, and integration with EMRs and HMIS. The winner and product are **unverified**. Only **KDETS** (Kampala, PSI/USAID MaNe project with KCCA) is documented: request, dispatch, track, notify the receiver, feedback to the sender. No public API was found for either. | [7], [11] |
| MoH e-referral | MoH says an e-referral system was piloted at Kiruddu NRH and rolled out to all RRHs. Hospitals report it does not cross hospitals and Arua's referral component "has not functioned" (2025). Platform, builder and data set are **unverified**. The RRH EMR (IHMIS, built by the government's IICS Technologies, 2020) tracks patients "through… referrals" within a hospital. No referral API, message format or FHIR profile was found. | [8], [9], [10] (secondary) |
| Referral form and guidelines | The national form is still **HMIS 032 Referral Note** (see the clinical-operations note). No current national referral guideline was found. SP II plans "standards and guidelines for EMS & referral". | prior note; [3] |
| Community referrals | eCHIS is MoH-owned, built on the Community Health Toolkit (CHT) with OpenSRP alongside, hosted in the MoH data centre and linked to DHIS2. It records referrals (14,917 sick children by October 2023). Sending VHT referrals to facility EMRs ("flagged at facilities") was a **planned next step** in 2023; no source shows it live. Today the patient carries a paper VHT referral note. | [9], [18], [19], [20] |

## 2. Emergency medical services

### 2.1 Law and policy

- **National EMS Policy, 2021.** Approved by Cabinet in 2021 and launched in November 2021 (secondary). Its text was not found online (**unverified** contents). The EMS Strategic Plan and the 2024 assessment cite it [1], [2].
- **Post Crash Emergency Care Regulations, 2023** [5], made under the Traffic and Road Safety Act by the minister for transport, 14 March 2023:
  - Reg. 7: the transport regulator, with UCC, "shall… establish and maintain a universal access number as a toll free number" for reporting a road crash to the call and dispatch centre. The number must be national, free from every landline and mobile, no more than three or four digits, "linked to a dispatch centre that can rapidly send an equipped ambulance with trained personnel", and protect the caller's confidentiality. Misuse is an offence.
  - Reg. 8: vehicle owners must display the number.
  - Reg. 9: the transport regulator, with MoH and the Police, "shall establish and maintain a call and dispatch centre" with a communication system, ambulances, medical equipment and trained staff.
  - Reg. 10: a facility that treats a crash patient may bill the patient after stabilisation; one that refuses to admit without proof of ability to pay may be deregistered, and the practitioner in charge disciplined.
- **Digital health plan.** The *Health Information and Digital Health Strategic Plan 2020/21–2024/25* lists as a priority: "Implement a call and dispatch system for emergency health services linked to health facility electronic medical record systems" [4].

### 2.2 The 912 number

- The 2024 MoH assessment: 912 "was allocated by the Uganda Communication Commission (UCC) for EMS" but "has been redundant due to the lack of an established medical call and dispatch system" [2].
- October 2023: MoH's EMS department said UCC had allocated 912 and that the World Bank and other partners fund the call and dispatch work [14] (secondary).
- 25 November 2025: MoH said it "will introduce a short code – 912 – by the end of this year" [13] (secondary).
- MoH's Uganda Health Alert System page today says: "Medical emergency? Call 0800-100-066 (toll-free), SMS 6767 or 912, or go to the nearest health facility" [6].
- No 2026 launch announcement was found. Whether 912 is answered by a medical dispatcher, and where it routes, is **unverified**. Police run 999 and 911/112 (clinical-operations note §2.6).

### 2.3 Call and dispatch centres

| Centre | Status | Source |
|---|---|---|
| Naguru (China-Uganda Friendship) NRH | Established, with an ambulance hub | SP II [3] |
| Masaka RRH | Established, with an ambulance hub | SP II [3] |
| Lira RRH | "Regional Ambulance Dispatch Centre… coordinates 12 ambulances serving nearly 100 health facilities across 10 administrative units in the Lango sub-region" (March 2026). Launched about October 2025, with GPS tracking. | [15] (secondary) |
| Arua, Mbale, Mbarara | Under construction from FY 2022/23 | SP II [3] |
| KCCA, Kampala | Emergency call and dispatch centre at City Hall since 2020, now run by KCCA | [11] |
| National coordination centre | Planned; no source shows it built | [1], [2] |

SP II plans five more regional centres [3]. The 2024 assessment found call and dispatch centres "generally unavailable in 78.3% of facilities"; only 10% reported a functional one, mostly general hospitals [2]. In December 2025 MoH announced it would register all ambulances in Uganda [16] (secondary).

### 2.4 How a call becomes a dispatch

From the EMS Strategic Plan, strategy 2.3 and Annex 5 [1] (a design, not a description of current practice):

1. A caller dials the universal access number (911 in the plan; 912 since). The plan links it to the Police 999 system, which already has location-based routing to regional command centres.
2. The regional call and dispatch centre, open 24 hours, categorises the call:
   - **AS1 Emergency (red):** immediate attention and response, highest priority.
   - **AS2 Urgent (yellow):** needs attention, but less than AS1.
   - **AS3 Routine (green):** a normal case.
3. Dispatchers use CAD with automatic vehicle location (AVL), GPS and GIS "to identify and assign the closest and most appropriate providers as well as concurrently capturing data on each incident".
4. A tiered response sends first responders from the nearest health facility first, and a second ambulance crew only if needed.
5. The crew can call for more resources from the scene.
6. Pre-hospital triage decides the destination, "instead of simply to the nearest health facility" when it lacks the care needed. A medical director can advise the first responder by phone.
7. An ambulance-to-hospital radio link gives pre-arrival notice "to set up an operation team or to have specialized equipment and personnel standing by for smooth hand over".
8. A national coordination centre links to the regional centres "for supervision and quality assurance".

In Kampala, call agents were trained in 2020 on "the existing MOH national call and dispatch manual" [11]. That manual was not found online (**unverified**).

### 2.5 What is recorded

- **Call and dispatch indicators** (plan Table 6) [1]:

  | Indicator | Interval |
  |---|---|
  | Activation time | Receipt of call to mobilisation of an ambulance |
  | Travel time | Dispatch to arrival at the scene |
  | Response time | Receipt of call to arrival at the patient |
  | Case holding time | Arrival at the scene to leaving with the patient |

- **Ambulance performance** [1]: median time from arrival to first vitals and to first provider care; patients with severe emergencies managed and transported by crew, by type; severe emergencies not transported; median transport time; patients given scene care; emergency deaths within 2, 8 and 24 hours of arrival.
- **Data flow** [1]: emergency units, hospital emergency departments, regional EMS hubs and national level; reported "via the National DHIS2 system". "Ambulance performance data forms will be developed and managed at the regional EMS hubs." Calls to the centres "will be routinely collected in electronic formats" and used for syndromic surveillance. Out-of-hospital data are "summarised… by the medical responders and ambulance crew and handed over to the Emergency medical departments". The plan says new data forms for out-of-hospital care "have been integrated in the DHIS".
- **Forms in use** [2]: the ambulance form **HMIS EMS 002** and emergency unit form **HMIS EMS 003**. In 2024, 73.7% of facilities lacked EMS 002 and 80.7% lacked EMS 003 or a referral register; where present, EMS 002 was used in 60% (9 of 15). Facilities called the EMIS forms outdated. The forms' fields were not found online (**unverified**).
- **HMIS 105** carries an Emergency Medical Services section (ES01–ES09) (see the clinical-operations note §4.3).
- **Kampala centre log** [11]: an Excel tool with the caller's biographical and demographic details, reason for the call, and referring and receiving facilities. KDETS let drivers add transport details "to help monitor data on response time".

### 2.6 Prehospital patient care record

No national prehospital patient care report, run sheet or handover form was found. The only named per-ambulance tool is HMIS EMS 002, and whether it is per patient or per trip is **unverified**. A Kawempe study of obstetric ambulance transfers took data from the driver, the sending facility's referral form and the receiving hospital's ambulance log book [23] (secondary, via search summary).

### 2.7 Ambulance types

MoH lists seven medical emergency vehicle types (road, water and air), citing MoH 2018 standards [1]:

| Type | Definition | Public-sector role |
|---|---|---|
| **Type A** | Patient transport vehicle | Left to the private sector for non-emergency transport |
| **Type B** | Basic life support / emergency ambulance | Universal coverage, 1 per 100,000 people |
| **Type C** | Advanced life support / intensive care unit ambulance | RRHs and super-specialised centres, 1 per 2 M people |
| Isolation | Negative-pressure ambulance for highly infectious patients | — |
| Air ambulance | Licensed under the Civil Aviation Act | Plan: 5 aero-medical units |
| Medical rescue vehicle | Equipped for medical rescue | — |
| Medical response vehicle | Equipped for medical response | — |

An ambulance must be marked, equipped and "deployed with a minimum of two emergency care providers" [1]. The plan also budgets boat ambulances. **No "Type D"** was found in any MoH source. One news item mentions "278 Type D ambulances" (secondary); it is likely a misprint for Type B (**unverified**).

Fleet: as of June 2026 Uganda had 296 standard public ambulances, 54% of the required fleet, a deficit of 254. In 2026/27 the government will buy 50 Type B and 2 Type C, and 4 Type B through road projects [17] (secondary). Earlier figures were 285 Type B and 11 Type C [24] (May 2026, secondary).

### 2.8 The facility's role in inter-facility transfers

From the plan, strategy 2.4 [1]:

- Inter-facility transfer, step up or step down, "has been the commonest use of ambulances".
- "The EMS will only process and act on emergency transfers requested for by the Health facility."
- "The emergency department/unit personnel will activate interfacility transferrals by the following standard protocols and National guidelines for case referral and inter-facility transfers." That guideline was not found (**unverified**).
- Non-emergency transport is left to the private sector (Type A) or ambulances outside the regional network.
- As receiver, the facility gets pre-arrival notice and takes the handover (§2.4 step 7).

In KDETS (§3.2) a facility both requests the ambulance and, as receiver, is notified and sends case information back to the sender [11].

## 3. Dispatch systems

### 3.1 National call and dispatch system

- **Design** [1]: one cloud CAD platform for all 14 regions, with AVL, GPS and GIS.
- **Procurement** [7] (secondary copy of a MoH notice): MoH "Notice of Expression of Interest: Design and Development of the National Emergency Medical Services Call & Dispatch System", published 25 May 2023, closing 15 June 2023.
  - Purpose: a system for decentralised Medical Emergency Coordination Centres, to raise the share of emergency patients reaching an ambulance within one hour from 10% to 50% in three years, improve referrals and "integrate with Electronic Medical Record systems". The notice says ambulances are "uncoordinated across district, regional, and national levels".
  - Bidders needed at least five years of healthcare software (especially emergency management), NITA-U level 2 approval, medical call and dispatch experience, GPS/GIS, EMR and HMIS experience, and a local digital health partner.
- **Outcome:** no award, product name, vendor or interface specification was found (**unverified**). MoH would own the system as the procuring entity.
- **Lira** (2025–2026): GPS-tracked dispatch of the closest vehicle [15] (secondary). The software is **unverified**.

### 3.2 KDETS (Kampala)

From the *Global Health: Science and Practice* paper on the MaNe project, June 2023 [11]:

- **Owners:** built by the USAID-funded Kampala Slum Maternal and Newborn (MaNe) project of PSI Uganda and KCCA. KCCA supervised the centre and "has taken over the operating costs".
- **Centre:** opened April 2020 at KCCA City Hall with computers, phones, headsets and ambulance tracking screens; nine staff, 24 hours, three shifts; a toll-free number.
- **App functions:** (1) private and public facilities and the community request referral transport; (2) requests go through the centre; (3) agents dispatch to available drivers; (4) agents notify the receiving facility to prepare; (5) after the trip, drivers add transport details for response-time monitoring; (6) receiving providers send case management information back to the referring facility.
- **Pilot:** December 2020 with four public and four private facilities, on project tablets. Modelled on ride-hailing ("like Uber").
- **Volume:** 10,183 calls April 2020–June 2021; 25% maternal, 3% newborn, 14% COVID-19; an ambulance was dispatched for 35% of calls.
- **Interfaces:** none published. No API, standard or source code was found.

## 4. MoH e-referral and the national referral form

### 4.1 The MoH e-referral system

- MoH says an e-referral system was piloted at Kiruddu NRH and rolled out to all RRHs. Hospitals reported a "rough and bumpy start", paper in parallel, no hospital able to share results with another, and that Arua's national referral component "has not functioned"; Arua staff use WhatsApp [10] (secondary, July 2025; not re-read because the site returned HTTP 503 on 2026-11-19).
- Platform, developer, data set and coverage are **unverified**. No MoH document describing it was found.

### 4.2 The hospital EMRs it sits on

- **IHMIS** (Integrated Health Facility Information Management System): enrolled in ten RRHs (Masaka, Mbarara, Naguru, Entebbe, Soroti, Lira, Mbale, Kabale, Moroto, Hoima) in October 2020, built by "the government's Integrated Intelligent Computer Systems (IICS) technologies", with Global Fund-funded networks. It tracks patients "from diagnosis through prescription, treatment, and referrals, on a centrally monitored network" [8] (secondary).
- **MoH EMR rollout** [9]: phase 1 to July 2023 covered 23 national and regional referral hospitals and the MoH data centre; phase 2 (70 hospitals) from July 2023; phase 3 (183 HC IVs) 2024–2025; phase 4 (2,064 HC IIIs) from 2025. EMRs were live in 15 hospitals in October 2023.
- **Interoperability status in 2023** [9]: integrations done were eAFYA with UgandaEMR, the facility registry with the EMR and DHIS2, the client registry with UgandaEMR, and ClinicMaster with the ALIS lab system. A Shared Health Record, terminology services and "eCHIS and Registries Integration" were next steps. MoH named FHIR skills as a challenge. No referral exchange was listed.
- **March 2026:** MoH took over 16 national digital health systems from Makerere's METS programme [21] (secondary). None is named as an e-referral system.
- **UgandaEMR sync module** (`METS-Programme/openmrs-module-ugandaemr-sync`) has a `referredorders` REST resource [12]. In the code it returns orders that carry an accession number and their sync task, which is referred lab samples, not patient referrals. The module's API document shows a `referralorder` endpoint with patient, referring and receiving facility, reason and urgency that does not match the code. No facility-to-facility patient referral exchange was found in UgandaEMR.
- **Lira EHRS:** a Busitema University thesis (2026) built an Electronic Healthcare Referral System for Lira RRH as a case study [25] (secondary). Whether it runs is **unverified**.

No national e-referral API, message format or FHIR profile was found (**unverified**), the same result as the clinical-operations note.

### 4.3 The national referral form and guidelines

- The form is **HMIS 032 Referral Note**, with sender and receiver halves (clinical-operations note §2.3). No newer form was found.
- No current national referral guideline was found. The EMS plan cites "National guidelines for case referral and inter-facility transfers" [1]; SP II plans to "Develop standards and guidelines for EMS & referral" [3].

## 5. Community referrals

### 5.1 eCHIS

- **Owner and platform:** "developed by the Ministry of Health with support from partners and is currently hosted at the Ministry of Health data centre" [18]. Built on the **Community Health Toolkit** (upgraded from 3.9 to 4.2 in 2023) [18]. The government chose two open-source technologies, CHT and **OpenSRP**, with data "integrated into a single data management and visualisation platform" [19]. A Google Play app "eCHIS Uganda (OpenSRP)" (`ug.go.health.echis`) is listed as FHIR-based with "referral tracking" (secondary, search summary; the listing could not be read).
- **Users:** VHTs, VHT coordinators and facility Health Assistants (CHT app) [19]; CHEWs workflows were being revised [18].
- **Modules:** iCCM, RMNCH, family planning, health promotion, ANC, PNC, nutrition, immunization, stock monitoring, VHT supervision [18].
- **Scale:** 8,350 CHWs in 12 districts (January–October 2023) [18]; over 15,000 CHWs by 2024 [20] (secondary). Target: 30% of 150,000 CHWs (45,000) by 2025 [18].
- **Referrals recorded:** 14,917 sick children referred by eCHIS VHTs in January–October 2023 [18]; 76,988 referred to date by December 2024 [20] (secondary).
- **Integration:**
  - DHIS2: done; "supports automated reports from the CHWs" [18], [19]. eCHIS data are in the national data warehouse [18].
  - Facility EMRs: **not done**. Listed as a next step in 2023: "Integrate with Electronic Medical Records being deployed in the Facilities to allow for complete care. VHTs referrals will automatically be flagged at facilities with all the key findings and history and VHTs could be notified to follow up discharged patients in the communities" [9]; and "Integrate the eCHIS with the national Electronic Medical Records Systems (EMRs) for continuity of care" [18].
  - Also planned: the national CHW registry, NIRA for birth and death notification, Family Connect [9], [18].
- No source shows an eCHIS referral reaching a facility system electronically today (**unverified** for 2025–2026). No eCHIS referral API or FHIR profile was published.

### 5.2 How a community referral reaches a facility today

- **Paper:** the VHT fills a referral note that goes with the patient. When forms run out, VHTs write on plain paper [26] (secondary). The 2010 HMIS manual's community module has a VHT/iCCM register with a "Referred" column and a newborn home-visit register with "Danger Sign" and "Referred" columns [22]. Stock-outs of referral notes and VHT registers of over 20 days were found in every surveyed district in 2019 [26] (secondary, via search summary).
- **Counting at the facility:** referrals in from VHTs enter HMIS 105 as referrals to the unit (ADR 0077).
- **Other channels:** Obulamu Mama SMS notices (framework paper; see the clinical-operations note).
- **CHEWs:** over 20,000 trained by February 2026 per MoH data, supported through eCHIS [27] (secondary); KCCA and MoH began CHEW rollout in Kampala in August 2026, working with VHTs and facilities [27] (secondary). Whether CHEWs send referrals electronically is **unverified**.

## 6. Open questions

1. Does 912 reach a medical dispatcher today, at which centres, and is it linked to Police 999/112?
2. Who won the 2023 national call and dispatch system tender, what product runs at Naguru, Masaka and Lira, and does it expose an API?
3. What are the fields of HMIS EMS 002 and EMS 003, and is EMS 002 a per-patient prehospital care record?
4. Where are the "MOH national call and dispatch manual" and the "National guidelines for case referral and inter-facility transfers"?
5. What platform is the MoH e-referral system, who built it, what fields does it carry, and does it work between hospitals? Is it the IHMIS referral module?
6. Is the eCHIS-to-EMR referral link built, and on what standard? Does the OpenSRP eCHIS app expose referrals as FHIR?
7. Is the National EMS Policy 2021 text available, and does it define ambulance types or a Type D?

## 7. Sources

1. Uganda MoH, *National Emergency Medical Services Strategic Plan 2018/19–2024/25* (February 2019; 2020/21–2024/25 edition overlaid): vehicle types (p. 4), strategies 2.1–2.4 (pp. 13–15), §6 data and Table 6 indicators (pp. 29–30), Annex 5 call and dispatch flow (p. 59). <https://guluhospital.net/wp-content/uploads/2022/12/NATIONAL-EMERGENCY-MEDICAL-SERVICES-STRATEGIC-PLAN.pdf>
2. Uganda MoH, *Emergency Care Systems in Uganda: A Comprehensive Facility-Based Assessment for Emergency, Critical and Operative Care*, December 2024: background (912), §3.5 data systems, Table 27, Table 32. <https://seedglobalhealth.org/wp-content/uploads/2025/03/Final-Report-ASSESSMENT-OF-EMS.pdf>
3. Uganda MoH, *Strategic Plan II 2025/26–2029/30*: EMS progress (call and dispatch centres), results framework. <https://guluhospital.net/wp-content/uploads/2025/12/30.pdf>
4. Uganda MoH, *The Uganda Health Information and Digital Health Strategic Plan 2020/21–2024/25*, January 2021: priority interventions (p. 37). <https://extranet.who.int/cpcd/sites/default/files/public_file_repository/uga_uganda_health-information-digital-health-strategic-plan_2020-2025.pdf>
5. *The Traffic and Road Safety (Post Crash Emergency Care) Regulations, 2023*, SI 2023 No. 31, Uganda Gazette Supplement No. 12, 31 March 2023: regs. 7–10. <https://works.go.ug/wp-content/uploads/2026/04/Traffic-and-Road-Safety-Post-Crash-Emergency-Care-Regulations-2023.pdf>
6. Uganda MoH, Uganda Health Alert System, "Report Health Alert" page. <https://alerts.health.go.ug/add-alert>
7. MoH, "Notice of Expression of Interest: Design and Development of the National Emergency Medical Services Call & Dispatch System", published 25 May 2023 (copy on a jobs board; secondary). <https://www.greatugandajobs.com/jobs/job-detail/job-Notice-Of-Expression-Of-Interest-Eoi-Design-And-Development-Of-The-National-Emergency-Medical-Services-Call-Dispatch-System-Tender-Job-At-Ministry-Of-Health-45553>
8. The Independent (URN), "Referral hospitals enrolled on centralized patient management system", 21 October 2020 (secondary). <https://independent.co.ug/referral-hospitals-enrolled-on-centralized-patient-management-system>
9. Uganda MoH, *Health Sector Digitization Report*, October 2023: EMR phases, Integration and Interoperability Working Group, eCHIS next steps. <https://library.health.go.ug/sites/default/files/resources/Health%20Sector%20Digitization%20Bulletin.pdf>
10. Plus News, Uganda e-referral system in regional referral hospitals, 28 July 2025 (secondary; as summarised in the clinical-operations note, HTTP 503 on 2026-11-19). <https://plusnews.ug/?p=32362>
11. "Strengthening Kampala's Urban Referral System for Maternal and Newborn Care Through Establishment of an Emergency Call and Dispatch Center", *Global Health: Science and Practice* 11(3), June 2023. <https://pmc.ncbi.nlm.nih.gov/articles/PMC10285736/>
12. `METS-Programme/openmrs-module-ugandaemr-sync`: `docs/API_REFERENCE.md` (Referral Management), `omod/.../web/resource/ReferralOrderResource.java`, `.../DTO/ReferralOrder.java`. <https://github.com/METS-Programme/openmrs-module-ugandaemr-sync>
13. Nile Post, "Health Ministry says over 64% of Uganda now served by ambulances", 25 November 2025 (secondary). <https://nilepost.co.ug/news/305425/health-ministry-says-over-64-of-uganda-now-served-by-ambulances>
14. Kyaggwe TV, "Uganda's 912 Emergency Call, Dispatch System To Expedite Emergency Response", 13 October 2023 (secondary). <https://kyaggwetv.com/ugandas-912-emergency-call-dispatch-system-to-expedite-emergency-response/>
15. The Independent, "Lira Referral Hospital set to handle AFCON 2027 medical emergencies", 11 March 2026 (secondary); AllAfrica, "Lira Hospital's Smart Ambulance Hub…", October 2025 (via search summary). <https://www.independent.co.ug/lira-referral-hospital-set-to-handle-afcon-2027-medical-emergencies>
16. AllAfrica, "Ministry of Health to Register All Ambulances…", 11 December 2025 (secondary). <https://allafrica.com/stories/202512110124.html>
17. Monitor, "Uganda faces 254-ambulance deficit as govt orders 52 new units", 9 September 2026 (secondary). <https://www.monitor.co.ug/uganda/news/national/uganda-faces-254-ambulance-deficit-as-govt-orders-52-new-units-5590404>
18. Uganda MoH, *Electronic Community Health Information System* brief (performance January–October 2023, printed with support from Medic). <https://library.health.go.ug/file-download/download/public/1676>
19. Community Health Toolkit forum, "The Uganda journey of implementing a national Electronic Community Health Information System", 29 May 2023. <https://forum.communityhealthtoolkit.org/t/the-uganda-journey-of-implementing-a-national-electronic-community-health-information-system/2787>
20. Medic, "Transforming community health in Uganda: the journey of the electronic Community Health Information System", 12 December 2024 (secondary). <https://medic.org/stories/transforming-community-health-in-uganda-the-journey-of-the-electronic-community-health-information-system/>
21. UBC, "Government takes over national digital health systems from Makerere University", 31 March 2026 (secondary). <https://ubc.go.ug/?p=22880>
22. Uganda MoH, *The Health Management Information System, Volume 1: Health Unit Procedure Manual*, August 2010: Technical Module 6 (community registers). <https://www.gou.go.ug/sites/default/files/media-files/THE%20HEALTH%20MANAGEMENT%20INFORMATION%20SYSTEM_0.pdf>
23. "Obstetric ambulance transfers to Kawempe National Referral Hospital" study, PMC10359711 (via search summary; secondary). <https://pmc.ncbi.nlm.nih.gov/articles/PMC10359711>
24. Daily Monitor, "Uganda short 294 ambulances, health minister reveals amid Japanese donation", 5 May 2026 (via aggregator; secondary). <https://www.ugandaonline.net/news/2026-05-06--uganda-short-294-ambulances-health-minister-reveals-amid-japanese-donation--a89c0a14355a/>
25. Busitema University, *Electronic Healthcare Referral System (EHRS)*, case study at Lira RRH (via search summary; secondary). <https://bdears.busitema.ac.ug/handle/123456789/9167>
26. University of Cambridge EDC, *Community engagement, Kisoro* workshop report (VHT referral forms); MCSP Uganda baseline report, July 2019 (HMIS tool stock-outs) (both via search summary; secondary). <https://www-edc.eng.cam.ac.uk/sites/default/files/Community%20engagement_kisoro.pdf> ; <https://mcsprogram.org/download/45929>
27. New Vision, "Over 20,000 CHEWs trained as Uganda expands…", February 2026; KCCA, "KCCA, Ministry of Health begin rollout of CHEWs programme in Kampala", August 2026 (both via search summary; secondary). <https://www.newvision.co.ug/category/health/over-20000-chews-trained-as-uganda-expands-ac-NV_227462_022026> ; <https://kcca.go.ug/news/1167>

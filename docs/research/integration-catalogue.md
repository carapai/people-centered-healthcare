# Research: integration catalogue for Uganda

Findings for the research ticket [Which lab analysers, national lab systems, FHIR R4 peers and insurer APIs must the platform integrate with in Uganda?](https://github.com/carapai/people-centered-healthcare/issues/812). This note reports facts and decides nothing. It builds on [ADR 0051](../adr/0051-raw-json-structural-check-before-typed-parsing.md) (the Inbound Dead Letter), [fhir-r5-model.md](../architecture/fhir-r5-model.md) (R4 only at the boundaries), [pricing-and-insurance.md](../architecture/pricing-and-insurance.md) (one adapter per payer) and [desktop-client.md](../architecture/desktop-client.md) (analyser links out of v1).

It does not repeat earlier notes:
- [health-financing.md](health-financing.md): NHIS history to October 2026, IRA licensing, no national claims-exchange standard found, mobile money.
- [patient-identity.md](patient-identity.md): the MoH HIE guidelines' Client Registry and ECIDs, OpenCR and its Uganda/CPHL origin, NIRA's Third Party Interface.
- Branch `research/dhis2-hmis`: UgandaEMR posts `dataValueSets` to DHIS2 through `ugisl.mets.or.ug/ehmis`.

Research date: **2026-11-19**. Every source was read on that date. Claims marked **unverified** have no primary source yet. "Secondary" means a news item, newsletter, vendor marketing page or third-party copy, not the owner of the fact. Source numbers in square brackets refer to §7.

## 1. Summary

| Topic | Key facts | Main sources |
|---|---|---|
| Analysers in use | The MoH 2017 *National Standard Test Menu* lists the "current harmonized analyzers": Sysmex (XP-300, pocH-100i, XS, XT-2000i), Beckman Coulter, Nihon Kohden Celltac and HumaCount for haematology; Roche cobas (c111, c311/Integra, e411, 6000) and Human Humastar/Humalyte for chemistry; BD FACSCalibur, Partec CyFlow, Pima and BD FACSPresto for CD4; Abbott, Roche CAP/CTM and cobas 8800, Alere q, SAMBA and GeneXpert for viral load; GeneXpert, LPA, MGIT, BACTEC and Vitek for TB and microbiology. Mindray is **not** on the 2017 list. No later list was found. | [1] |
| Analyser protocols | GeneXpert: ASTM E1394-97 **or** HL7 v2.5, both over the ASTM E1381-02 low-level protocol on TCP/IP; host query and unsolicited order download are both specified. Of 37 OpenELIS Global plugins, most Sysmex, Roche cobas, BD and older molecular ones read **exported files**; Sysmex XP and XN-L, GeneXpert and Horiba speak ASTM over RS-232 or TCP; Mindray BC-5380, Abbott Architect and cobas 6800 speak HL7 over TCP. | [2], [3] |
| Interface engines | BLIS Interface Client (Java, RS-232/TCP/HTTP, HL7/ASTM/LIS2-A2/proprietary, flat files and analyser databases) lists 24 analysers. OpenELIS Global plugins (37) plus an HTTP↔ASTM bridge. Mirth Connect became closed-source from 4.6 (2025); 4.5.2 is the last open release; Open Integration Engine is an MPL fork. | [3], [4], [5], [6] |
| CPHL viral load | UgandaEMR posts the VL request to `ugisl.mets.or.ug/vl/sendbundledservicerequest`, posts programme data to `/vl-program/send`, and polls `/vl/request` for results. The request JSON is **FHIR-shaped but not valid FHIR**. Results come back as a Bundle of DiagnosticReport and Observation, matched by `basedOn` to the order. A manual CSV download from the CPHL dashboard and upload into UgandaEMR is the fallback. | [7], [8], [9] |
| National lab systems | ALIS (A-LIS, MoH's rebrand of C4G BLIS) runs in facility labs. The CPHL Laboratory Data Repository (LDR) and a Sample Tracking App launched in February (year not stated by APHL); over 100 laboratories connected; phased national rollout. No public LDR, EID or sample-tracking API was found. UgandaEMR has an ALIS profile (ServiceRequest out, results back). | [10], [11], [12], [13] |
| Mediator | `ugisl.mets.or.ug` is an **OpenHIM** console 1.18.2 titled "Uganda InterOperability Service Layer (UgISL)", hosted by METS. UgandaEMR's sync module also targets a staging MoH Client Registry at `mediator-api-staging.health.go.ug/fhir/Patient`, a facility Shared Health Record (HAPI FHIR), a national data warehouse, the EAC cross-border HIE and the eCBSS. | [7], [14] |
| FHIR versions | MoH HIE guidelines (January 2023) mandate "FHIR and HL7 V2.5.1 and above", CDA for documents, LOINC for lab reports and SNOMED CT; they name **no FHIR version and no IG**. UgandaEMR's sync module says FHIR R4. The OpenMRS FHIR2 module serves R3 and R4 only (no R5), with 20 R4 resource providers including ServiceRequest, DiagnosticReport and Task. | [7], [15], [16] |
| Insurers | Smart Applications International runs Smart Access (visit code by SMS or app, fingerprint plus OTP, existing biometric cards) and an installed "Smart system" at facilities; online preauthorization is described. A Sanlam–Smart web EDI rollout for e-claims is reported (secondary). Jubilee reports e-claims (secondary). No public provider API from any Ugandan insurer or TPA was found. NHIS: passed 2021, not assented; government targets 2027 rollout (November 2026 report). | [17], [18], [19], [20] |
| Failed messages | OpenHIM keeps every transaction with status (`Failed`, `Completed with error(s)` …), reruns from the console as tasks linked by `parentID`, and auto-retries per channel; bodies can be culled per channel. Mirth reprocesses from the message browser, optionally overwriting and choosing destinations. HAPI rejects with 422 + OperationOutcome and stores nothing. Azure `$import` writes OperationOutcome NDJSON to an error container; HealthLake writes SUCCESS/FAILURE folders and a manifest. | [21], [22], [23], [24], [25] |

## 2. Laboratory analysers

### 2.1 What is installed

- **Harmonized list (2017).** Table 5 of the MoH *National Standard Test Menu, Techniques and Supplies List for Laboratories* (March 2017) [1]. Equipment was first harmonized per level of care in 2011.

| Haematology | Chemistry | CD4 | Viral load / EID | TB and microbiology |
|---|---|---|---|---|
| Beckman Coulter 3/5-part diff | Roche cobas c111 | BD FACSCalibur | Abbott (m2000) | GeneXpert |
| Sysmex XP-300 | Roche cobas 6000 ("Cobas 600") | Partec CyFlow | Roche cobas CAP/CTM | Line probe assay |
| Sysmex pocH-100i | Roche cobas Integra / c311 | Point of care: Pima CD4 | Roche cobas 8800 | MGIT |
| Sysmex XS-1000i/500i/800i | Roche cobas e411 | BD FACSPresto | Point of care: Alere q, SAMBA, GeneXpert | BACTEC FX, bioMérieux Vitek |
| Sysmex XT-2000i | Human Humalyte Plus 3 | | | |
| Nihon Kohden Celltac 3/5-part diff | Human Humastar 80/100/200/600 | | | |
| HumaCount 5L, 30/60TS | | | | |

- **Levels.** The same document and the laboratory construction guideline put one haematology, one chemistry and one CD4 analyser in an HC IV laboratory; specialised tests (EID, viral load, microbiology) reach CPHL through the hub sample-transport network [1].
- **Unverified:** the current (2026) installed base. Mindray analysers are widely sold in East Africa and appear in BLIS and OpenELIS plugin lists [3], [4], but are not on the 2017 harmonized list. Whether the cobas 8800 and any Hologic Panther units are in service at CPHL today was not confirmed (a 2016–2017 MoH presentation lists 6 Abbott m2000 and 5 Roche CAP/CTM, with one cobas 8800 "on the way"; secondary [27]).

### 2.2 How they talk

- **Standards.** ASTM E1381 (low-level framing) and E1394 (records: header, patient, order, result, query, terminator) are now CLSI LIS01-A2 and LIS02-A2. HL7 v2 analysers normally use ORU^R01 for results and OML/ORM or QBP/RSP for orders and queries.
- **GeneXpert (Cepheid LIS Interface Protocol Specification 302-2261 Rev. C, 2020)** [2]:
  - covers GeneXpert Dx 4.7b and later, Omni, Xpress 5.1 and Infinity Xpertise 6.4b;
  - "implementing the ASTM E1394-97 protocol or the HL7 v2.5 protocol … both using ASTM E1381-02 low level protocol", so HL7 here is **not** MLLP-framed;
  - "built on top of TCP/IP", with the LIS and the GeneXpert PC "in the same network that is protected by a firewall";
  - flows: the instrument queries for all orders, the instrument sends a host query for one or more specimens, the instrument uploads results, and the host downloads unsolicited orders. Results are per specimen and test.
- **OpenELIS Global plugin inventory (v1.1.0, 2026-02-02)** [3]:

| Transport and protocol | Analysers |
|---|---|
| Exported file read from disk | Sysmex XT-2000i, XT-4000i, KX-21, XN-1000; Roche cobas c111, Integra 400, 4800, TaqMan 48/96 (VL and DBS); BD FACSCalibur, FACSCanto II, FACSPresto; GeneXpert (file variant); Applied Biosystems 7500 Fast, QuantStudio 3 and 7 Flex |
| ASTM over RS-232 or TCP | Sysmex XP, XN-L; GeneXpert; Horiba Micros 60, Pentra 60; pocH-100i; Stago STart 4; a generic ASTM plugin |
| HL7 over TCP | Mindray (BC-5380); Abbott Architect; Roche cobas 6800 VL; GeneXpert HL7 variant; a generic HL7 plugin |

- The inventory labels pocH-100i as Horiba ABX; it is a Sysmex product. Treat the inventory's vendor column with care.
- ASTM analysers reach OpenELIS through a separate bridge that relays between HTTP and ASTM (documented on port 12000) [5].
- **BLIS Interface Client** (`stevkky/BLISInterfaceClient`, last pushed 2022-08-26) [4]: "Interfacing BLIS with equipment using Protocols like RS232, TCP/IP, HTTP. It can also connect directly to analyzer database such as MS Access or even from a flat file." Formats: HL7, ASTM, CLSI LIS2-A and LIS2-A2, ABX and Vital proprietary formats. v2.6.12 lists 24 analysers, including Sysmex XS-500i, XT-2000i, XN-1000, KX-21N; Mindray BC-3600, BC-5380, BC-2800, BS-200E, BS-240, BS-300; BD FACSCalibur; GeneXpert; ABX Pentra/Micros; Selectra; Flexor; URIT.
- **BD** offers FACS Workflow Manager (RS-232 or TCP/IP, ASTM or HL7) and FACSLink (with Data Innovations Instrument Manager) as LIS middleware. Neither lists FACSPresto on the pages read (secondary marketing pages). **Unverified:** a native FACSPresto LIS link; OpenELIS reads its export file [3].
- **Unverified:** primary host-interface manuals for Sysmex XP/XS/XT, Mindray BC-5380, Roche cobas c111/c311, Abbott m2000 and Humastar. Only third-party copies were found (for example a Mindray manual stating "TCP/IP … with HL7 version 2.3.1 or ASTM E1394-97", and a cobas c311 manual citing E1381-91 and E1394-91). No "LabBench" middleware in use in Uganda was found.

### 2.3 Trade-offs that follow from the facts

- **Serial versus TCP.** Several harmonized analysers (older Sysmex, Horiba, pocH) are RS-232 only or file-export only. A link needs a PC or serial-to-IP converter beside the analyser; a TCP link needs the analyser on the facility LAN.
- **File import.** For many installed models the open-source engines read export files rather than a live protocol. That is one-way: no host query, no order download.
- **Bidirectional query** is specified for GeneXpert; for most other harmonized models the LIS-side behaviour is not documented in a primary source here.

## 3. National laboratory systems

### 3.1 CPHL viral load and EID exchange

- **User workflow** [8]: a clinician ticks viral load; the lab user enters the specimen ID (the serial number on the CPHL request form), picks the specimen type, ticks "REFER TEST" and selects "CPHL"; the request is sent "when synchronization occurs in the background"; results "will be automatically pulled by UgandaEMR when a result is availed by the CPHL dashboard. Requires active internet connection." Credentials for the CPHL Viral Load system are entered into two sync task types.
- **Code** (`METS-Programme/openmrs-module-ugandaemr-sync`, commit of 2026-07-10) [7]:
  - Task types `Send Viral Load To CPHL` → `https://ugisl.mets.or.ug/vl/sendbundledservicerequest`; `Send VL Program Data To CPHL` → `/vl-program/send`; `Request Viral Load Results form CPHL` → `/vl/request`. The default configuration embeds an `api_token` query parameter in each URL (not reproduced here).
  - The send template is a `ServiceRequest` with non-FHIR elements (`locationCode`, `locationReference`, `performerType: "????"`), `subject` as a `Location`, and a contained-style `Specimen` whose `identifier` and `type` are bare strings. It would fail a FHIR structural check.
  - The programme-data template is an `Observation` with a contained `Patient` and SNOMED-coded components (treatment start, regimen, duration, pregnancy, breastfeeding …), and identifier systems such as `http://health.go.ug/cr/internalid`.
  - The result request is the same ServiceRequest shape carrying the specimen identifier. The response is parsed as a Bundle: each `DiagnosticReport` and its `Observation`s are written to the encounter of the order resolved from `basedOn[0].reference`; values are read from `valueQuantity.value` or `valueString`.
  - A newer task (EMR 3.4.1) sends the full request-form data; the older one sends only sample identifiers [9].
- **Manual fallback** [9]: results are downloaded as CSV from the VL dashboard by date range and uploaded on a UgandaEMR sync page. Outcomes reported: patient not found, no matching encounter on the collection date, or uploaded.
- **Scale (secondary):** METS reports the Viral Load HIE runs through the Health Information Mediator with over 600 sites participating (newsletter, February 2024; the PDF now returns 404) [28]. A 2021 MoH/UNHLS presentation gave 560 ART sites with dashboard accounts, 290 (52%) downloading.
- **EID.** The sync code writes VL **or** EID results into the encounter, but no EID-specific task type or endpoint was found in the default configuration. **Unverified:** how EID requests are sent today.

### 3.2 ALIS, LDR and sample tracking

- **ALIS.** MoH, the HLIMS Technical Working Group and CDC "adopted and customized the C4G Basic Laboratory Information System (BLIS), rebranded as the Africa Laboratory Information System (A-LIS)". It produces HMIS 105 section 6, 033A and 033B, supply orders and referral-sample information [10]. First tested in Jinja, Masaka and Mubende RRHs in May 2017 ahead of a 12-facility pilot (secondary).
- **UgandaEMR ↔ ALIS.** The sync module has a `Send Lab Request to ALIS Task` profile (Patient, Person, Encounter, Observation, ServiceRequest to an `/api/alisdata` URL; the default is a private LAN address) and a `ReceiveLabResultFromALISTask` [7]. ClinicMaster also reports ALIS request/result exchange (MUWRP, secondary).
- **LDR.** CPHL describes the National Laboratory Data Repository as "a central digital exchange and analytics layer for Uganda's laboratory ecosystem", launched by MoH with APHL and CDC; it "feeds into the National Health Information System and National Health Data Warehouse"; facilities "send standardised data securely in real time"; access is role-based under MoH [11]. APHL: the LDR, a Sample Tracking App (hub coordinators, drivers, chain of custody, turnaround times) and a dashboard launched "in February"; "over 100 laboratories" connected; phased national rollout [12].
- **Not found:** a published LDR, sample-tracking or EID API, message standard or onboarding process. Whether facility EMRs (rather than LIS) can submit to or receive from the LDR is **unverified**.

## 4. FHIR R4 peers and the national HIE

- **MoH guidelines** (*Uganda Health Information Exchange and Interoperability Guidelines*, January 2023) [15]:
  - "For public health and clinical content messaging, FHIR and HL7 V2.5.1 and above shall be used", including "orders and results of laboratory tests".
  - CDA and CCD for clinical documents; DICOM for images; SNOMED CT, ICD-11 and LOINC ("LOINC … shall be the standard used for coding laboratory test reports").
  - Components defined: Client Registry, Electronic Health Facility Registry, Shared Health Record, terminology services, a Health Information Mediator (HIM).
  - **No FHIR version, IG, profile set or conformance process is named.** A compliance checklist "shall be developed" (not found).
- **UgISL is OpenHIM.** `https://ugisl.mets.or.ug/` serves the OpenHIM console; its `config/default.json` reads `"version": "1.18.2"`, `"minimumCoreVersion": "5.0.0"`, `"title": "Uganda InterOperability Service Layer (UgISL)"`, "Powered by OpenHIM", hosted by METS [14]. Whether a separate MoH-hosted HIM exists is **unverified**; `mediator-api-staging.health.go.ug` (behind Cloudflare) returned 404 for `/fhir/metadata`.
- **UgandaEMR sync profiles** (default configuration) [7]:

| Profile | Resources | Target (default) |
|---|---|---|
| Client Registry Integration | Patient | `mediator-api-staging.health.go.ug/fhir/Patient` |
| FSHR Integration | Patient, Encounter, Observation, Condition, AllergyIntolerance, DiagnosticReport, Immunization, MedicationDispense, MedicationRequest | a HAPI FHIR JPA server |
| National Data Ware House | Patient, Person, EpisodeOfCare, Encounter, Immunization, MedicationRequest, Observation, ServiceRequest, Practitioner, Condition, AllergyIntolerance, DiagnosticReport | `ugisl.mets.or.ug/central-server` |
| Cross Border Integration | Patient, Encounter, Observation | `hie.eac.int/fhir` |
| eCBSS Integration (disabled) | Encounter, Patient, Person, Observation | `services.fhir.hispuganda.org/fhir` |
| Send Lab Request to ALIS | Patient, Person, Encounter, Observation, ServiceRequest | facility ALIS |

- The module README states "HL7 FHIR R4"; it builds against OpenMRS FHIR2 2.8.0 and strips `contained` from generated JSON before sending.
- **OpenMRS FHIR2 module** [16]: latest tag 4.3.0 (main is 5.0.0-SNAPSHOT); `FhirVersionUtils` knows only R3 and R4; R4 providers: AllergyIntolerance, Condition, DiagnosticReport, Encounter, EpisodeOfCare, Group, Immunization, Location, Medication, MedicationDispense, MedicationRequest, Observation, Patient, Person, Practitioner, RelatedPerson, ServiceRequest, Task, ValueSet. The CapabilityStatement is at `/ws/fhir2/R4/metadata`.
- **Unverified:** whether the Client Registry and Shared Health Record are in national production; no national FHIR IG for Uganda was found on the FHIR registry.

## 5. Insurers and payers

- **Smart Applications International (Uganda)** [17]:
  - Smart Access app: the member starts a visit and gets a code on screen, by SMS or email; "Biller or hospital attendant inputs the code to get member details on the Smart System"; two fingerprints registered at first visit, fingerprint authentication plus OTP; existing biometric smart cards still work.
  - Online preauthorization with real-time notifications to scheme administrator, member and hospital; costs are deducted from benefits at the point of service.
  - A "Smart system" is installed at participating facilities. The page names no insurer and describes no API or EDI.
  - Reach (secondary, May 2023): 728 facilities in 92 districts [18].
- **Sanlam** (secondary): a Smart–Sanlam rollout of a web-based EDI for e-claim capture, reconciliation and submission [19].
- **Jubilee Health Insurance Uganda** (secondary, own newsletter June 2025): e-claims processing, SMS bill notifications, a member reimbursement portal [20]. No provider API described.
- **AAR, UAP Old Mutual, Prudential:** nothing found beyond press mentions of biometrics (AAR). **Unverified.**
- **NHIS:** passed in 2021, "not assented to by President Museveni, leaving its implementation pending"; government "plans to roll out the long-awaited National Health Insurance Scheme by 2027" (Daily Monitor, 4 November 2026) [26]. No claims platform, IT design or format is described.
- **Trade-off visible in the facts:** the dominant intermediary works through its own installed client and visit codes, not a published API, so an adapter may have to be a file or portal workflow or a vendor partnership. No Ugandan payer was found accepting FHIR Claim.

## 6. Failed inbound messages elsewhere

| System | Where failures appear | Correct and replay | Audit and retention |
|---|---|---|---|
| OpenHIM core | Every request is a transaction with status `Processing`, `Pending Async`, `Successful`, `Completed`, `Completed with error(s)` or `Failed`, plus error details, routes and orchestrations | Console reruns create tasks (`Queued`, `Processing`, `Paused`, `Cancelled`, `Completed`) in batches; a rerun is a new transaction whose `parentID` points to the original (`wasRerun`, `canRerun`); `txRerunAcl` limits who can rerun; channels can auto-retry (`autoRetryEnabled`, period default 60 min, max attempts, 0 = unlimited). Reruns replay the stored request; no edit step was found | ATNA-style audit store; per-channel `maxBodyAgeDays` (1–36,500) culls bodies but keeps metadata |
| Mirth Connect | Message browser per channel, with an Errors tab per connector message | Reprocess selected or all filtered messages; "Overwrite existing messages and update statistics"; choose destinations | Data pruner (off unless scheduled), per-channel "Prune Errored Messages"; 4.6+ is proprietary |
| HAPI FHIR | `RequestValidatingInterceptor` can add a header, add issues to the OperationOutcome, or fail with HTTP 422 | Caller resends; the server keeps no rejected payload | Optional interceptor records AuditEvent resources (BALP) |
| Azure Health Data Services `$import` | Job status `error.count` and `error.url`; NDJSON files of OperationOutcome (line and byte offset) in an `errorContainerName` container | Re-register with a new file name or ETag (registration is idempotent) | Successfully imported resources are not rolled back |
| AWS HealthLake import | `manifest.json`, `SUCCESS` and `FAILURE` folders; `SUCCESS_WITH_SEARCH_VALIDATION_FAILURES` at `minimal` level | New import job | Validation levels `strict` (default), `structure-only`, `minimal`; R4 only |

Sources: OpenHIM [21], Mirth [22], HAPI [23], Azure [24], HealthLake [25].

## 7. Sources

1. MoH / UNHLS, *National Standard Test Menu, Techniques and Supplies List for Laboratories*, March 2017, Table 5 "List of the current harmonized analyzers". <https://cphl.go.ug/uploads/library/1782711688387-d8c77a2c99d1.pdf>
2. Cepheid, *GeneXpert LIS Interface Protocol Specification* 302-2261 Rev. C (2020). <https://infomine.cepheid.com/sites/default/files/2021-10/LIS%20Protocol%20Specification%20302-2261%2C%20Rev.%20C.pdf>
3. `DIGI-UW/openelisglobal-plugins`, `analyzers/INVENTORY.md` v1.1.0 (2026-02-02). <https://github.com/DIGI-UW/openelisglobal-plugins/blob/main/analyzers/INVENTORY.md>
4. `stevkky/BLISInterfaceClient`, README. <https://github.com/stevkky/BLISInterfaceClient>
5. OpenELIS Global wiki, "ASTM Bi-Directional Analyzer Setup" and "Setup Analyzers". <https://uwdigi.atlassian.net/wiki/x/CwBPDg>, <https://uwdigi.atlassian.net/wiki/spaces/OG/pages/470810700/SETUP+ANALYZERS>
6. NextGen, "A new era for Mirth Connect" and 4.6.0 upgrade notes. <https://www.nextgen.com/blog/a-new-era-for-mirth-connect-by-nextgen-healthcare>, <https://docs.nextgen.com/en-US/mirthc2ae-connect-by-nextgen-healthcare-user-guide-3273569/4-6-0-upgrade-notes-203222>
7. `METS-Programme/openmrs-module-ugandaemr-sync` (commit 1b4dbe7, 2026-07-10): `README.md`; `api/src/main/resources/configuration/hie/synctasktype/*.json`; `.../syncprofile/*.json`; `server/SyncConstant.java` (`VL_SEND_SAMPLE_FHIR_JSON_STRING`, `VL_SEND_PROGRAM_DATA_FHIR_JSON_STRING`, `VL_RECEIVE_RESULT_FHIR_JSON_STRING`); `api/impl/UgandaEMRSyncServiceImpl.java` (`addTestResultsToEncounter`); `tasks/ReceiveLabResultFromALISTask.java`. <https://github.com/METS-Programme/openmrs-module-ugandaemr-sync>
8. UgandaEMR documentation, "Viral Load Integration". <https://mets-programme.gitbook.io/ugandaemr-documentation/data-exchange/viralloadexchange>
9. UgandaEMR documentation, "Uploading Viral Load Results"; UgandaEMR community, "Send VL to CPHL vs. Send VL Program Data to CPHL". <https://mets-programme.gitbook.io/ugandaemr-documentation/data-exchange/patient-data-upload/upload_vl_results>, <https://talk.ugandaemr.mets.or.ug/t/send-vl-to-cphl-vs-send-vl-program-data-to-cphl/315>
10. MoH / CPHL, *ALIS User Guide*; UNHLS eNotice Board, May 2017 (secondary for rollout). <https://cphl.go.ug/web/sites/default/files/2024-10/ALIS%20User%20Guide.pdf>, <https://www.cphl.go.ug/web/sites/default/files/2024-10/eNotice%20Board%20-%20May%202017.pdf>
11. CPHL, "National Laboratory Data Repository". <https://cphl.go.ug/ldr>
12. APHL, "Creating more modern laboratory systems in Uganda". <https://aphl.org/resources/resource-detail/creating-more-modern-laboratory-systems-in-uganda>
13. MUWRP, *Contribution to Uganda's eHealth agenda* (2026, secondary). <https://muwrp.org/wp-content/uploads/2026/07/MUWRP-CONTRIBUTION-TO-UGANDAS-eHEALTH-AGENDA-edited.pdf>
14. UgISL OpenHIM console, `https://ugisl.mets.or.ug/config/default.json` (fetched 2026-11-19). <https://ugisl.mets.or.ug/>
15. MoH, *The Uganda Health Information Exchange and Interoperability Guidelines*, January 2023, "Messaging Standard 1: HL7/FHIR" and "Vocabulary Standard 3: LOINC". <https://library.health.go.ug/file-download/download/public/1883>
16. `openmrs/openmrs-module-fhir2`: tags, `providers/r4/`, `web/util/FhirVersionUtils.java`. <https://github.com/openmrs/openmrs-module-fhir2>
17. Smart Applications International Uganda, "Smart Access". <https://uganda.smartapplicationsgroup.com/smart-access/>
18. Nile Post / allAfrica, "Medical insurance goes cardless as Smart launches mobile app", May 2023 (secondary). <https://allafrica.com/stories/202305180118.html>
19. Business Focus, Smart–Sanlam EDI rollout (secondary). <https://businessfocus.co.ug/?p=60043>
20. Jubilee Insurance Uganda, June 2025 newsletter (secondary). <https://jubileeinsurance.com/ug/wp-content/uploads/2025/06/June-Newsletter-2025.pdf>
21. `jembi/openhim-core-js` (2026-04-15): `src/model/transactions.js`, `src/model/channels.js`, `src/model/tasks.js`, `src/model/audits.js`, `src/tasks.js`, `src/middleware/rerun*.js`. <https://github.com/jembi/openhim-core-js>
22. `nextgenhealthcare/connect`, `client/.../ReprocessMessagesDialog.java`; NextGen user guide, "Message Browser Tasks" and "Message Pruning Settings". <https://github.com/nextgenhealthcare/connect>, <https://docs.nextgen.com/en-US/mirthc2ae-connect-by-nextgen-healthcare-user-guide-3231169/message-browser-tasks-14205>
23. HAPI FHIR, "Built-in server interceptors". <https://hapifhir.io/hapi-fhir/docs/interceptors/built_in_server_interceptors.html>
24. Microsoft Learn, "Import data into the FHIR service in Azure Health Data Services" (updated 2026-10-23). <https://learn.microsoft.com/en-us/azure/healthcare-apis/fhir/import-data>
25. AWS, "Importing FHIR data with AWS HealthLake". <https://docs.aws.amazon.com/healthlake/latest/devguide/import-datastore.html>

26. Daily Monitor, "Govt targets 2027 rollout of long-awaited health insurance scheme", 4 November 2026 (secondary). <https://www.monitor.co.ug/uganda/news/national/govt-targets-2027-rollout-of-long-awaited-health-insurance-scheme-5658826>
27. MoH, VL scale-up presentation (undated, secondary). <https://medbox.org/index.php/dl/5e148832db60a2044c2d36bd>
28. METS, *METS WATCH*, February 2024 (secondary; 404 on 2026-11-19). <https://mets.or.ug/wp-content/uploads/2024/03/METS-WATCH-Feb-2024.pdf>

## 8. Open questions

1. What is the current (2026) harmonized analyser list, and which models sit in HC IV and general hospital labs?
2. Does CPHL (or METS) publish a specification and onboarding process for the UgISL VL endpoints, the LDR, EID and sample tracking, for an EMR other than UgandaEMR and ClinicMaster?
3. Will UgISL accept valid FHIR in place of the current ServiceRequest shape, and which FHIR version?
4. Are the Client Registry and Shared Health Record in production, and under which host (MoH or METS)?
5. Does Smart Applications offer an integration API for a hospital EMR, or only its installed client?

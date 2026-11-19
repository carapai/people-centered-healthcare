# Research: DICOM worklists, image storage and teleradiology in Uganda

Findings for the research ticket [What do DICOM worklists, image storage and teleradiology require in Uganda?](https://github.com/carapai/people-centered-healthcare/issues/773). This note reports facts and decides nothing. It covers what [ADR 0079](../adr/0079-imaging-is-report-centred-and-the-platform-never-stores-dicom.md) and [imaging.md](../architecture/imaging.md) leave out of v1: the DICOM modality worklist, image storage, teleradiology, and the standards for sharing images across sites. It builds on the note on branch `research/clinical-operations` (`docs/research/clinical-operations.md`, §5 imaging and §6 FHIR fit) and does not repeat it: modalities by level, HMIS 056, the 2023 Mulago teleradiology centre, the 75 radiologists, the Orthanc licence and the FHIR maturity levels are there.

Research date: **2026-11-19**. Every source was read on that date unless a date is given. Claims marked **unverified** have no primary source yet. "Secondary" means a news item, study, vendor page or thesis that states the fact, not the owner of the fact (the standard body, the software project or the Ministry of Health (MoH)). Source numbers in square brackets refer to §7. Arithmetic marked "derived" is this note's own calculation from cited figures.

## 1. Summary

| Topic | Key facts | Main sources |
|---|---|---|
| Orthanc as worklist server | Orthanc core answers Modality Worklist C-FIND (SOP class 1.2.840.10008.5.1.4.31) only through a plugin. Options: the new **Worklists plugin** (AGPLv3+, packaged separately from November 2025) with a REST API `POST /worklists/create`, storage in a folder or in the Orthanc database (PostgreSQL or SQLite); the legacy sample plugin reading `.wl` files; a Python plugin callback (`RegisterWorklistCallback`, from Python plugin 3.2); or a C/C++ plugin. No FHIR feed exists in Orthanc; a Python plugin is the documented bridge to HL7, FHIR or a RIS. | [1], [2], [3], [4] |
| MPPS | Orthanc's DICOM conformance summary lists no MPPS (N-CREATE/N-SET) SCP. Storage commitment SCP and SCU exist since 1.6.0. | [5], [6] |
| Worklist fields | DICOM PS3.4 Table K.6-1 required matching keys: Scheduled Station AE Title, Scheduled Procedure Step Start Date and Time, Modality, Scheduled Performing Physician's Name, Patient's Name, Patient ID. Accession Number, Requested Procedure ID, Study Instance UID, Patient's Birth Date and Sex are optional matching keys; Study Instance UID is return type 1 (must be supplied). | [7] |
| Modalities | Conformance statements read: Philips Incisive CT (MWL, MPPS, storage commitment as SCU), Fujifilm FDX DR consoles (MWL, MPPS, storage commitment), Esaote Q7 ultrasound (MWL only, no MPPS). CT and DR copy the worklist Accession Number into the image. Which brands Uganda's RRH CT scanners and X-ray units are is **unverified**. | [8], [9], [10] |
| Matching studies | QIDO-RS servers must support Accession Number as a study-level matching attribute. FHIR R5 `ImagingStudy.basedOn` carries the accession number as an identifier of type `ACSN` on a ServiceRequest reference. | [11], [12] |
| Storage sizing | Orthanc recommends PostgreSQL once storage reaches "several hundreds of thousands" of instances, and reports deployments of 65 TB / 150 million instances. A published low-resource PACS (Bhutan, 2016) planned 250 MB per CT, 40 MB per MR and 15 MB per digital radiograph, uncompressed, halved by lossless compression. | [13], [14] |
| Retention | MoH *Health Data Protection, Privacy and Confidentiality Guidelines* (2023) §5.6: "All health data shall be retained for a minimum period of 5 (five) years from the date of creation before being archived"; organisations set periods per record type above that floor. No imaging-specific period was found. Data Protection and Privacy Act 2019 s.18 bars keeping data longer than needed unless law, purpose, contract or consent allows. | [15], [16] |
| Backup | Orthanc's own guidance: back up the configuration, the storage folder and the index; SQLite needs Orthanc stopped, PostgreSQL allows hot backups. `MaximumStorageSize` with the default `Recycle` mode silently deletes the oldest patients. | [13], [17], [18] |
| Teleradiology in Uganda | Public teleradiology is CT only, from RRHs to the Mulago centre (2023). Private facilities send all modalities, often to radiologists abroad (India named), through PACS or "Radweb" platforms, with no register of who reports. The software at Mulago is **unverified**; a 2025 thesis found 69.7% of respondents did not know it and some named "Teleradiology Solutions". | [19], [20] |
| Licensing of remote reporters | UMDPC's requirements (September 2025) make "Digital health (Telemedicine and telehealth) services" an add-on licence to a licensed private health unit. "All practitioners offering digital health services must be registered and licensed by the Council irrespective of their physical location and must be employed by a licensed private health unit located in Uganda." Store-and-forward of radiology images is named under telehealth. | [21] |
| Data leaving Uganda | Act s.19: storage outside Uganda needs adequate protection there or the data subject's consent. The MoH guidelines §5.4 add that processing outside Uganda needs PDPO authorisation. | [15], [16] |
| Bandwidth | Government bandwidth on the National Backbone costs USD 35 per Mbps per month since 1 August 2023 (was 70). A 250 MB CT study takes about 17 minutes at 2 Mbps (derived). Orthanc's Transfers Accelerator gzips buckets (2–3× smaller) and retries over HTTP(S), but needs Orthanc at both ends. | [22], [23] |
| Standards | DICOMweb PS3.18 (2026d): Studies Service (WADO-RS retrieve, STOW-RS store, QIDO-RS search), Worklist Service (UPS-RS), URI Service (WADO-URI). Orthanc implements WADO-URI, WADO-RS, QIDO-RS, STOW-RS, not UPS-RS. IHE RAD: SWF.b, XDS-I.b, WIA (DICOMweb) and IID final text; EBIW (point-of-care imaging without an order) trial implementation. FHIR endpoint connection types include `dicom-wado-rs`, `dicom-qido-rs`, `dicom-stow-rs`, `ihe-iid`. | [24], [25], [26], [27], [28] |

## 2. Modality worklist

### 2.1 How Orthanc serves a worklist

- **Design.** Orthanc is a C-FIND SCP for the Modality Worklist Information Model [5], but "an user-defined plugin must be provided to answer a list of worklists given a MWL query" [3]. The FAQ gives the reason: worklist generation is specific to each clinical flow and RIS [3].
- **Worklists plugin (new).** Source on GitHub as `orthanc-server/orthanc-worklists`, AGPLv3+ [1]. It "provides a REST API to create, delete and edit worklists"; the legacy page says it has been "packaged independently" from November 2025 [2]. Book changesets tie the REST API to Orthanc 1.12.10 (secondary, via search summary) [1]. The page documents:
  - `POST /worklists/create` with a JSON body of DICOM tags; `GET /worklists/{ID}`; `DELETE /worklists/{ID}`; `GET /worklists/?format=Simplify|Short|Full`. No modify call is documented.
  - Example body: `{"Tags": {"PatientID": "PID-45", "PatientName": "Toto", "ScheduledProcedureStepSequence": [{"Modality": "US", "ScheduledProcedureStepStartDate": "20251014", "ScheduledProcedureStepDescription": "Description"}]}}`.
  - Options: `Directory` (folder read from or written to), `SaveInOrthancDatabase` (worklists kept in the Orthanc database when it is PostgreSQL or SQLite; then they must be created through REST), `FilterIssuerAet` (match on the calling modality's AE title, for modalities that omit Scheduled Station AE Title), `LimitAnswers`, `SetStudyInstanceUidIfMissing`, `DeleteWorklistsOnStableStudy` (delete the worklist when a stable study with its Study Instance UID arrives; REST-created worklists only), `DeleteWorklistsDelay` (hours, default example 24), `HousekeepingInterval` (seconds).
- **Legacy sample plugin.** Shipped since Orthanc 1.0.0, "mainly a sample plugin to help developers write their own plugin". It serves `.wl` DICOM files from a folder, re-read on every C-FIND; files are made with DCMTK `dump2dcm` [2].
- **Python plugin.** `orthanc.RegisterWorklistCallback` from Python plugin 3.2, `RegisterWorklistCallback2` from 7.0. The book calls this "especially useful" for bridges between Orthanc, HL7/FHIR messages and RIS systems; no FHIR example is given [4].
- **Lua.** `IncomingWorklistRequestFilter` (since 1.4.2) can rewrite incoming worklist queries [2].
- **Version.** Latest Orthanc core is 1.13.1, dated 2026-11-17 [29].

### 2.2 MPPS and storage commitment

- Orthanc's conformance summary lists SCP support for C-ECHO, C-STORE, C-FIND (patient, study, worklist), C-MOVE and C-GET, and no MPPS SOP class (1.2.840.10008.3.1.2.3.3) [5]. The worklist plugin pages do not mention MPPS [1], [2]. A 2021 mailing-list post says MPPS was not supported (secondary) [30]. The Orthanc Book lists a third-party Docker package with "some MPPS and MWL features" (`sscotti/PACS_Integration`), not evaluated.
- "Starting with release 1.6.0, Orthanc implements DICOM storage commitment, both as SCP and as SCU" [6].

### 2.3 Fields a worklist entry needs

From DICOM PS3.4 Table K.6-1 [7]:

| Attribute | Tag | Matching key | Return type |
|---|---|---|---|
| Scheduled Procedure Step Sequence | (0040,0100) | R | 1 |
| > Scheduled Station AE Title | (0040,0001) | R (single value) | 1 |
| > Scheduled Procedure Step Start Date / Time | (0040,0002) / (0040,0003) | R (single or range) | 1 |
| > Modality | (0008,0060) | R (single value) | 1 |
| > Scheduled Performing Physician's Name | (0040,0006) | R | 2 |
| > Scheduled Procedure Step ID | (0040,0009) | O | 1 |
| Patient's Name | (0010,0010) | R (single or wildcard) | 1 |
| Patient ID | (0010,0020) | R (single value) | 1 |
| Patient's Birth Date | (0010,0030) | O | 2 |
| Patient's Sex | (0010,0040) | O | 2 |
| Requested Procedure ID | (0040,1001) | O | 1 |
| Study Instance UID | (0020,000D) | O | 1 |
| Accession Number | (0008,0050) | O | 2 |

Return type 1 means the worklist provider must supply a value; type 2 may be empty. Accession Number is DICOM VR SH (at most 16 characters) and AE titles are VR AE (at most 16 characters) (PS3.5 Table 6.2-1, not re-read for this note).

### 2.4 What common modalities support

No inventory of the brands and models in Ugandan public facilities was found (**unverified**). News reports describe the RRH CT scanners as 128-slice units costing about UGX 2.5 billion, without naming the maker (secondary) [31]. Conformance statements for representative models:

| Device | Type | MWL SCU | MPPS SCU | Storage commitment | Accession from MWL into image | Source |
|---|---|---|---|---|---|---|
| Philips Incisive CT Console v5.0 (2021) | CT | Yes | Yes | Yes (SCU) | Yes ("MWL, USER") | [8] |
| Fujifilm FDX Console DR-ID 300CL–900CL (October 2020) | DR X-ray | Yes | Yes | Yes (SCU) | Yes ("From MWL") | [9] |
| Esaote Q7 v1.1 | Ultrasound | Yes | No | No | Yes ("MWL/USER") | [10] |

- The Q7 lets the user query by Patient Name, Patient ID, Accession Number, Requested Procedure ID and exam date [10].
- Mindray manuals for DC-series ultrasound list DICOM Worklist and MPPS among DICOM functions (secondary, manual mirrors). Whether MWL and MPPS are paid options on any of these devices is **unverified**.
- CR digitisers and older non-DICOM X-ray units cannot query a worklist themselves; the CR console or reader does (Fujifilm publishes MWL and MPPS for its newer CR/DR consoles; secondary via search summary).

### 2.5 Matching returned studies by accession number

- QIDO-RS origin servers "shall support" Accession Number, Patient ID, Study Date, Modalities In Study and Study Instance UID as study-level matching attributes (PS3.18 Table 10.6.1-5) [11]. Orthanc's DICOMweb plugin implements QIDO-RS [24].
- When the modality fills Accession Number and Study Instance UID from the worklist [8], [9], [10], the returned study carries the same values the worklist provider created. Worklist Study Instance UID is return type 1 [7]; the Orthanc plugin's `SetStudyInstanceUidIfMissing` generates one [1].
- FHIR R5 `ImagingStudy`: "The study accession number can also be encoded as an `Reference.Identifier` using the ServiceRequest Reference type", with identifier type code `ACSN` from `http://terminology.hl7.org/CodeSystem/v2-0203`. The Study Instance UID goes in `identifier` with system `urn:dicom:uid` and value prefixed `urn:oid:`. Search parameter `based-on` covers "Accession Number associated with a ServiceRequest" [12].

## 3. Storage, retention and backup

### 3.1 Sizing

- **Study sizes.** Ratib et al., PACS for Bhutan (2016), Table 1, uncompressed: CT 250 MB, MR 40 MB, digital radiography 15 MB per study; "lossless compression can reduce these volumes by half". Their national referral hospital planned 6,000 CT, 4,000 MR and 5,000 radiography studies a year, about 1.7 TB. Ultrasound was archived only selectively and "hard to estimate" [14].
- **Derived example.** At those sizes, 5,000 radiographs and 2,000 CT a year would be about 75 GB + 500 GB = 575 GB uncompressed, about 290 GB with lossless compression. Actual Ugandan hospital volumes by modality are **unverified** (HMIS 056 counts films, see the clinical-operations note).
- **Orthanc scale.** No hard limit; production use reported at "more than 65TB of data, 340,000 studies and around 150 millions of instances". Move from SQLite to PostgreSQL "once Orthanc must store several hundreds of thousands of DICOM instances"; keep the index on SSD; XFS or Btrfs suggested; object storage plugins (MinIO with the S3 plugin is named) for heavy parallel I/O [13].
- **Compression options.** `StorageCompression` (transparent zlib, default false; the scalability FAQ advises false for performance). `IngestTranscoding` transcodes incoming instances to a set transfer syntax; lossy JPEG "will change the SOPInstanceUID" [13], [18].
- **Quotas.** `MaximumStorageSize` and `MaximumPatientCount` with `MaximumStorageMode` `Recycle` (default: oldest patients deleted) or `Reject` (new in 1.11.2) [18].

### 3.2 Retention rules

- **MoH guidelines (2023)** §5.6(a): "All health data shall be retained for a minimum period of 5 (five) years from the date of creation before being archived." Organisations "shall establish their policies" with "specific retention periods for different types of health records, such as medical records, diagnostic test results, treatment plans, and billing information". §5.6(b): disposal follows the Ministry of Public Service and the National Records and Archives Act 2001. §5.5(d): "Digital Health Information shall be encrypted" [15].
- **Data Protection and Privacy Act 2019** s.18: no retention longer than necessary unless required by law, needed for a lawful purpose, required by contract or consented; data used to decide about a person kept for the period the law sets or long enough for the person to request access; destroy or de-identify at the end [16].
- **Facility standards** (clinical-operations note): "film storage per guidelines". No MoH or Atomic Energy Council rule giving a retention period for images or films was found (**unverified**).
- The teleradiology study reports providers' concern over "who controls and retains patient records, especially when data is stored by external vendors", and whether patients returning years later find data deleted [19].

### 3.3 Backup

- Orthanc Book: back up the configuration file, the storage folder and the index. With SQLite, stop Orthanc first; with PostgreSQL, hot backups work. Backups taken on a running system may not match; "resend recent data, such as the last 24 hours, from the modalities", and set `OverwriteInstances` to true. Restores can leave orphan files (secondary, via search summary of the backup page) [17].
- Scalability FAQ: "make sure that the filesystem is properly backed up, and that technologies such as RAID are enabled" [13].

## 4. Teleradiology in Uganda

### 4.1 Arrangements found

- **Public.** "In public health facilities, the teleradiology modalities are limited to CT scans that have been recently installed in the regional referral hospitals", sent to the Mulago centre [19]. No MoH document describing the centre's software, network, volumes or turnaround was found (**unverified**).
- **Mulago software.** A 2025 Makerere undergraduate dissertation (33 respondents, mostly radiography students): 69.7% were unsure which system was used; a few named "Teleradiology Solutions (TRS)" (secondary) [20].
- **Private workflow** [19]: registration and request in the hospital system, payment confirmed, demographics "crossed into RIS", images to "PACS or Radweb", radiologist logs into Radweb to report, report back into the hospital system. Private facilities send CT, MRI, X-ray, ultrasound, fluoroscopy and mammography, mostly cross-sectional; on holidays "all images". Some send "to consultants out there in different countries but more often India", with "no register of who is doing teleradiology".
- **Phone photos.** A nephrologist: "when I make requests, some of them are taken pictures of, and they send them out and they send us reports" [19].
- **Vendors (secondary, promotional).** Manipal Hospitals Radiology Group advertises teleradiology for Uganda, including remote ultrasound reporting for a Kampala clinic; Africa Telerad offers cloud PACS hosted in Nairobi [32]. Neither was verified.
- **Participants asked for** a RIS integrated with the hospital system and PACS, financial tracking against fraud, individual logins, patient consent for teleradiology, and countersigning by local radiologists [19].

### 4.2 Licensing of remote reporters

- **UMDPC requirements** (PDF dated 9 September 2025), §23 "Requirements for registration of digital health units" [21]:
  - Telemedicine is "real-time interaction between the medical practitioner (Doctor) and the patient", continuing a relationship begun in person within twelve months.
  - Telehealth covers non-clinical services, including "Store-and-forward platforms used to transmit patient-level data, such as medical images… commonly used in radiology images."
  - "No person or body shall operate a digital health unit or service without first obtaining a license from the Council." Digital health licences are "issued as an addition to an existing license for a private health unit."
  - "All practitioners offering digital health services must be registered and licensed by the Council irrespective of their physical location and must be employed by a licensed private health unit located in Uganda."
  - Units must show PDPO compliance and follow MoH digital health guidelines; the Council inspects "telecommunication infrastructure".
- **Specialised Diagnostic Imaging Unit** (§15): "Supervised by a medical practitioner registered as a radiologist by the Council", responsible for "interpretation of imaging results" [21].
- How these rules apply to public facilities and to the Mulago centre is **unverified**; the document addresses private units. Study participants said foreign teleradiologists "are not regulated by UMDPC" [19].

### 4.3 Data leaving the country

- Act s.19: a controller storing data outside Uganda must ensure the country has protection "at least equivalent" to the Act, or the data subject has consented [16].
- MoH guidelines §5.4: no processing or storage outside Uganda without "authorisation from the PDPO", and the destination must have adequate protection [15].

### 4.4 Bandwidth

- NITA-U: government bandwidth on the National Backbone Infrastructure fell "from 70$ to 35$ per Mbps per month" from 1 August 2023; the backbone connects about 1,466 government offices [22]. How many hospitals are connected, and at what speed, is **unverified** (a NITA-U strategic plan cited by a search summary says five regional hospitals; the PDF failed to download).
- **Derived.** One 250 MB CT study [14] takes 250 × 8 / 2 = 1,000 s (about 17 minutes) at a sustained 2 Mbps, about 3.5 minutes at 10 Mbps, before protocol overhead.
- Orthanc Transfers Accelerator: groups small instances into buckets (default 4 MB), splits large ones, gzip "by a factor of 2 to 3", jobs can pause and resubmit, `MaxHttpRetries` per bucket, MD5 check per instance; both ends must run Orthanc with the plugin [23].
- Orthanc DICOMweb client: `POST /dicom-web/servers/{name}/stow` sends studies to a remote DICOMweb server over HTTP(S), asynchronously as a job if asked, with Basic, bearer-header or client-certificate authentication [24].
- MoH SP II plans to "Functionalize teleradiology services at all regional referral hospital and GH (internet and renumeration)" (clinical-operations note).

## 5. Standards

### 5.1 DICOMweb (PS3.18, 2026d)

- Chapter 9 URI Service (WADO-URI); chapter 10 Studies Service: Retrieve (WADO-RS, 10.4, including rendered and thumbnail resources), Store (STOW-RS, 10.5), Search (QIDO-RS, 10.6); chapter 11 Worklist Service (UPS-RS); chapter 12 Non-Patient Instances [25].
- Orthanc DICOMweb plugin: "basic, reference implementation of WADO-URI, WADO-RS, QIDO-RS and STOW-RS"; "The full standard is not implemented yet". It does not mention UPS-RS. Client and server in one plugin. Its licence is not stated on the page [24].

### 5.2 IHE Radiology

| Profile | Status (RAD TF 23.0) | What it does | Source |
|---|---|---|---|
| Scheduled Workflow (SWF.b) | Final text | Orders to RIS, worklist to modality, MPPS status to RIS and PACS, storage commitment for custody; HL7 v2.5.1 | [26], [27] |
| Cross-Enterprise Document Sharing for Imaging (XDS-I.b) | Final text | A DICOM manifest (and PDF/CDA reports) registered in XDS; images stay at the source and are fetched by RAD-69 (SOAP) or WADO; "images… are not transferred to the Document Repository" | [26], [27] |
| Web-based Image Access (WIA) | Final text (added 2024) | QIDO-RS and WADO-RS access to studies over HTTP(S), neutral about the backend; options to front XDS-I or MHD | [26], [27] |
| Invoke Image Display (IID) | Final text (added to TF 2025) | An EHR opens a PACS viewer with an HTTP GET carrying `requestType` (PATIENT or STUDY), `patientID`, `accessionNumber` or `studyUID`, plus viewer options | [26], [27] |
| Encounter-Based Imaging Workflow (EBIW) | Trial implementation (revised 2025-06-19) | Point-of-care imaging without a prior order: bedside ultrasound, scopes, camera or phone photos; linking images to the encounter | [26], [27] |
| Import Reconciliation Workflow (IRWF.b) | Trial implementation | Importing outside studies and reconciling identifiers | [26] |

### 5.3 FHIR R5

- `ImagingStudy` (maturity 4, trial use): "SHALL reference one DICOM Study"; "The DICOM instances are not stored in the ImagingStudy resource"; it "will only eliminate the need for DICOM query (e.g., QIDO-RS) in the simplest cases"; `basedOn` may be ServiceRequest, CarePlan, Appointment, AppointmentResponse or Task; status `registered | available | cancelled | entered-in-error | unknown`; a series endpoint overrides a study endpoint of the same connection type [12].
- Endpoint connection types (HL7 Terminology CodeSystem `endpoint-connection-type` 2.1.1): `dicom-wado-rs`, `dicom-qido-rs`, `dicom-stow-rs`, `dicom-wado-uri`, `ihe-iid`, `ihe-xds`, `ihe-xca`, `ihe-xdr`, `ihe-xcpd` [28].
- `ImagingSelection` (maturity 1): key images and regions within one study; see the clinical-operations note §6.

## 6. Open questions

1. What software, network and reporting rota does the Mulago teleradiology centre use, which RRHs send to it, and what is its turnaround?
2. Which makes and models are the RRH CT scanners and hospital X-ray and ultrasound units, and are MWL and MPPS licensed on them?
3. Does any MoH, Atomic Energy Council or National Records rule set a retention period specifically for images, films or radiation dose records beyond the 5-year floor?
4. Do UMDPC's digital health licensing rules apply to public facilities and to radiologists reporting from abroad for a public hospital?
5. What sustained bandwidth do RRHs and general hospitals have, and how many are on the National Backbone?
6. What are imaging volumes by modality at a typical RRH and general hospital, to size storage?
7. The licence of Orthanc's DICOMweb plugin, and whether any maintained Orthanc add-on provides an MPPS SCP.

## 7. Sources

1. Orthanc Book, "Worklists plugin" (`orthanc-server/orthanc-worklists`, AGPLv3+). <https://orthanc.uclouvain.be/book/plugins/worklists-plugin-new.html> ; book changesets (via search summary) <https://orthanc.uclouvain.be/hg/orthanc-book/rev/a2c1ac6c5b6b>
2. Orthanc Book, "Sample Modality Worklists plugin (legacy)". <https://orthanc.uclouvain.be/book/plugins/worklists-plugin.html>
3. Orthanc Book FAQ, "Modality worklists". <https://orthanc.uclouvain.be/book/faq/worklist.html>
4. Orthanc Book, "Python plugin for Orthanc", section "Handling worklist SCP requests". <https://orthanc.uclouvain.be/book/plugins/python.html>
5. Orthanc source, `OrthancServer/Resources/DicomConformance.txt` (DICOM Conformance Summary of Orthanc), default branch. <https://orthanc.uclouvain.be/hg/orthanc/raw-file/default/OrthancServer/Resources/DicomConformance.txt>
6. Orthanc Book, "DICOM storage commitment". <https://orthanc.uclouvain.be/book/users/storage-commitment.html>
7. DICOM PS3.4 (current), §K.6.1.2.2, Table K.6-1 "Attributes for the Modality Worklist Information Model". <https://dicom.nema.org/medical/dicom/current/output/chtml/part04/sect_K.6.html>
8. Philips, *DICOM Conformance Statement Incisive CT – Console v5.0*, HSDP-678997, 2021: SOP class table, worklist query keys, General Study Module. <https://www.documents.philips.com/assets/DICOM%20Conformance%20Statement/20220223/794deac03d6b4314bcbbae450111c945.pdf>
9. Fujifilm, *DICOM Conformance Statement FDX Console DR-ID 300CL/330CL/700CL/800CL/900CL*, 18th edition, October 2020. <https://healthcaresolutions-us.fujifilm.com/wp-content/uploads/2023/02/897N101668P_DR-ID-300CL-DICOM-Conformance-Statement-Standard-18th-Edition_Z72N2185P.pdf>
10. Esaote, *DICOM Conformance Statement Q7*, version 1.1: Table 1, §2.3. <https://www.esaote.com/fileadmin/GLOBAL/Ultrasound/Products/Q7/DICOM_CONFORMANCE_STATEMENT_Q7.pdf>
11. DICOM PS3.18 (current), §10.6.1.2.3, Table 10.6.1-5 "Required Matching Attributes". <https://dicom.nema.org/medical/dicom/current/output/chtml/part18/sect_10.6.html>
12. HL7 FHIR R5 (5.0.0), ImagingStudy: resource content, notes, search parameters. <https://hl7.org/fhir/R5/imagingstudy.html>
13. Orthanc Book FAQ, "Scalability of Orthanc". <https://orthanc.uclouvain.be/book/faq/scalability.html>
14. Ratib O., Roduit N., Nidup D., De Geer G. et al., "PACS for Bhutan: a cost effective open source architecture for emerging countries", *Insights into Imaging* 7 (2016), doi:10.1007/s13244-016-0512-7, Table 1 (full text via Europe PMC, PMC5028341). <https://pmc.ncbi.nlm.nih.gov/articles/PMC5028341/>
15. Uganda MoH, *The Uganda Health Data Protection, Privacy and Confidentiality Guidelines*, 2023 (file dated 16 January 2023): §5.4–5.6. <https://library.health.go.ug/sites/default/files/resources/Health%20Data%20Protection%20and%20Confidentiality%20Guidelines%20for%20Uganda_16012023.pdf>
16. *The Data Protection and Privacy Act, 2019* (Act 9 of 2019), ss. 18–19 (Parliament copy). <https://bills.parliament.ug/attachments/Data%20Protection%20and%20Privacy%20Act%202019.pdf>
17. Orthanc Book, "Backup" (content via search summary of the page and its source history; secondary). <https://orthanc.uclouvain.be/book/users/backup.html>
18. Orthanc source, `OrthancServer/Resources/Configuration.json`, default branch: `StorageCompression`, `MaximumStorageSize`, `MaximumStorageMode`, `IngestTranscoding`. <https://orthanc.uclouvain.be/hg/orthanc/raw-file/default/OrthancServer/Resources/Configuration.json>
19. Eyotaru F. et al., "Current practices and implementation considerations for teleradiology services in Uganda: a qualitative study of health care providers' perspectives", Research Square preprint, 29 June 2025 (17 providers, mostly private; secondary). <https://www.researchsquare.com/article/rs-6976547/v1>
20. Kabuuka B., *Assessing the impact of Teleradiology on diagnostic accuracy and turn around time at Mulago National Referral Hospital*, undergraduate dissertation, Makerere University, 2025 (abstract; secondary). <https://dissertations.mak.ac.ug/handle/20.500.12281/21746>
21. Uganda Medical and Dental Practitioners Council, *Requirements for different categories of registration and licensure*, PDF created 9 September 2025: §15 (Specialised Diagnostic Imaging Unit), §23 (digital health units). <https://umdpc.go.ug/downloads/requirements/requirements.pdf>
22. NITA-U, "NITA Uganda reduces the cost of Internet for Government" (August 2023). <https://www.nita.go.ug/news-updates/nita-uganda-reduces-cost-internet-government>
23. Orthanc Book, "Transfers accelerator plugin". <https://orthanc.uclouvain.be/book/plugins/transfers.html>
24. Orthanc Book, "DICOMweb plugin". <https://orthanc.uclouvain.be/book/plugins/dicomweb.html>
25. DICOM PS3.18 2026d, *Web Services*, table of contents. <https://dicom.nema.org/medical/dicom/current/output/chtml/part18/ps3.18.html>
26. IHE Radiology, profiles and Technical Framework index (RAD TF Rev. 23.0). <https://profiles.ihe.net/RAD/>
27. IHE wiki profile pages: Scheduled Workflow.b <https://wiki.ihe.net/index.php/Scheduled_Workflow.b> ; XDS-I.b <https://wiki.ihe.net/index.php/Cross-enterprise_Document_Sharing_for_Imaging> ; WIA <https://wiki.ihe.net/index.php/Web-based_Image_Access> ; IID <https://wiki.ihe.net/index.php/Invoke_Image_Display> ; EBIW <https://wiki.ihe.net/index.php/Encounter-based_Imaging_Workflow>
28. HL7 Terminology, CodeSystem `endpoint-connection-type` 2.1.1. <https://terminology.hl7.org/CodeSystem-endpoint-connection-type.html>
29. Orthanc source downloads. <https://orthanc.uclouvain.be/downloads/sources/orthanc/index.html>
30. comp.protocols.dicom / Orthanc users post on MPPS, 2021 (via search summary; secondary). <https://groups.google.com/g/comp.protocols.dicom/c/XzDCRlXHoVU>
31. News on RRH CT scanners: The Independent, "Government installs first CT machine in northern Uganda"; Watchdog Uganda, "Kabale hospital CT scan lying idle 7 months after installation", 2 April 2023 (both via search summary; secondary). <https://independent.co.ug/government-installs-first-ct-machine-in-northern-uganda> ; <https://watchdoguganda.com/news/20230402/151699/kabale-hospital-ct-scan-lying-idle-7-months-after-installation.html>
32. Manipal Hospitals Radiology Group, "Teleradiology Reporting Services in Uganda"; iColo, "Teleradiology Cloud Solution From Anywhere in Africa" (both promotional, via search summary; secondary). <https://radiologygroup.manipalhospitals.com/teleradiology-reporting-services-uganda> ; <https://icolo.io/?p=485>

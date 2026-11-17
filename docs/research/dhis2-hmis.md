# Research: Uganda HMIS and DHIS2 reporting

Findings for the research ticket [How does Uganda's HMIS expect facility data, and how do platforms send it to DHIS2?](https://github.com/carapai/people-centered-healthcare/issues/527). It feeds #529. In the architecture, DHIS2 is a central-only integration: `interoperability-service` runs `health-integration` adapters ([service-catalogue.md](../architecture/service-catalogue.md), [deployment-topology.md](../architecture/deployment-topology.md)). Nothing here is a decision. Claims marked **unverified** have no primary source yet. Sources are numbered at the end and were read in November 2026.

## Summary

- **The national HMIS is DHIS2.** Uganda's eHMIS runs on DHIS2 at `hmis.health.go.ug` (the login page answered when checked; MoH's DHIS2 user manual describes data entry, the Complete button and completeness reports [27]). MoH guidelines say digital solutions used for data collection "are required to transmit the defined HMIS data set to eHMIS/DHIS2" [1].
- **The format is aggregate.** Facility HMIS reporting is aggregate data sets: one value per data element × category option combo × org unit × period. Weekly surveillance is the same shape. No source found requires case-based tracker data from facility EMRs for HMIS.
- **The organisation unit is the key.** Every value is keyed by the facility's DHIS2 organisation unit. Uganda's National Health Facility Registry (NHFR) publishes a public API that is DHIS2-shaped. That API returns 7,330 level-6 facilities, each with a registry UID, an NHFR identifier, a "historical or DHIS2" id, and a `reporting` flag [2].
- **UgandaEMR posts JSON to `dataValueSets`.** It builds a `dataValueSets` JSON payload per report from a template that carries the DHIS2 UIDs of the data set, data elements and category option combos. A user previews the payload, then it is POSTed with Basic auth, either directly to DHIS2 or through the METS interoperability layer (`ugisl.mets.or.ug/ehmis`) [3][4][5].
- **No FHIR path into the HMIS exists today.** DHIS2 core has no native FHIR aggregate endpoint. DHIS2 recommends Apache Camel facades [6], and the old FHIR Adapter is deprecated [7]. IHE mADX (FHIR MeasureReport) is a trial-use standard [26], not something Uganda's instance accepts. **Unverified:** whether Uganda runs any mADX or FHIR gateway.

## Facility reporting forms

| Form | Content | Frequency | Facility deadline | Channel today |
|---|---|---|---|---|
| HMIS 033a | Notifiable disease report | Immediate (within 24 h) | Immediately [8] | Phone or paper; eIDSR (**unverified**) |
| HMIS 033b | Weekly epidemiological surveillance (epidemic-prone diseases, maternal and perinatal deaths) | Weekly | Monday of the following week [8]. Counted timely if in DHIS2 by 12:00 Monday, complete by 12:00 Wednesday [9] | mTRAC SMS (6767), synchronised into DHIS2 [9] |
| HMIS 105 | Health unit outpatient monthly report (OPD, maternal, EPI, FP, HTS, lab, stock status §6, …) | Monthly | 7th of the following month to the HSD [8][10]. Analyses of DHIS2 timeliness use the 15th [11] | Paper to the district, entered into DHIS2 there [11]; EMR push where an EMR is installed [4] |
| HMIS 108 | Health unit inpatient monthly report | Monthly | 7th of the following month [8][10] | As for 105 |
| HMIS 106a | Quarterly report. In current versions this is the HIV care and ART quarterly report; UgandaEMR has `106A1A` 2019 and 2024 templates | Quarterly (July–June financial year) | 7th of the month after the quarter: Oct, Jan, Apr, Jul [8] | As for 105 |
| HMIS 106b | Quarterly assessment | Quarterly | As for 106a [8] | **Unverified** |
| HMIS 107 | Annual report; 101–103 inventories and staff listing | Annual | 7 August [8] | **Unverified** |

Notes:

- **The deadlines are from the 2010 Health Unit Procedure Manual** [8], which is the latest procedure manual found online. The forms were revised in 2014 [12], and UgandaEMR ships separate 2019 and 2024 templates [5]. Data set and data element UIDs therefore change between form versions. A current (2020+) procedure manual and reporting calendar were not found: **unverified**.
- **The 15th-of-the-month figure** comes from published DHIS2 timeliness analyses [11]. It is probably the "days after period to qualify for timely submission" setting on the instance's data sets (**unverified**).

## The DHIS2 aggregate API a central adapter would implement

| Purpose | Endpoint | Key facts |
|---|---|---|
| Push values | `POST /api/dataValueSets` (JSON, XML, CSV, or ADX with `application/adx+xml`) | Body: `dataSet`, `period`, `orgUnit`, `attributeOptionCombo`, `completeDate`, `dataValues[{dataElement, categoryOptionCombo, value, comment}]`. Parameters: `importStrategy=CREATE_AND_UPDATE` (default), `dryRun`, `async`, `idScheme`, `orgUnitIdScheme`, `dataElementIdScheme`, … (`uid` by default; `code` by default for ADX), `force` (superuser only). The response is an import summary with `imported`, `updated`, `ignored` and `deleted` counts and conflicts [13] |
| Single value | `POST`/`DELETE /api/dataValues?de=&pe=&ou=&co=&cc=&cp=` | For corrections to one cell [13] |
| Read back / reconcile | `GET /api/dataValueSets?dataSet=&period=&orgUnit=&lastUpdated=&includeDeleted=` | Deletes are soft (`deleted=true`) [13] |
| Mark complete | `POST`/`DELETE /api/completeDataSetRegistrations` | `{dataSet, period, organisationUnit, attributeOptionCombo}`. Completeness and timeliness reports count these [14][15] |
| Approval status | `GET /api/dataApprovals?wf=&pe=&ou=&aoc=`, `/api/dataApprovals/approvals`, `/api/dataAcceptances` | Once approved, the data set "will be locked for that period and organisation unit, and any further data entry or modification will be prohibited unless it is first un-approved". Accepted data cannot change until it is unaccepted [16][17] |
| Locks | Data set `expiryDays`, data input periods, `/api/lockExceptions?ds=&pe=&ou=` | Expiry days close the period N days after it ends. A lock exception reopens one data set × period × org unit [18][19] |
| Metadata pull | `GET /api/dataSets/{uid}?fields=…`, `/api/organisationUnits`, `/api/categoryOptionCombos` | Needed to validate mappings before sending (standard metadata API) |
| Case-based (not HMIS) | `POST /api/tracker` (`trackedEntities`, `enrollments`, `events`; async by default; `atomicMode=ALL`) | `/api/events` and `/api/trackedEntityInstances` were removed in v42 [20]. Only relevant if MoH asks for case-based surveillance such as eIDSR or a TB tracker (**unverified** for Uganda) |

**Periods.** Monthly `yyyyMM`, quarterly `yyyyQn`, weekly `yyyyWn` and Financial July `yyyyJuly`. ADX uses ISO 8601 `start/duration`, for example `2017-07-01/P1Y` [13][21]. Uganda's financial year starts in July. **Unverified:** whether the Uganda instance's weekly period type is ISO (Monday-start) weeks.

## Facility identity (org unit matching)

- **The NHFR API** is at `https://nhfr.health.go.ug/api/v1/orgunits?level=6&reporting=true` and was public, without auth, when tested on 2026-11-17 [2]. Each facility record carries:
  - `mfl_uid`: an 11-character registry UID;
  - `identifier`: the NHFR number;
  - `historical_id`: documented as "historical or DHIS2 facility identifier";
  - `path`, the parents (region, district, DLG, subcounty), `facility_level`, `ownership`, `authority`, `status` and `reporting`.
- **The NHFR feeds DHIS2.** Its admin screens include an "MFL Integrator" and a "registry-owned DHIS2 API publisher" [2]. **Unverified:** whether eHMIS org unit UIDs equal `mfl_uid` or `historical_id`. In the sample record, `historical_id` equals the last element of `path`.
- **UgandaEMR** stores the facility's DHIS2 org unit UID as a global property (`ugandaemr.dhis2.organizationuuid`) and substitutes it for `orgUnit` in every payload [5][22].
- **Implication for us.** Each facility needs a mapping from its platform facility code ([identifiers.md](../architecture/identifiers.md)) to its DHIS2 org unit UID, set at central and checked against the NHFR. This is a candidate for national configuration or facility settings (ADR 0067).

## How EMRs submit today

- **UgandaEMR** (METS) [3][4][5]:
  - Reports are OpenMRS report definitions plus a JSON template per form version (for example `106A1A2024Report.json`, `HMIS_105_HTS_2024.json`). The template carries `dataSet`, `attributeOptionCombo` and per-cell `dataElement`/`categoryOptionCombo` UIDs, with placeholders for values.
  - The server fills the values and the period. The period is computed from the end date as `yyyyQn` in the 106a path.
  - A user runs the report, reviews a preview, and clicks Send. There is no unattended schedule for HMIS reports.
  - The POST uses Basic auth. Credentials and URL are set per facility in global properties.
  - The `Send HMIS Reports` task type targets `http://ugisl.mets.or.ug/ehmis`. PEPFAR MER reports go to `/adx-mer` on the same host, which is a mediator, not DHIS2 itself.
  - Resubmission means re-running and re-sending. Because of `CREATE_AND_UPDATE`, DHIS2 counts the second send as `updated`.
- **OpenMRS generally:** the DHIS Connector module (Jembi) maps Reporting-module indicators to DHIS2 data sets and posts `dataValueSets`, with ADX support [23]. **Unverified** in detail.
- **Manual entry:** most public facilities still report on paper, keyed in at the district or HSD [11].
- **Weekly 033b** goes by mTRAC SMS, not from EMRs [9].

## Corrections and resubmission

- **Idempotent overwrite.** Re-POSTing the same data element × category option combo × attribute option combo × period × org unit overwrites the value. Identical values are ignored. An empty value or `importStrategy=DELETE` soft-deletes, and DHIS2 audits every change [13]. A correction is therefore "resend the whole data set for that period", and the import summary shows the delta.
- **Locks.** A correction fails when:
  - the period is past `expiryDays` or outside the data input period;
  - the data is approved or accepted at any level above;
  - the data set registration is complete and the instance has "complete data set locks entry" set (**unverified** for Uganda).
  The fix is a lock exception or an un-approval by the district or MoH, or `force` (superuser only) [13][16][19]. The adapter must surface these conflicts as human tasks, not retry them.
- **Zeros.** Zeros are stored only for data elements marked "zero is significant" [24]. Otherwise a zero is dropped, which matters when correcting a non-zero value to zero.
- **Uganda's approval workflow.** Whether eHMIS uses DHIS2 data approval for HMIS data sets, and at which levels (HSD, district, MoH), is **unverified**.

## Governance, legal and credentials

- **Solution approval.** Under the MoH *Guidelines for the Introduction of Digital Health Solutions and Innovations* (January 2023), every digital health solution must follow the *Uganda Digital Health Enterprise Architecture, Standards and Knowledge Guidelines*. The approval route [1]:
  1. Register with the Digital Health Subcommittee: `dhs@health.go.ug` or the Digital Health Tools Registry.
  2. Present to the DHSC.
  3. HIIRE TWG approval, for a pilot only.
  4. Senior Management Committee.
  5. Top Management, for scale-up.
  Systems already deployed without approval must go through the same steps [1].
- **Credentials.** UgandaEMR uses one DHIS2 account per facility (Basic auth), and MoH issues each facility's DHIS2 UID [4][22]. A central adapter that pushes for many facilities needs one service account whose user org units cover those facilities, with data-entry authority on each data set. Who issues that account, and whether MoH allows a single integration user, is **open**.
- **The eHealth Policy (2016)** calls for data-sharing agreements that define the purposes for which health data may be shared [25]. No published MoU template or API access process for eHMIS was found: **open**.
- **Aggregate data only.** HMIS submissions are aggregate. Pushing to DHIS2 sends no PII, but small-cell disclosure rules, if any, are **unverified**.

## Open questions for #529

1. Must the adapter post to `hmis.health.go.ug` directly, or through an MoH or METS mediator (`ugisl`, an OpenHIM)? What auth applies: Basic, a Personal Access Token, or OAuth2?
2. Where do current data set, data element and category option combo UIDs come from? Pulling them from the instance's metadata API, or a versioned MoH metadata package, would let mappings ship as configuration packages ([configuration.md](../architecture/configuration.md)) with effective periods per form revision.
3. Which approval workflow, expiry days and timely-submission days apply to each HMIS data set on the Uganda instance?
4. Does a facility's EMR submission replace the district's manual entry, or do they conflict? Who is the "storedBy" of record?
5. Case-based surveillance (eIDSR, TB, HIV case surveillance): is there a DHIS2 tracker program MoH expects EMRs to feed?
6. How does the MoH approval (DHSC → HIIRE TWG → SMC → Top Management) sequence with pilot deployments?

## Sources

1. Uganda MoH, *Guidelines for the Introduction of Digital Health Solutions and Innovations in Uganda*, January 2023, §1.1.3 and §2. https://library.health.go.ug/file-download/download/public/1886
2. Uganda National Health Facility Registry: the public API (`/api/v1/orgunits`, tested 2026-11-17) and the API reference embedded in the web app. https://nhfr.health.go.ug
3. UgandaEMR user manual, "Sending Report to DHIS2" and "Sending Aggregate Reports". https://mets-programme.gitbook.io/ugandaemr-documentation/reporting/sending_to_dhis2 and https://mets-programme.gitbook.io/ugandaemr-documentation/data-exchange/sending_aggregate_reports_to_dhis2
4. `METS-Programme/openmrs-module-ugandaemr-sync`: `SendDHIS2DataToCentralServerTask.java`, `SendReportsTask.java`, `UgandaEMRSyncConfig.java`, `configuration/hie/synctasktype/synctasktype-send-hmis-reports.json`, `liquibase.xml`. https://github.com/METS-Programme/openmrs-module-ugandaemr-sync
5. `METS-Programme/openmrs-module-ugandaemr-reports`: `reports2019/106A1A2024Report.json`, `reports/MER_TX_NEW_2019.json`, `EvaluateReportDefinitionRestController.java` (`processFinalPayload`, `getYearAndQuarter`). https://github.com/METS-Programme/openmrs-module-ugandaemr-reports
6. DHIS2, "DHIS2 and FHIR". https://dhis2.org/integration/fhir/
7. `dhis2/dhis2-fhir-adapter` README (deprecated; fork at ITINordic). https://github.com/dhis2/dhis2-fhir-adapter
8. Uganda MoH, *Health Management Information System, Volume 1: Health Unit Procedure Manual*, August 2010, Technical Module 7, Table N1. https://www.gou.go.ug/sites/default/files/media-files/THE%20HEALTH%20MANAGEMENT%20INFORMATION%20SYSTEM_0.pdf
9. *Timeliness and completeness of weekly surveillance data reporting on epidemic prone diseases in Uganda, 2020–2021*, BMC Public Health 2023. https://pmc.ncbi.nlm.nih.gov/articles/PMC10072024/
10. Uganda MoH, national support supervision tool (submission of 105 and 108 by the 7th). https://library.health.go.ug/index.php/file-download/download/public/1518
11. *Timeliness and completeness of monthly disease surveillance data reporting, Uganda, 2020–2021*, Pan African Medical Journal 46:3. https://www.panafrican-med-journal.com/content/article/46/3/full
12. Uganda MoH / FANTA, *HMIS Nutrition Facilitator Manual*, September 2017 (revised HMIS 2014). https://www.fantaproject.org/sites/default/files/resources/Uganda-HMIS-Nutrition-FACILITATOR-Manual-Sep2017_0.pdf
13. DHIS2 developer docs, Web API, "Data" (data value sets, ADX, import parameters, soft delete, `/api/dataValues`). https://docs.dhis2.org/en/develop/using-the-api/dhis-core-version-master/data.html (source: `dhis2/dhis2-docs` `src/developer/web-api/data.md`)
14. DHIS2 developer docs, "Complete data set registrations" (`dhis2/dhis2-docs` `src/developer/web-api/data-validation.md`). https://github.com/dhis2/dhis2-docs/blob/master/src/developer/web-api/data-validation.md
15. DHIS2 user docs, data set configuration: expiry days and days for timely submission (`src/user/configure-metadata.md`). https://github.com/dhis2/dhis2-docs/blob/master/src/user/configure-metadata.md
16. DHIS2 user docs, "Data approval" (`src/user/data-approval.md`). https://github.com/dhis2/dhis2-docs/blob/master/src/user/data-approval.md
17. DHIS2 developer docs, "Data approval". https://docs.dhis2.org/en/develop/using-the-api/dhis-core-version-master/data-approval.html
18. As [15], "Expiry days".
19. DHIS2 developer docs, "Maintenance: lock exceptions" (`src/developer/web-api/maintenance.md`). https://github.com/dhis2/dhis2-docs/blob/master/src/developer/web-api/maintenance.md
20. DHIS2 developer docs, "Tracker". https://docs.dhis2.org/en/develop/using-the-api/dhis-core-version-master/tracker.html
21. DHIS2 developer docs, Web API overview, date and period formats (`src/developer/web-api/overview.md`). https://github.com/dhis2/dhis2-docs/blob/master/src/developer/web-api/overview.md
22. `METS-Programme/openmrs-module-aijar`: `AijarConstants.java` (`ugandaemr.dhis2.organizationuuid`); UgandaEMR manual, "HIV recency testing" (DHIS2 UID used as the username). https://github.com/METS-Programme/openmrs-module-aijar
23. OpenMRS wiki, "DHIS Connector Module". https://openmrs.atlassian.net/wiki/spaces/docs/pages/25475767/DHIS+Connector+Module
24. DHIS2 developer docs, metadata: data element "Zero is significant" (`src/developer/web-api/metadata.md`).
25. Uganda MoH, *Uganda National eHealth Policy*, November 2016. https://library.health.go.ug/file-download/download/public/1031
26. IHE QRPH, *Mobile Aggregate Data Exchange (mADX)* 3.0.0, trial use, FHIR R4 Measure and MeasureReport; transactions QRPH-58 and QRPH-59; uses mCSD for locations and SVCM for code lists. https://profiles.ihe.net/QRPH/mADX
27. DHIS2 user manual, Uganda v2.3 (Central Public Health Laboratories): data entry, completeness, HMIS support contact. https://cphl.go.ug/uploads/library/1782711642465-33b2278d0fcd.pdf

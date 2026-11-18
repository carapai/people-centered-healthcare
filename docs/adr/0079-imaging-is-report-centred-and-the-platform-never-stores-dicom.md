# Imaging is report-centred and the platform never stores DICOM

Decided in [How does the platform support imaging: orders, studies, reports and images?](https://github.com/carapai/people-centered-healthcare/issues/626), after the research [What do referrals, inpatient care, emergency care and imaging in Uganda require?](https://github.com/carapai/people-centered-healthcare/issues/623) ([clinical-operations.md](https://github.com/carapai/people-centered-healthcare/blob/research/clinical-operations/docs/research/clinical-operations.md), branch `research/clinical-operations`).

That research found:
- Much plain X-ray in Uganda is still on film.
- Only 34% of public HC IVs have a working ultrasound.
- About 75 radiologists serve the country.
- No inventory of PACS use exists.
- The standards require a signed request and a report with a clear conclusion. The X-ray register (HMIS 056) counts films by size.
- A FHIR `ImagingStudy` must reference a DICOM study, whose images live on a DICOM or DICOMweb server.

**The decision:**
- **Report-centred v1.** Imaging covers the request, the examination performed, the report and optional key images. An examination without DICOM (film, or most ultrasound) is a `Procedure` with no `ImagingStudy`.
- **Ordering.**
  - An imaging `ServiceRequest` is signed by a role allowed to request imaging.
  - Examinations come from an imaging catalogue in the Service Catalogue: modality and body region, with LOINC codes where they exist, priced through the Price Book.
  - Each request gets an accession number, printed on the slip.
- **Performing.**
  - Each modality has its own worklist.
  - The examination is a `Procedure`. Films used, by size, are issued from Stock. An examination can be marked not done, with a reason.
  - These records give the HMIS 056 register and the "X-rays taken" count.
- **Reporting.**
  - A `DiagnosticReport` (imaging category), with findings and a required conclusion, either preliminary or final, written by a role allowed to report.
  - Obstetric ultrasound has a structured template whose values become `Observation`s.
  - Critical findings use the critical-result banner. Reports go to the requester's inbox.
- **Images.**
  - The platform never stores or syncs DICOM.
  - A facility with digital modalities can run Orthanc beside its edge. The edge matches studies to requests by accession number over DICOMweb, and records an `ImagingStudy` with an endpoint. Images open in Orthanc's web viewer on the LAN.
  - Up to 5 key images per report (JPEG) can be attached through the blob store (ADR 0021).
- **Deferred.**
  - DICOM modality worklist.
  - Teleradiology and moving images to a remote reader.
  - A report from outside is entered by the facility, naming the remote reporter.

## Considered options

- **DICOM in the platform's blob store and sync:**
  - studies are hundreds of megabytes;
  - facility uplinks can't carry them;
  - most examinations are film anyway.
- **A PACS required for imaging:** would leave out every facility with film or a standalone ultrasound, which is most of them.
- **An embedded DICOM viewer in the client:** Orthanc's web viewer already exists, and an embedded one adds a large dependency for few facilities.
- **`ImagingStudy` for every examination:** R5 requires a DICOM study behind it.

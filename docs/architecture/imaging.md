# Imaging

Requests, examinations, reports and images. Decided in [How does the platform support imaging: orders, studies, reports and images?](https://github.com/carapai/people-centered-healthcare/issues/626) ([ADR 0079](../adr/0079-imaging-is-report-centred-and-the-platform-never-stores-dicom.md)). The facts are in the research note on branch `research/clinical-operations`.

## Resources

| Step | Resource | Owner |
|---|---|---|
| Request | `ServiceRequest` (imaging), with an accession number | orders |
| Examination performed | `Procedure` (modality, region, performer, time, films used) | imaging |
| DICOM study, when one exists | `ImagingStudy` with an endpoint to the facility's Orthanc | imaging |
| Report | `DiagnosticReport` (imaging category) with findings and a conclusion | imaging |
| Obstetric ultrasound values | `Observation`s from the report template | imaging |
| Key images | JPEG `Attachment`s on the report, in the blob store | imaging |

## Flow

1. **Request.**
   - A clinician whose role allows imaging requests picks an examination from the **imaging catalogue** in the Service Catalogue, with modality and body region, and LOINC codes where they exist.
   - They add clinical information, relevant previous examinations and priority (routine or urgent), then sign.
   - The request gets an **accession number**, printed on the slip.
   - Price comes from the Price Book. Urgent requests defer payment gates (ADR 0033).
2. **Perform.**
   - The radiographer or sonographer works from the worklist for their modality and records the `Procedure`.
   - Films used, by size, are issued from the imaging store's Stock.
   - An examination can be marked not done, with a reason.
3. **Report.**
   - A reporter (a radiologist, or a sonographer or medical officer where the facility allows) writes findings and a required **conclusion**, and marks the report preliminary or final.
   - The report reaches the requester's inbox. A critical finding raises the critical-result banner ([clinical-workspaces.md](clinical-workspaces.md)).

## Obstetric ultrasound

A structured template records:
- gestational age;
- number of foetuses;
- presentation;
- foetal heart activity;
- placental position;
- liquor volume;
- conclusion.

Each value is also an `Observation`, so antenatal care and Decision Support Rules can use it. Other examinations use free text with a conclusion.

## Images

- **The platform never stores or syncs DICOM.**
- **Orthanc (optional):**
  - A facility with digital modalities can run Orthanc beside its edge, as an optional part of the deployment.
  - The edge queries it over DICOMweb, matches each study to its request by accession number, and records an `ImagingStudy` with an endpoint.
  - Images open in Orthanc's web viewer, in a new tab on the LAN.
- **Key images:** up to 5 JPEGs per report, such as an ultrasound screenshot or a photo of a film, are attached through the blob store and downloaded on demand ([synchronization.md](synchronization.md)).

## Register and counts

- The HMIS 056 register view is built from the recorded examinations. It shows:
  - the serial number, restarting monthly;
  - the patient;
  - OPD or ward;
  - the examination and region;
  - films used by size.
- Ultrasound and CT use the same view.
- "X-rays taken", and examinations by modality, are Indicators.

## Not in v1

- A DICOM modality worklist, pushing orders to the machines.
- Teleradiology and moving images to a remote reader. A report from outside is entered by the facility, naming the remote reporter.
- Embedded DICOM viewing in the client.

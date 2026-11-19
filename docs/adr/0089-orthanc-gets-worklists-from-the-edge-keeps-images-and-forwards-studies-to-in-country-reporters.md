# Orthanc gets worklists from the edge, keeps images, and forwards studies to in-country remote reporters

Decided in [How does the platform feed modality worklists, keep images, and support remote reporting?](https://github.com/carapai/people-centered-healthcare/issues/774), after the research [What do DICOM worklists, image storage and teleradiology require in Uganda?](https://github.com/carapai/people-centered-healthcare/issues/773) ([imaging-integration.md](https://github.com/carapai/people-centered-healthcare/blob/research/imaging-integration/docs/research/imaging-integration.md), branch `research/imaging-integration`). It extends [ADR 0079](0079-imaging-is-report-centred-and-the-platform-never-stores-dicom.md), which deferred the modality worklist and teleradiology. The platform still never stores or syncs DICOM.

The research found:
- Orthanc serves DICOM Modality Worklist through its Worklists plugin (AGPLv3+, packaged separately), which has a REST API and can delete an entry when its study arrives. Orthanc has no MPPS server.
- Common DR, CT and ultrasound units query worklists and copy the accession number into the images.
- MoH guidelines (2023) set a 5-year minimum for health data, and the data protection law restricts storage outside Uganda. Orthanc's default recycling deletes the oldest patients.
- Public teleradiology is CT from regional referral hospitals to Mulago; private facilities often send abroad. UMDPC licenses remote reporting as an add-on and requires every reporter to be registered.
- A 250 MB CT takes about 17 minutes at 2 Mbps; the Transfers Accelerator compresses and retries.

**The decision:**
- **Worklist.** When an imaging request reaches its modality's worklist on the platform (after any payment gate), the edge creates an Orthanc worklist entry over REST: accession number, Patient Number, name, date of birth, sex, modality, scheduled date and time, procedure, and the AE title configured for the modality. The entry is removed when the study arrives or the request is cancelled.
- **The official Worklists plugin**, unmodified, is part of the optional Orthanc deployment; the platform talks to it only over REST.
- **Completion.** With no MPPS, the study's arrival, matched by accession number, prefills the examination `Procedure` (time, modality, series and image counts); the radiographer confirms it and adds films used or notes.
- **Keeping images.** Orthanc recycling is off; PostgreSQL holds the index at hospitals with real volume; images are compressed losslessly; images are kept at least 5 years and deleted only by an explicit retention job, off by default. Storage raises a warning at a threshold. Backups go nightly to a second disk and regularly to an off-site copy in Uganda.
- **Remote reporting.** A request can be marked **report remotely** to a named **Remote Reporting Service**, configured at central with its Orthanc or DICOMweb address; only destinations in Uganda can be configured. The facility's Orthanc sends the study through the Transfers Accelerator, and each sending records who, when and where.
  - A service on the platform receives the request in its Sync Scope like a referral; its radiologist reads the copy at their site and writes the `DiagnosticReport`, which syncs back. The `ImagingStudy` gains an endpoint at the reporting site.
  - A service not on the platform has its report entered by the facility, naming the reporter (ADR 0079).
- **Reporters.** A remote reporter's Practitioner record carries a UMDPC registration number; only reporters with one can sign remote reports. It is not checked online.
- **Deferred.** MPPS, an embedded DICOM viewer, cloud PACS, reporting from outside Uganda, IHE XDS-I sharing and DICOMweb UPS-RS.

## Considered options

- **A Python worklist plugin of our own:** more control, but our code inside Orthanc; the official plugin already does create, list, delete and auto-delete on arrival.
- **Worklist entries at order-sign:** unpaid or not-yet-arrived patients would crowd the modality's list; the platform's own worklist already says who is ready.
- **Orthanc's default storage cap:** silently deletes the oldest patients, breaking the 5-year minimum.
- **Allowing reporters abroad:** the data protection law restricts storage outside Uganda and UMDPC requires local registration and employment; this can be revisited with legal review.

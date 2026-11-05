# health-shared-record

Layer: **domain** (shared-record). The shared record's patient summary (clinical-workspaces.md; #104): what a clinician sees first, and in what order. The records come in already filtered by what the clinician may see, because sensitive labels are the policy decision point's.

- **`summarize(Record) -> Summary`** keeps what is current and orders it:
  - **Allergies:** those not resolved, refuted or entered in error, high criticality first.
  - **Conditions:** active, recurring or relapsing, and not refuted or entered in error, latest onset first.
  - **Medications:** active or on hold, latest first.
  - **Vitals:** the latest reading of each code, leaving out cancelled and entered-in-error readings.
  - **Visits:** the latest `VISITS` (10) Encounters, leaving out those entered in error.
  - **Programs:** active and waitlisted EpisodesOfCare.
- **An unseen section:** a section the clinician may not search at all is `None` (`null` in JSON), not empty.
- **Dates** are FHIR `date` or `dateTime` text in UTC, so their text orders them.

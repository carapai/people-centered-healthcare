# Operations are booked onto Theatre Lists, consented, and recorded without moving the ward placement

Decided in [How does the platform support the operating theatre: bookings, the safety checklist, anaesthesia and the theatre register?](https://github.com/carapai/people-centered-healthcare/issues/685), after the research [What do theatre, labour and delivery, blood transfusion, ICU and the mortuary require in Uganda?](https://github.com/carapai/people-centered-healthcare/issues/684) ([specialist-inpatient.md](https://github.com/carapai/people-centered-healthcare/blob/research/specialist-inpatient/docs/research/specialist-inpatient.md), branch `research/specialist-inpatient`). It extends [ADR 0078](0078-inpatient-stays-are-received-into-wards-and-counted-from-location-history.md), which deferred the theatre.

The research found:
- The theatre register is HMIS 057: one per theatre, major and minor kept separately, with a serial number restarting each January.
- The 2016 service standards require consent, a pre-anaesthetic assessment, an anaesthetic record, an operation report naming the team, and recovery times with handover signatures.
- No national anaesthesia, booking or checklist form was found. Studies put WHO Surgical Safety Checklist compliance at 12–90% by hospital.
- The live HMIS 108 counts procedures by specialty, with caesarean sections as SP01.

**The decision:**
- **Request and schedule.**
  - A surgeon places an **Operation Request** (a surgical `ServiceRequest`) from a consultation, a ward round or the emergency unit.
  - Theatre staff schedule it onto a **Theatre List** for a date and theatre, in order. Emergencies go to the top.
  - Theatres are Locations of type operating theatre, marked major or minor. They are not wards.
  - Inpatients and day cases both use it. A minor operation on an outpatient stays within the outpatient Encounter. Caesarean sections follow the same path; the birth itself belongs to labour and delivery.
- **The ward placement does not move.** An inpatient keeps their ward bed while in theatre. Theatre in and out times are on the `Procedure`, not the Encounter's location history, so the midnight census and bed-days are unaffected.
- **Procedure Consent.** The person who consented (the patient or a named guardian), the witness, the date and a photo of the signed form. The operation cannot start without it, except by an emergency override that records the reason and who decided.
- **Surgical Safety Checklist.** The three phases (sign in, time out, sign out) ship as a configurable form in a package, defaulting to the WHO checklist. Each phase records who completed it and when. It is **recorded, not enforced**: a missing phase never blocks the operation, but it shows on the operation and in a compliance indicator.
- **Anaesthesia.**
  - A pre-anaesthetic assessment: ASA class, airway, allergies and fasting.
  - An **Anaesthesia Record**: the anaesthesia type (general, spinal, regional, local or sedation); drugs and gases as `MedicationAdministration`s issued from the theatre store; vital signs at intervals as `Observation`s, entered by hand.
  - Recovery: arrival and discharge times and handover signatures; the anaesthetist authorises discharge from recovery.
- **Operation Note.** A signed `Composition` naming the surgeon, assistants, anaesthetist and scrub nurse, with the operation, findings, post-operative diagnosis and post-operative orders, which are placed from it.
- **Coding.** Operations come from the facility's operation catalogue in the Service Catalogue, each mapped to its HMIS 108 procedure row. No international procedure code system is adopted yet.
- **Stock and charges.**
  - Consumables are issued from the theatre store at the time of use.
  - Operation, theatre and anaesthesia fees come from the Price Book.
  - Emergency operations always defer payment gates (ADR 0033). A deposit for elective operations is a facility setting.
- **Register and counts.**
  - An HMIS 057 register view per theatre, built from the recorded operations.
  - HMIS 108 procedure counts by specialty are Indicators.
  - Sterilisations and implant insertions and removals feed HMIS 105's family planning rows.
- **Deferred.** Sterilisation unit records (autoclave logs, instrument sets), monitor and device integration, theatre utilisation analytics and booking optimisation.

## Considered options

- **A ward transfer to theatre and back:** puts theatre time into the location history, so a patient in theatre at midnight would leave their ward's census.
- **Enforcing the safety checklist as a gate:** with compliance as low as 12%, a hard gate would be clicked through or falsified, and the record would lose its value.
- **SNOMED CT or ICHI procedure codes now:** SNOMED CT licensing for Uganda is undecided, ICD-11 has no procedure axis, and HMIS reports by local rows anyway.
- **A separate Encounter for each operation:** splits one inpatient stay and complicates length of stay; the `Procedure` already carries the theatre, times and team.

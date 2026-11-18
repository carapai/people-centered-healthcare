# Operating theatre

Requests, Theatre Lists, consent, the safety checklist, anaesthesia, the Operation Note and the theatre register. Decided in [How does the platform support the operating theatre: bookings, the safety checklist, anaesthesia and the theatre register?](https://github.com/carapai/people-centered-healthcare/issues/685) ([ADR 0082](../adr/0082-operations-are-booked-onto-theatre-lists-and-recorded-without-moving-the-ward-placement.md)). The facts are in the research note on branch `research/specialist-inpatient`. Wards, admission and the census are in [inpatient-and-emergency.md](inpatient-and-emergency.md).

## Resources

| Step | Resource | Owner |
|---|---|---|
| Operation Request | `ServiceRequest` (surgical category) | orders |
| Theatre List entry | `Appointment` for the theatre Location, ordered within the list | theatre |
| Procedure Consent | `Consent` (treatment scope) with the signed form as an attachment | theatre |
| Surgical Safety Checklist, pre-anaesthetic assessment | `QuestionnaireResponse`s from package forms | theatre |
| The operation | `Procedure` (theatre, in and out times, performers, catalogue code) | theatre |
| Anaesthesia drugs and vital signs | `MedicationAdministration`s and `Observation`s | theatre |
| Operation Note | `Composition`, signed | theatre |

## Flow

1. **Request.** A surgeon picks an operation from the facility's operation catalogue and signs an **Operation Request** from a consultation, a ward round or the emergency unit, with priority (elective or emergency).
2. **Schedule.** Theatre staff place it on a **Theatre List** for a theatre and date. Emergencies go to the top. Lists print.
3. **Prepare.** The pre-anaesthetic assessment (ASA class, airway, allergies, fasting) and the **Procedure Consent** are recorded. Without consent the operation cannot start, unless an emergency override records the reason and who decided.
4. **Operate.**
   - The **Surgical Safety Checklist** phases (sign in, time out, sign out) are recorded with who completed each and when. A missing phase is shown, never blocking.
   - The **Anaesthesia Record** holds the anaesthesia type, drugs and gases given (from the theatre store) and vital signs at intervals.
   - Consumables are issued from the theatre store as they are used.
5. **Recover.** Recovery arrival and discharge times and handover signatures; the anaesthetist authorises discharge from recovery.
6. **Note.** The surgeon signs the **Operation Note**: team, operation done, findings, post-operative diagnosis and post-operative orders.

An inpatient's ward placement does not change while they are in theatre, so the census and bed-days in [inpatient-and-emergency.md](inpatient-and-emergency.md) are unaffected. A minor operation on an outpatient stays within the outpatient Encounter.

## Charges

- Operation, theatre and anaesthesia fees come from the Price Book.
- Emergency operations always defer payment gates (ADR 0033). A deposit for elective operations is a facility setting.

## Register and counts

- **HMIS 057 register view**, per theatre, major and minor separate: date, serial number (restarting each January), patient, patient number, age, sex, surgeon, anaesthetist, anaesthesia used, diagnosis, operation and remarks.
- **HMIS 108** procedure counts by specialty are Indicators, from each catalogue entry's mapping to its row.
- Sterilisations and implant insertions and removals feed HMIS 105's family planning rows.
- A checklist compliance indicator: operations with all three phases recorded.
- The current HMIS 057 layout and the HMIS 108 procedure rows' disaggregation are not yet verified.

## Not in v1

- Sterilisation unit records: autoclave logs and instrument sets.
- Monitor and device integration.
- Theatre utilisation analytics and booking optimisation.
- International procedure codes (SNOMED CT or ICHI).

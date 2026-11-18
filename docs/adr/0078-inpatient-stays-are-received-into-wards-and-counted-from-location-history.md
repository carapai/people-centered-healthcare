# Inpatient stays are received into wards and counted from location history

Decided in [How does the platform support inpatient and emergency care: admission, wards, beds, rounds and discharge?](https://github.com/carapai/people-centered-healthcare/issues/625), after the research [What do referrals, inpatient care, emergency care and imaging in Uganda require?](https://github.com/carapai/people-centered-healthcare/issues/623) ([clinical-operations.md](https://github.com/carapai/people-centered-healthcare/blob/research/clinical-operations/docs/research/clinical-operations.md), branch `research/clinical-operations`).

That research found that Uganda's inpatient care runs on paper:
- the treatment sheet (HMIS 051);
- the follow-up chart (HMIS 034), whose tick grid is the medication administration record;
- the discharge form (HMIS 052);
- the inpatient register (HMIS 054), whose IP numbers restart each July.

HMIS 108 reports by 20 ward types and counts patient days with a midnight census. Emergency care uses red, yellow and green triage (UTAT: red within 10 minutes, yellow within 1 hour). Emergency counts go into HMIS 105 rows ES01–ES09.

**The decision:**
- **Admission is two steps.**
  - A clinician's **Admission Order** (a `ServiceRequest` to a ward) puts the patient on the ward's incoming list.
  - The ward receives the patient. That creates the inpatient `Encounter` with its admit source, "referred from" and **Inpatient Number**: a per-facility sequence that restarts each July.
  - The originating OPD or emergency Encounter is closed and linked.
- **Wards and beds are Locations.**
  - A ward carries one of the HMIS 108 **Ward Types** and a designated bed count. Beds are its child Locations.
  - Placement and ward transfers are periods in the Encounter's location history, so a transfer keeps the same Encounter.
  - A patient may be placed without a bed. They count in the census, and the ward shows as over capacity.
- **Charting.**
  - The **administration chart** shows each medication order as scheduled doses. Each dose is recorded as given, not given with a reason, or held, as a `MedicationAdministration`. This posts stock for ward stores that track at administration (ADR 0035).
  - **Nursing Orders** (recurring non-medication instructions) appear on the same chart.
  - Vital signs and fluid balance are `Observation`s.
  - There is no early-warning score in v1.
- **Ward rounds.** A ward round list in bed order shows new results and missed doses. A **Ward Round Note** (`Composition`) holds the assessment and plan, and orders are placed from it.
- **Discharge.**
  - Statuses are discharged, died, referred out (which creates a Referral, ADR 0077), or self-discharged/absconded.
  - A signed, printed **Discharge Summary** has the HMIS 052 fields. It fills the Referral Feedback for a referred-in patient.
  - Payment never blocks clinical discharge. An unpaid balance stays on the Account.
- **Deaths.**
  - A death is recorded at discharge or as an emergency outcome, with a cause-of-death certificate in the WHO MCCD layout (ICD-11).
  - The Patient is marked deceased, which stops reminders, Messages and Due List entries.
- **Emergency.**
  - The triage scale ships in a package and is configurable. The default is red, yellow and green with UTAT timings, plus ETAT signs for children.
  - The triage category is an `Observation` that orders the queue and defers payment gates (ADR 0033). Time-to-be-seen is shown against the target.
  - An emergency `Encounter` carries:
    - a case category: medical, obstetrics and gynaecology, surgical, or road traffic injury;
    - the injury mechanism for road traffic injuries;
    - for external haemorrhage, whether it was controlled;
    - an outcome: admitted, discharged, referred, or died.
  - Observation in the emergency unit stays within that Encounter.
- **Unknown patients** are registered with sex, estimated age and a temporary name, treated at once, and linked or merged through the golden record once identified.
- **Counting.**
  - HMIS 108 census figures are computed per Ward Type from the location history by the midnight-census rule.
  - Bed-day charges use the same rule.
  - Discharge diagnoses map to item 6.
  - Emergency Encounters feed HMIS 105 ES01–ES09.
- **Deferred.** Operating theatre, blood transfusion, labour ward, ICU and the mortuary.

## Considered options

- **The Admission Order alone as the admission:** places patients into wards that may be full, and splits the moment of admission from the bed.
- **A new Encounter per ward transfer:** inflates admissions in HMIS 108 and breaks length of stay.
- **Patient days from admission and discharge dates only:** the method third-party compilers use, but it differs from the manual's daily census for patients still on the ward.
- **Discharge blocked until the bill is cleared:** detaining patients for unpaid bills harms them and fills beds.
- **One fixed triage scale:** Uganda uses UTAT, IITT and ETAT side by side, and MoH policy is still settling.

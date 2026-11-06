# health-medications

Layer: **domain** (medications). Prescribing (inventory-and-procurement.md § Dispensing; #108). The MedicationRequest is the medications module's.

- **`prescribe`** writes an `active` prescription, with `medications.prescribed.v1`.
  - It has **no stock effect**: a MedicationRequest never reserves or moves stock (ADR 0035).
  - It needs a patient, a medicine, a dosage and its prescriber. A course, if stated, lasts at least a day.
- **`Prescription::transition`** follows R5:
  - active and on hold move between each other;
  - an active prescription completes, stops or is cancelled, and a held one stops or is cancelled;
  - an open one may be entered in error.
- **`based_on`:** the request a prescription fulfils, such as a patient's refill proposal (#114), written as `basedOn`.
- **`SafetyChecks`** is the clinical safety checks hook. Clinical decision support is in the map's fog. `BasicChecks` warns of:
  - the same medicine already open (`duplicate-therapy`);
  - a medicine whose name contains a recorded allergy's substance (`allergy`).

  A warning is not a refusal: the prescriber acknowledges it.
- **`worklist(prescriptions, facility)`:** the pharmacy worklist, the facility's open prescriptions oldest first.

# Blood transfusion

Blood Units, Transfusion Requests, cross-match, issue, bedside administration and reactions. Decided in [How are blood transfusions requested, issued, given and watched for reactions?](https://github.com/carapai/people-centered-healthcare/issues/687) ([ADR 0084](../adr/0084-blood-units-are-single-unit-stock-issued-only-when-cross-matched.md)). The facts are in the research note on branch `research/specialist-inpatient`. The administration chart is in [inpatient-and-emergency.md](inpatient-and-emergency.md).

## Resources

| Step | Resource | Owner |
|---|---|---|
| Blood Unit | `BiologicallyDerivedProduct`, and a Stock Unit of quantity one in the blood bank store | laboratory |
| Transfusion Request | `ServiceRequest` (component, units, urgency, indication, pre-transfusion haemoglobin) | orders |
| Patient blood group | `Observation` (ABO and Rh) | laboratory |
| Compatibility Test | `Observation` per unit and patient (immediate spin, indirect antiglobulin test) | laboratory |
| Blood Issue | `BiologicallyDerivedProductDispense`, with a stock issue | laboratory |
| Transfusion given | `MedicationAdministration`-style entry on the administration chart, with `Observation`s | inpatient |
| Transfusion Reaction | `AdverseEvent` | inpatient |

## Flow

1. **Receive.** The blood bank store receives units from UBTS, each with its unit number, component, group, expiry and source bank, and who received and inspected it.
2. **Request.** A clinician signs a **Transfusion Request**: component, number of units, urgency (routine, urgent, emergency without cross-match), indication and pre-transfusion haemoglobin. A sample goes to the laboratory for grouping and cross-match. Emergency requests defer payment gates.
3. **Group and cross-match.** The laboratory records the patient's group and a **Compatibility Test** for each candidate unit. An incompatible unit cannot be issued. In an emergency, uncross-matched group O units can be issued with a recorded reason and approver.
4. **Issue.** The unit is reserved and issued as a **Blood Issue**: who issued it, who carried it, and when. An unused unit returned within the facility's limit (default 30 minutes) goes back to stock; later, it is discarded.
5. **Give.** At the bedside two staff record the identity check; a unit issued to another patient cannot be recorded as given. The transfusion goes on the administration chart with start, end and volume, and vital signs before, at 15 minutes and at the end (a facility setting).
6. **Watch for reactions.** A suspected **Transfusion Reaction** records type, severity, whether the transfusion was stopped and the actions taken. It notifies the laboratory and the hospital transfusion committee, and a reaction report prints for UBTS.

## Register and counts

- **HMIS 091 register view**: unit number, group, component, receipt and inspection, patient, patient's group, hospital number, haemoglobin, ward, requesting clinician, compatibility results, expiry, issue, carrier, return, reason; plus administration and reaction columns.
- **HMIS 108 item 5:** UT01 units requested, UT02 received, UT03 transfused, UT04 adverse reactions, as Indicators.
- Cross-match and transfusion fees come from the Price Book.
- The HMIS 091 number and the UT rows' disaggregation are not yet verified.

## Not in v1

- Electronic ordering from UBTS and traceability back to the donor.
- Donor and collection management, and component processing.
- Blood fridge temperature monitoring.
- ISBT 128 barcode scanning.
- A two-sample grouping rule.

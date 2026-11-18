# Blood units are single-unit Stock, issued only when cross-matched, and given on the administration chart

Decided in [How are blood transfusions requested, issued, given and watched for reactions?](https://github.com/carapai/people-centered-healthcare/issues/687), after the research [What do theatre, labour and delivery, blood transfusion, ICU and the mortuary require in Uganda?](https://github.com/carapai/people-centered-healthcare/issues/684) ([specialist-inpatient.md](https://github.com/carapai/people-centered-healthcare/blob/research/specialist-inpatient/docs/research/specialist-inpatient.md), branch `research/specialist-inpatient`). It extends [ADR 0078](0078-inpatient-stays-are-received-into-wards-and-counted-from-location-history.md) (the administration chart) and the Stock model ([ADR 0035](0035-stock-moves-on-fulfilment-with-reservations-and-fefo.md)).

The research found:
- The laboratory keeps the blood transfusion record (HMIS 091): unit, group, component, cross-match (immediate spin and indirect antiglobulin test), issue and return. It has no columns for giving the blood or for reactions.
- HMIS 108 counts units requested, received and transfused, and adverse reactions (UT01–UT04).
- Blood comes from 7 regional UBTS blood banks. No national haemovigilance route or reaction form was found.

**The decision:**
- **Blood Units are Stock.** Each Blood Unit is a Stock Unit of quantity one, with the UBTS unit number as its batch, plus component, ABO and Rh group, expiry and source blood bank; it is also a `BiologicallyDerivedProduct`. The laboratory's blood bank store receives units, recording who received and inspected them. Expired or spoilt units are discarded with a reason.
- **Transfusion Request.** A clinician signs a `ServiceRequest` with component, number of units, urgency (routine, urgent, or emergency without cross-match), indication and pre-transfusion haemoglobin. It creates the grouping and cross-match order for the laboratory. Emergency requests defer payment gates (ADR 0033).
- **Compatibility Test.** The laboratory records the patient's ABO and Rh group as an `Observation`, and a Compatibility Test (immediate spin, indirect antiglobulin test) for each candidate unit. **An incompatible unit cannot be issued.** The only exception is the emergency path: uncross-matched group O units, issued with a recorded reason and approver.
- **Blood Issue.** A compatible unit is reserved for the patient and issued to the ward as a `BiologicallyDerivedProductDispense`, recording who issued it, who carried it and when, and posting a stock issue. A unit returned unused within the facility's limit (default 30 minutes) goes back to stock; after it, the unit is discarded.
- **At the bedside.** A unit issued to one patient cannot be recorded as given to another. Two staff record the bedside identity check. The transfusion is given on the administration chart (start, end, volume), with vital signs before, at 15 minutes and at the end as `Observation`s; the schedule is a facility setting.
- **Transfusion Reaction.** A suspected reaction is an `AdverseEvent` with type, severity, whether the transfusion was stopped, and actions taken. It notifies the laboratory and the hospital transfusion committee, counts towards UT04, and prints as a reaction report for UBTS.
- **Register and counts.** An HMIS 091 register view, with administration and reaction columns added. HMIS 108 UT01–UT04 are Indicators from requests, receipts, completed administrations and reactions. Cross-match and transfusion fees come from the Price Book.
- **Deferred.** Electronic ordering from and traceability to UBTS, donor and collection management, component processing, blood fridge temperature monitoring, ISBT 128 barcode scanning, and a two-sample grouping rule.

## Considered options

- **Blood as ordinary batch Stock:** a batch holds many items, but each blood unit has its own number, group and compatibility, and must be traced to one patient.
- **Warning, not blocking, on an incompatible unit:** an ABO-incompatible transfusion can kill; the emergency uncross-matched path already covers genuine urgency.
- **Keeping administration in the laboratory record:** the ward gives the blood, and the administration chart already records doses, times and observations.
- **No reaction record until a national route exists:** UT04 must still be counted, and the facility's transfusion committee needs the cases.

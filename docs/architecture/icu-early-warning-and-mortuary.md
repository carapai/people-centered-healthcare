# ICU, early warning and the mortuary

ICU and HDU care, the Early Warning Score, the mortuary and the weekly Death Return. Decided in [How does the platform support ICU and HDU, early-warning scores and the mortuary?](https://github.com/carapai/people-centered-healthcare/issues/727) ([ADR 0085](../adr/0085-icu-and-the-mortuary-build-on-wards-and-deaths-and-early-warning-is-advisory.md)). The facts are in the research note on branch `research/specialist-inpatient`. Wards, the chart, deaths and the cause-of-death certificate are in [inpatient-and-emergency.md](inpatient-and-emergency.md).

## Resources

| Step | Resource | Owner |
|---|---|---|
| ICU or HDU ward | `Location` (ward) with a level of care | inpatient |
| ICU Chart entries | `Observation`s from the ICU Chart package form | inpatient |
| Early Warning Score | `Observation` (calculated) and an escalation `Task` | inpatient |
| Mortuary | `Location` of type mortuary | mortuary |
| Mortuary Admission and Body Release | `Encounter` at the mortuary for the deceased Patient, with `QuestionnaireResponse`s | mortuary |
| Post-mortem | `Procedure` with a signed `DiagnosticReport` | mortuary |
| NIRA death particulars | `QuestionnaireResponse`, listed on the weekly Death Return | inpatient |

## ICU and HDU

- A ward's **level of care** is ICU or HDU. Its Ward Type stays one of HMIS 108's; an HDU defaults to the ICU Ward Type, so beds, admissions, deaths and patient days are counted by the census as usual.
- The **ICU Chart** records hourly vital signs, GCS, ventilation mode and settings, oxygen and hourly fluid balance. Drugs and infusions stay on the administration chart.
- **ICU register view:** admission source, diagnosis, ventilated or not, days and outcome.
- Local Indicators: ventilator days and readmission to ICU within 48 hours.

## Early Warning Score

- Shipped as a package defaulting to MEWS, switched on per ward; off by default.
- Calculated from vital signs already on the chart.
- At or above the package threshold: shown on the chart, and an escalation Task for the nurse in charge and the doctor on call.
- Advisory: never blocks anything.

## Mortuary

1. **Arrive.** A **Mortuary Admission** on the deceased's Patient: arrival time, source (ward, emergency unit, brought in dead), who brought and who received the body, identifying features, a photo and the storage place. A body brought in dead is registered as a Patient first, through the unknown-patient flow if unidentified.
2. **Services.** Embalming and storage are charged from the Price Book.
3. **Police cases.** Any death can be marked a police case. A post-mortem is a `Procedure` with a signed report that informs the cause-of-death certificate.
4. **Release.** A **Body Release** records the person collecting (name, NIN or other ID, relationship) and the documents checked, and prints a **gate pass**. A police case cannot be released until police clearance is recorded. Unpaid fees stay on the account and never block a release.

The **mortuary register view** lists each body from arrival to release.

## Death Return

- When any facility death is recorded, the NIRA particulars are collected: the informant's name, capacity, NIN and signature.
- The weekly **Death Return** lists every facility death with the NIRA copy of its cause-of-death certificate, printed for the registration officer. The certificate is due within 7 days.

## Not in v1

- ICU device integration and severity scores (APACHE, SOFA).
- PEWS and MEOWS (ready to add as packages).
- Mortuary fridge temperature monitoring.
- Quarterly mortality review tooling.

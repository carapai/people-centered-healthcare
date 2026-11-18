# Labour and delivery

Admission in labour, the Labour Chart, the Delivery, newborns, PMTCT, maternal and perinatal deaths, and notifying NIRA. Decided in [How does the platform support labour and delivery: the labour record, births, newborns and maternal deaths?](https://github.com/carapai/people-centered-healthcare/issues/686) ([ADR 0083](../adr/0083-labour-is-an-inpatient-maternity-stay-and-every-live-birth-is-a-patient-from-birth.md)). The facts are in the research note on branch `research/specialist-inpatient`. Admission, wards, discharge and deaths are in [inpatient-and-emergency.md](inpatient-and-emergency.md); caesarean sections in [operating-theatre.md](operating-theatre.md).

## Resources

| Step | Resource | Owner |
|---|---|---|
| Maternity admission | Inpatient `Encounter` in a maternity ward, with the Maternity Number and a link to the antenatal enrolment | inpatient |
| Labour Chart | `Observation`s from the active labour monitoring package form | maternity |
| Delivery | `Procedure` (delivery) with an outcome per baby; stillbirths as outcomes | maternity |
| Newborn | `Patient`, with the mother as Caregiver, and a newborn `Encounter` | maternity |
| Newborn measurements | `Observation`s: birth weight, Apgar at 1 and 5 minutes, breastfeeding within 1 hour | maternity |
| Maternal or perinatal death notification and review | `Task`s with `QuestionnaireResponse`s (HMIS 120a/120, 121a/121) | maternity |
| Birth particulars for NIRA | `QuestionnaireResponse` (Form 3), listed on the weekly Birth Return | maternity |

## Flow

1. **Admit.** By Admission Order to a maternity ward, received as in [inpatient-and-emergency.md](inpatient-and-emergency.md). The admission links to the antenatal enrolment, or is marked not booked or referred, and gets the **Maternity Number** (restarting each July). Women who delivered before arrival are admitted the same way.
2. **Monitor labour** on the **Labour Chart**: the partograph in v1, or the WHO Labour Care Guide once that package is activated. Observations are plotted with the alert and action lines.
3. **Deliver.** The **Delivery** records mode, date and time, third-stage management, complications and the birth attendant, with an outcome per baby: live birth, fresh stillbirth or macerated stillbirth. A caesarean goes through the theatre and links back.
4. **Register the newborn.** Each live-born baby becomes a Patient at once ("Baby of …"), with the mother as Caregiver: sex, birth weight, Apgar scores, breastfeeding within 1 hour. Birth doses (BCG, polio) are recorded in the immunization record. A healthy baby has a newborn Encounter linked to the mother's and is not counted in the census; a sick baby is admitted to the Neonatal Unit.
5. **Discharge** mother and baby, recording each one's condition, collecting the Form 3 particulars, and booking postnatal Appointments at 6 days and 6 weeks.

## PMTCT

The maternity form shows HIV status and the ARV regimen from the HIV program to roles allowed to see them. ARVs to mother and baby are medication records, and the register's PMTCT columns are derived from them.

## Maternal and perinatal deaths

- The death of a woman aged 15–49 prompts a screen for maternal death.
- A **Maternal Death Notification** (HMIS 120a) is due within 24 hours: a Task and Notification for the ward in-charge and the surveillance focal person, printable. A review Task (HMIS 120) is due within 7 days.
- Stillbirths and deaths within 7 days of birth get a perinatal notification (HMIS 121a) and review (HMIS 121) on the same clock.
- Weekly counts of maternal and perinatal deaths, including zero, are Indicators.
- The cause-of-death certificate is as in [inpatient-and-emergency.md](inpatient-and-emergency.md).

## NIRA

- **Births:** the weekly **Birth Return** lists every birth in the facility with its Form 3 particulars and signatures, printed for the NIRA registration officer.
- **Deaths:** every facility death, maternal or not, goes on the weekly Death Return in [icu-early-warning-and-mortuary.md](icu-early-warning-and-mortuary.md) (ADR 0085).

## Register and counts

- **HMIS 072 register view** from maternity admissions and deliveries.
- HMIS 105 maternity rows, and HMIS 108 maternal (MC) and neonatal (ND) conditions, as Indicators.
- The current HMIS 072 layout, its serial cycle and the live HMIS 105 maternity rows are not yet verified.

## Not in v1

- Foetal monitor integration.
- MPDSR committee action tracking.
- NIRA's electronic channel (MVRS).
- mTRAC and eHMIS event submission.
- Kangaroo Mother Care unit specifics.
- Decision support on the Labour Chart beyond the alert and action lines.

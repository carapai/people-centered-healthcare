# Labour is an inpatient maternity stay with a package-defined Labour Chart, and every live birth is a Patient from birth

Decided in [How does the platform support labour and delivery: the labour record, births, newborns and maternal deaths?](https://github.com/carapai/people-centered-healthcare/issues/686), after the research [What do theatre, labour and delivery, blood transfusion, ICU and the mortuary require in Uganda?](https://github.com/carapai/people-centered-healthcare/issues/684) ([specialist-inpatient.md](https://github.com/carapai/people-centered-healthcare/blob/research/specialist-inpatient/docs/research/specialist-inpatient.md), branch `research/specialist-inpatient`). It extends [ADR 0078](0078-inpatient-stays-are-received-into-wards-and-counted-from-location-history.md) (admission, wards, deaths) and [ADR 0082](0082-operations-are-booked-onto-theatre-lists-and-recorded-without-moving-the-ward-placement.md) (caesarean sections in theatre).

The research found:
- The maternity register, HMIS 072, records every admission to maternity, delivered or not, with its own serial, and a partogram is completed for every delivery.
- The 2016 standards still require the partograph. MoH is reportedly adopting the WHO Labour Care Guide, but no order was found.
- Maternal and perinatal deaths are notified within 24 hours (HMIS 120a, 121a) and reviewed within 7 days (HMIS 120, 121) under the 2017 MPDSR guidelines.
- Facilities must record every birth and death and file weekly returns with the NIRA registration officer; the cause-of-death certificate reaches NIRA within 7 days.

**The decision:**
- **Admission.** A woman in labour is admitted by Admission Order and received into a ward of Ward Type maternity (ADR 0078). The admission links to her antenatal enrolment, or is marked not booked or referred. Besides the Inpatient Number she gets a **Maternity Number**, the maternity register's serial, restarting each July (to be confirmed against the current HMIS 072). A woman who delivered before arrival is admitted the same way.
- **Labour Chart.** The labour monitoring form ships in a package. v1 activates the **partograph**; a WHO Labour Care Guide package is ready to activate when MoH orders the switch. Entries (cervical dilatation, foetal heart rate, contractions, maternal vital signs, drugs) are `Observation`s plotted with the alert and action lines, and the chart carries the Maternity Number.
- **Delivery.** One Delivery per birth event, with an outcome for each baby: mode (spontaneous, assisted, breech or caesarean), date and time, third-stage management, complications and the birth attendant. A caesarean is performed through the theatre (ADR 0082) and linked. Stillbirths, fresh or macerated, are outcomes on the mother's record, not Patients.
- **Newborn.** Every live-born baby is a Patient from birth, named "Baby of …" until named, with the mother linked as Caregiver and the Delivery as the source: sex, birth weight, Apgar at 1 and 5 minutes, breastfeeding within 1 hour. Birth doses go to the immunization record (ADR 0076). A healthy baby rooming in has a newborn Encounter linked to the mother's that is not counted in the census; a sick baby is admitted to the Neonatal Unit by Admission Order and counted.
- **PMTCT.** The maternity form shows HIV status and the ARV regimen from the HIV program to roles allowed to see them, under the confidential-category rules. ARVs to mother or baby are medication records, and the register's PMTCT columns are derived, never typed twice.
- **MPDSR.**
  - The death of a woman aged 15–49 prompts a maternal death screen.
  - A maternal death creates a **Maternal Death Notification** (HMIS 120a) due within 24 hours, as a Task and Notification for the ward in-charge and the surveillance focal person, printable, and a review Task (HMIS 120) due within 7 days.
  - Perinatal deaths (stillbirths and deaths within 7 days of birth) do the same with HMIS 121a and 121.
  - Weekly maternal and perinatal death counts, including zero, are Indicators.
- **NIRA.** At discharge the Form 3 particulars are collected for each birth, and a weekly **Birth Return** prints for the registration officer. The NIRA copy of each cause-of-death certificate goes on a weekly death return.
- **Discharge.** The inpatient discharge statuses apply ("ran away" is self-discharged). The condition of mother and baby are recorded, and postnatal follow-up is booked as Appointments at 6 days and 6 weeks; postnatal visits belong to the antenatal and postnatal program.
- **Counts.** An HMIS 072 register view, the HMIS 105 maternity rows, and HMIS 108's maternal (MC) and neonatal (ND) conditions are built from these records.
- **Deferred.** Foetal monitor integration, MPDSR action tracking, NIRA's electronic channel (MVRS), mTRAC and eHMIS event submission, Kangaroo Mother Care unit specifics, and decision support on the Labour Chart beyond the alert and action lines.

## Considered options

- **The Labour Care Guide as the default now:** current standards and supervision still check the partograph, and no MoH order to switch was found; a package switch costs nothing later.
- **The newborn as part of the mother's record until discharge:** birth doses, neonatal admission, neonatal deaths and the NIRA birth return all need the baby as a person from birth.
- **Stillbirths as Patients:** they are not persons under registration law and are counted from the mother's delivery outcome.
- **The Inpatient Number alone:** the maternity register numbers maternity admissions separately, and the partograph carries that number.
- **Electronic NIRA notification in v1:** no MVRS interface for health systems was found.

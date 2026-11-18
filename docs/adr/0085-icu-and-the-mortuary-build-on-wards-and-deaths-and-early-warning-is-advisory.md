# ICU and the mortuary build on wards and deaths, and early warning is an advisory package

Decided in [How does the platform support ICU and HDU, early-warning scores and the mortuary?](https://github.com/carapai/people-centered-healthcare/issues/727), after the research [What do theatre, labour and delivery, blood transfusion, ICU and the mortuary require in Uganda?](https://github.com/carapai/people-centered-healthcare/issues/684) ([specialist-inpatient.md](https://github.com/carapai/people-centered-healthcare/blob/research/specialist-inpatient/docs/research/specialist-inpatient.md), branch `research/specialist-inpatient`, §5–6). It extends [ADR 0078](0078-inpatient-stays-are-received-into-wards-and-counted-from-location-history.md), which deferred the ICU, the mortuary, early-warning scores and death notification to NIRA.

The research found:
- HMIS 108 counts ICU and Neonatal Unit as Ward Types; there is no HDU Ward Type and no ICU-specific data set.
- The MPDSR guidelines name ICU and mortuary registers as sources for finding deaths, but no national form exists for either.
- No MoH document mandates an early-warning score; Ugandan studies at Mulago validate MEWS.
- Standard 7.50 requires "a control system" for the movement and release of bodies, including death notifications and related documents.
- The Births and Deaths Regulations 2015 make the facility administrator record every facility death and file weekly returns; the cause-of-death certificate (Form 13) reaches the registration officer within 7 days.

**The decision:**
- **Level of care.** A ward has a **level of care**, ICU or HDU, alongside its Ward Type. Ward Type stays HMIS 108's list; an HDU reports under the Ward Type the facility chooses, defaulting to ICU.
- **ICU Chart.** ICU and HDU stays use an **ICU Chart** package form on top of the inpatient chart: hourly vital signs, GCS, ventilation mode and settings, oxygen, and hourly fluid balance, as `Observation`s. Infusions and drugs stay on the administration chart. Ventilator days and readmission to ICU within 48 hours are local Indicators.
- **ICU register view** built from ICU and HDU stays: admission source, diagnosis, ventilated or not, days and outcome.
- **Early Warning Score.** A package, defaulting to MEWS, calculated from vital signs already charted. A facility activates it per ward; it is off by default. A score at or above the package threshold shows on the chart and raises an escalation Task for the nurse in charge and the doctor on call. It is advisory and never blocks anything. PEWS and MEOWS can follow as packages.
- **Mortuary.** The mortuary is a Location of type mortuary, not a ward, and outside the census. A **Mortuary Admission** on the deceased's Patient records arrival, source (ward, emergency unit or brought in dead), who brought and who received the body, identifying features, a photo, the storage place, and services such as embalming, charged from the Price Book.
- **Body Release.** Records the person collecting (name, NIN or other ID, relationship), the documents checked, and prints a **gate pass**. Unpaid fees stay as an account balance and **never block a release**.
- **Brought in dead and police cases.** A body brought in dead is registered as a Patient, through the unknown-patient flow if unidentified, with the death recorded. Any death can be marked a **police case**; it cannot be released until police clearance is recorded. A post-mortem is a `Procedure` with a signed report that informs the cause-of-death certificate.
- **Death Return.** When a death is recorded, the NIRA particulars (informant's name, capacity, NIN and signature) are collected. A weekly **Death Return** lists every facility death with the NIRA copy of its cause-of-death certificate, printed for the registration officer.
- **Mortuary register view** from arrival to release, until a national form exists.
- **Deferred.** ICU device integration, severity scores (APACHE, SOFA), mortuary fridge temperature monitoring, and quarterly mortality review tooling from the 2025 surveillance guidelines.

## Considered options

- **An HDU Ward Type of our own:** HMIS 108 has none, so it would not report; a level of care keeps both local and national views.
- **Early warning on by default, or enforced:** no MoH mandate exists, thresholds need local agreement, and ward vital-sign frequency varies; a forced score on sparse observations would alarm wrongly.
- **Holding bodies until fees are paid:** a known abuse that facilities are told not to practise; the balance is kept and pursued like any other.
- **The death return inside maternity:** registration law makes it facility-wide, so it belongs with deaths, not labour.

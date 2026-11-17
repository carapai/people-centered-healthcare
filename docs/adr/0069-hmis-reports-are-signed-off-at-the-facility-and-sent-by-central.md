# HMIS reports are signed off at the facility and sent to DHIS2 by central

Decided in [How does the platform report to DHIS2: who submits, when, and how are corrections handled?](https://github.com/carapai/people-centered-healthcare/issues/529), from the research in [dhis2-hmis.md](https://github.com/carapai/people-centered-healthcare/blob/research/dhis2-hmis/docs/research/dhis2-hmis.md) (branch `research/dhis2-hmis`). Uganda's eHMIS is DHIS2 and takes aggregate data sets only. Indicators are computed on each Facility Edge Node ([ADR 0068](0068-indicators-are-fhirpath-measures-computed-on-each-edge.md)). Integrations run only at central, and a facility's in-charge answers for the numbers sent in its name.

**The decision:**
- **Central sends.** Central's integration service posts `dataValueSets` and the completeness registration to DHIS2. The target (eHMIS directly, or a relay such as METS) and its credentials are national configuration plus a stored secret. One integration account, or a login per facility, are both supported until MoH says which it grants.
- **A Reporting package kind** maps indicators, or one disaggregation of an indicator, to a DHIS2 data set, data element and category option combo. Each mapping carries effective dates, because the forms are revised (2014, 2019, 2024) and their IDs change. Central's Validation checks the IDs against the target instance's metadata before publication.
- **Facility org unit.** Each facility's DHIS2 org unit ID is a facility setting that only central sets, checked against the National Health Facility Registry.
- **Sign-off before sending.** Nothing goes to DHIS2 until a user holding `reporting.submit` at the facility (normally the in-charge) reviews and signs off the form's preview for that period. The preview is built from the facility's Indicator Reports and works offline on the edge. The sign-off syncs, and central sends it on arrival. Nothing is sent automatically, and a form unsigned by MoH's deadline shows as late to the district.
- **Corrections.** A late entry or a fix marks that period's report changed. The in-charge sees the differences and signs again, and central resends the whole form for that period. DHIS2 overwrites, and the import summary shows what changed.
  - A rejection because the period is locked or approved becomes a task for the district HMIS focal person, never an automatic retry.
  - A value corrected to zero is sent as a deletion, unless the mapping marks that cell's zeros as significant.
- **v1 scope:** the aggregate forms 105 (OPD), 108 (inpatient) and 106a (HIV quarterly). The weekly 033b stays on mTRAC SMS. Case-based (tracker) surveillance waits until MoH asks for it.
- **Alongside manual entry.** Central keeps a per-facility, per-form switch, *reported through the platform*, so the district stops keying that form. If the district already entered values, the send overwrites them, and the overwrite counts are shown to the in-charge and the district.

## Considered options

- **The edge sends directly** (the UgandaEMR way): needs internet at the facility and a DHIS2 credential on every edge, and breaks the rule that integrations run at central.
- **Unattended scheduled sends:** numbers would leave in the in-charge's name without their review.
- **Mappings inside program packages:** program content would be tied to one country's HMIS form IDs, which change on their own schedule.

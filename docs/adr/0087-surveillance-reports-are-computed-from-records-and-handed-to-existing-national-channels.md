# Surveillance reports are computed from records and handed to existing national channels

Decided in [How does the platform support immunization surveillance: weekly 033b, AEFI reporting and VPD case investigation?](https://github.com/carapai/people-centered-healthcare/issues/742), after the research [What do immunization campaigns, weekly surveillance and AEFI reporting require in Uganda?](https://github.com/carapai/people-centered-healthcare/issues/740) ([immunization-campaigns.md](https://github.com/carapai/people-centered-healthcare/blob/research/immunization-campaigns/docs/research/immunization-campaigns.md), branch `research/immunization-campaigns`). It extends [ADR 0076](0076-immunization-is-a-core-record-with-a-hand-authored-schedule-and-computed-due-doses.md), which deferred AEFI to eIDSR and the weekly mTrac report, and [ADR 0069](0069-hmis-reports-are-signed-off-at-the-facility-and-sent-by-central.md), which kept HMIS 033b on mTRAC.

The research found:
- HMIS 033b is due every Monday for the past epidemiological week (Monday to Sunday), with cases, deaths, tested and positive per disease and zero reporting, sent by mTrac SMS that feeds DHIS2.
- AEFI, AFP, measles and neonatal tetanus are immediately reportable: case information to the next level within 24 hours.
- AEFI uses IDSR Annex 11A (the WHO layout, with vaccine, batch, expiry and time given); NDA manages reports in VigiFlow.
- AFP, measles and neonatal tetanus have case investigation forms; specimens go to the UVRI EPI Laboratory; district surveillance focal persons (DSFPs) enter cases into eIDSR.
- The mTrac SMS syntax, eIDSR identifiers and an electronic route into VigiFlow are unverified.

**The decision:**
- **Weekly 033b.** Each Monday the platform computes the HMIS 033b report for the past epidemiological week from coded diagnoses, deaths, laboratory results and AEFIs, through a row mapping in a package, every row filled including zeros. The in-charge reviews and signs it; it prints in triplicate and shows the figures ready to key into mTrac. Sending stays on mTrac.
- **Immediately reportable conditions.** The IDSR list ships in a package. A recorded diagnosis or event on it raises a Notification and a Task, due within 24 hours, for the facility surveillance focal person: notify the DSFP and complete the case form.
- **AEFI.** The `AdverseEvent` from ADR 0076 captures the Annex 11A fields; vaccine, batch, expiry, date and time and dose number come from the `Immunization`. Seriousness (hospitalised, died, life-threatening) is recorded. It prints as Annex 11A and counts in the 033b AE row.
- **Case investigation forms.** AFP, measles and neonatal tetanus forms are package Questionnaires filled from the record: demographics, the child's recorded doses, and for neonatal tetanus the mother's Td doses and delivery. Specimens are laboratory requests referred to UVRI with collection dates; results are entered on return; the EPID number and final classification are recorded. The form prints to travel with the specimen.
- **Response prompts.** Package Decision Support Rules: a confirmed neonatal tetanus case puts the mother on the Due List for Td; a measles case prompts vitamin A. Advisory, like other Advice.
- **Deferred.** Electronic submission to mTrac, eIDSR and VigiFlow until their identifiers and access are confirmed, and outbreak threshold alerting.

## Considered options

- **Sending 033b ourselves now:** the SMS syntax is unverified and ADR 0069 kept 033b on mTrac; computing it removes the error-prone tally without depending on an interface we cannot test.
- **Leaving 033b fully manual:** the data is already in the records, and copying from registers is where weekly figures go wrong.
- **Electronic AEFI to VigiFlow (E2B) in v1:** no confirmed route or credentials; the printed national form is what facilities send today.
- **Case forms as free text:** the forms ask for immunization history and the mother's Td doses, which the record already holds.

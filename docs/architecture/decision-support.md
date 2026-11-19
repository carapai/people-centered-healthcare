# Decision support

Point-of-care Advice from Decision Support Rules. Decided in [Does the platform run CQL for clinical decision support, and how?](https://github.com/carapai/people-centered-healthcare/issues/585) ([ADR 0074](../adr/0074-decision-support-is-fhirpath-decision-tables-translated-from-who-content-with-no-cql-runtime.md)).

## Guards, Decision Support Rules and Advice

| | Workflow guard | Decision Support Rule |
|---|---|---|
| Does | Allows or refuses a transition | Produces Advice for a clinician |
| Language | FHIRPath | FHIRPath decision table |
| Ships in | Workflow Definition | Program package |
| Can block | Yes, by design | Only a critical Advice at order-sign, and only until overridden with a reason |

## When rules run

| Hook | Moment | Typical Advice |
|---|---|---|
| `patient-view` | A chart is opened | Contact or dose due, test overdue, danger sign recorded |
| Form saved | A Questionnaire is saved | Danger sign: refer; next contact date |
| `order-sign` | An order or prescription is signed | Allergy to an ingredient (critical); drug interaction |
| Dispense | A pharmacist records a dispense | Drug interaction |

Rules are evaluated on the node holding the record, normally the Facility Edge Node, so they work offline. Clients show the cards: an Advice panel on the chart's Summary tab, and cards beside the order or form. Actionable Advice can create a Task in the worklist.

## Rules

- **Indicators on cards:** *info*, *warning* or *critical*. Only critical at `order-sign` needs an override with a reason, recorded as a `DetectedIssue`.
- **Emergency care is never stopped.**
- **No CQL runtime.** WHO SMART L3 content is translated at import, and untranslatable logic is flagged for a person to write.
- **Tests before publication.** Each rule ships with test patients and the Advice they should get.

## Drug interactions

Decided in [Where do drug interaction checks get their knowledge, and how do they act at order-sign?](https://github.com/carapai/people-centered-healthcare/issues/786) ([ADR 0090](../adr/0090-drug-interactions-come-from-a-curated-national-interaction-list-not-a-licensed-knowledge-base.md)).

- **Knowledge.** A curated national **Interaction List** package: each pair is a `ClinicalUseDefinition` (`interaction`) with two interactants (ingredients or named drug classes, membership in the package), effect, severity, management, source and review date. Seeded from the ONC high-priority pairs and the Uganda Clinical Guidelines ART table. No ATC codes.
- **Check.** One built-in rule, offline on the node, at order-sign and dispense. It compares the new order with the other orders being signed, active prescriptions, and dispenses within a 30-day look-back (an Override Point), including other facilities' in the shared record.
- **Severity.** Contraindicated: critical, override with a reason (`DetectedIssue`). Major: warning, not interruptive. Moderate: info on the chart. ONC non-interruptive pairs are never critical. Emergency care is never stopped.
- **Advice** names both medicines, the effect, management and source; actions are cancel the new order, stop or change the existing one, or override. An override covers the pair for that patient while both stay active.
- **Gaps and governance.** Medicine Products without ingredients are not checked and are listed in a report. National clinical governance maintains the list; facilities cannot remove or downgrade pairs. A monthly report gives alerts and overrides per pair.
- **Deferred.** A licensed knowledge base connector; duplicate-therapy and dose-range checks.

## Open

- Re-running the CQL spike when `octofhir-cql` closes the gaps found in #532.

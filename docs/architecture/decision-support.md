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
| `order-sign` | An order or prescription is signed | Allergy to an ingredient (critical) |

Rules are evaluated on the node holding the record, normally the Facility Edge Node, so they work offline. Clients show the cards: an Advice panel on the chart's Summary tab, and cards beside the order or form. Actionable Advice can create a Task in the worklist.

## Rules

- **Indicators on cards:** *info*, *warning* or *critical*. Only critical at `order-sign` needs an override with a reason, recorded as a `DetectedIssue`.
- **Emergency care is never stopped.**
- **No CQL runtime.** WHO SMART L3 content is translated at import, and untranslatable logic is flagged for a person to write.
- **Tests before publication.** Each rule ships with test patients and the Advice they should get.

## Open

- Drug-drug interaction checks, which need a licensed knowledge base.
- Re-running the CQL spike when `octofhir-cql` closes the gaps found in #532.

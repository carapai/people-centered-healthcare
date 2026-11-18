# Decision support is FHIRPath decision tables translated from WHO content, with no CQL runtime

Decided in [Does the platform run CQL for clinical decision support, and how?](https://github.com/carapai/people-centered-healthcare/issues/585), after the spike [Does octofhir-cql evaluate WHO SMART indicator libraries correctly?](https://github.com/carapai/people-centered-healthcare/issues/532) ([cql-spike.md](https://github.com/carapai/people-centered-healthcare/blob/research/cql-spike/docs/research/cql-spike.md), branch `research/cql-spike`). That spike found that the Rust engine translates none of WHO's 144 indicator libraries as published, and reads R5 data as null without an error. The Java reference engine runs them, but it needs a JVM on every facility PC and edge, and R4 data. WHO's guidelines (DAKs) state their decision logic as decision tables; the CQL is generated from them. ADR 0043 already gives workflows a FHIRPath decision-table construct.

**The decision:**
- **Two terms.** A *Decision Support Rule* is a published decision table: rows of FHIRPath conditions leading to outcomes, run against a patient's record at a defined moment. An *Advice* is what it produces for a clinician. Advice never moves a workflow; guards do (ADR 0043).
- **Point-of-care Advice in v1,** for the programmes the platform ships (such as ANC danger signs, contacts and doses due, tests overdue).
- **No CQL runtime.**
  - WHO SMART L3 logic is translated at import into Decision Support Rules.
  - Anything the importer cannot translate is flagged in the Studio for a person to write.
  - The trigger to revisit is a new `octofhir-cql` release, after which the spike is run again.
- **Evaluated on the server node.** Rules run in Rust, with the FHIRPath engine used for guards and indicators, on the node that holds the record. That is usually the Facility Edge Node, so they work offline. Clients only display Advice.
- **Hooks and cards.**
  - Rules run when a chart is opened (`patient-view`), a form is saved, and an order is signed (`order-sign`).
  - Each Advice is a card shaped like a CDS Hooks card: summary, indicator (*info*, *warning*, *critical*) and suggested actions.
  - Cards appear in the chart's Advice panel and beside the order or form. Actionable Advice (a dose due) can also create a Task.
  - No outside CDS Hooks services are called or offered in v1.
- **Blocking.**
  - Only a *critical* Advice at `order-sign` needs an explicit override, with a reason. Everything else is advisory.
  - Nothing blocks silently, and emergency care is never stopped.
  - An override is recorded as a `DetectedIssue` with its mitigation. Advice that is only shown is not stored beyond the audit log.
- **Safety checks.** The check of an allergy against a product's ingredients is in v1, using the platform's own AllergyIntolerance and product records. Drug-drug interactions wait for a decision on a licensed knowledge base.
- **Authoring.**
  - Decision Support Rules ship in program packages, versioned and published like other content (ADR 0025).
  - The program designer edits them as tables.
  - Each rule carries test patients (WHO's test cases where they exist) with their expected Advice. Publication refuses a rule that fails its tests.

## Considered options

- **The Java reference engine as a sidecar:** runs WHO's CQL as published, but it adds a JVM to every facility PC and edge, needs more memory and a second runtime to update, and needs R5 data converted to R4.
- **Patching `octofhir-cql`:** months of work on an outside engine, still needing R5-to-R4 conversion. We would rather send the spike's gaps upstream and re-run the spike.
- **Evaluating in the client:** would duplicate the engine in TypeScript, and a client may not hold the whole record.
- **Advice that blocks by default:** in this setting, alert fatigue and blocked care cost more than they save.
